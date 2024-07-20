use std::cell::Cell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Weak};
use crate::core::{Environment, MSlock, run_main_async, Slock, StandardConstEnv};
use crate::event::{Event, EventResult};
use crate::state::{DirectlyInvertible, StoreContainer};
use crate::state::slock_cell::SlockCell;
use crate::util::geo::{Rect, Size};
use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};
use crate::view::menu::MenuChannel;

#[derive(Default)]
struct History {
    // id is not currently being used
    // but in the future it can be used to merge consecutive entries in some cases
    menu: Option<MenuChannel>,
    callbacks: HashMap<usize, Vec<Box<dyn DirectlyInvertible>>>,
    order: Vec<usize>,
}

struct UndoManagerInner {
    is_undoing: Cell<bool>,
    undo: SlockCell<History>,
    redo: SlockCell<History>,
}

impl UndoManagerInner {
    fn new() -> Self {
        Self {
            is_undoing: Cell::new(false),
            undo: SlockCell::new(History::default()),
            redo: SlockCell::new(History::default()),
        }
    }

    fn disable_menus(&mut self, s: MSlock) {
        let mut undo = self.undo.borrow_mut(s);
        if undo.menu.as_ref().unwrap().is_set(s) {
            undo.menu.as_mut().unwrap().unset(s);
        }

        let mut redo = self.redo.borrow_mut(s);
        if redo.menu.as_ref().unwrap().is_set(s) {
            redo.menu.as_mut().unwrap().unset(s);
        }
    }

    fn update_menus(&self, weak: Weak<SlockCell<UndoManagerInner>>, s: MSlock) {
        {
            let mut undo = self.undo.borrow_mut(s);
            if undo.menu.as_ref().unwrap().is_set(s) {
                undo.menu.as_mut().unwrap().unset(s);
            }

            if !undo.order.is_empty() {
                let weak = weak.clone();
                undo.menu.as_mut().unwrap().set(Box::new(move |s| {
                    if let Some(strong) = weak.upgrade() {
                        strong.borrow(s)
                            .undo(s);
                    }
                }), None, s);
            }
        }

        {
            let mut redo = self.redo.borrow_mut(s);
            if redo.menu.as_ref().unwrap().is_set(s) {
                redo.menu.as_mut().unwrap().unset(s);
            }

            if !redo.order.is_empty() {
                let weak = weak.clone();
                redo.menu.as_mut().unwrap().set(Box::new(move |s| {
                    if let Some(strong) = weak.upgrade() {
                        strong.borrow(s)
                            .redo(s);
                    }
                }), None, s);
            }
        }
    }

    // caller expected to call update menus after this
    fn undo(&self, s: MSlock) {
        let mut undo = self.undo.borrow_mut(s);
        let id = undo.order.pop()
            .expect("No actions to undo");

        self.is_undoing.set(true);
        let mut action = undo.callbacks
            .get_mut(&id).unwrap()
            .pop().unwrap();
        action.invert(s.to_general_slock());
        self.is_undoing.set(false);
    }

    fn redo(&self, s: MSlock) {
        let mut redo = self.redo.borrow_mut(s);
        let id = redo.order.pop()
            .expect("No actions to undo");

        let mut action = redo.callbacks
            .get_mut(&id).unwrap()
            .pop().unwrap();
        action.invert(s.to_general_slock());
    }

    fn register_undo(&self, action: Box<dyn DirectlyInvertible>, s: Slock) {
        let mut undo = self.undo.borrow_mut(s);
        undo.order.push(action.id());
        undo.callbacks
            .entry(action.id())
            .or_default()
            .push(action);
    }

    fn register_redo(&self, action: Box<dyn DirectlyInvertible>, s: Slock) {
        let mut redo = self.redo.borrow_mut(s);
        redo.order.push(action.id());
        redo.callbacks
            .entry(action.id())
            .or_default()
            .push(action);
    }

    fn register_inverter(&self, action: Box<dyn DirectlyInvertible>, s: Slock) {
        if self.is_undoing.get() {
            self.register_redo(action, s);
        }
        else {
            self.register_undo(action, s);
        }
    }
}

#[derive(Clone)]
pub struct UndoManager {
    inner: Arc<SlockCell<UndoManagerInner>>
}

impl UndoManager {
    pub fn new(stores: &impl StoreContainer, s: MSlock) -> Self {
        let inner = Arc::new(SlockCell::new(UndoManagerInner::new()));

        let weak = Arc::downgrade(&inner);
        stores.subtree_inverse_listener(move |action, s| {
            let Some(strong) = weak.upgrade() else {
                return false;
            };

            strong.borrow(s)
                .register_inverter(action, s);

            // FIXME Would be nice to elide most async_main calls
            // OTOH, it may be more efficient than an invalidator call?
            // (which is also tricky to position here given the short lifetime of stores)
            let weak = weak.clone();
            run_main_async(move |s| {
                if let Some(strong) = weak.upgrade() {
                    let borrow = strong.borrow(s);
                    borrow.update_menus(weak.clone(), s)
                }
            });
            true
        }, s);

        UndoManager {
            inner,
        }
    }

    fn disable_menus(&self, s: MSlock) {
        let mut inner = self.inner.borrow_mut(s);
        inner.disable_menus(s);
    }

    fn update_menus(&self, s: MSlock) {
        let mut inner = self.inner.borrow_mut(s);
        inner.update_menus(Arc::downgrade(&self.inner), s);
    }
}

pub trait UndoManagerExt<E>: IntoViewProvider<E> where E: Environment, E::Const: AsRef<StandardConstEnv>, {
    fn mount_undo_manager(self, undo_manager: UndoManager)
        -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;

    fn mount_focused_undo_manager(self, undo_manager: UndoManager)
                    -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
}

impl<E, I> UndoManagerExt<E> for I where E: Environment, E::Const: AsRef<StandardConstEnv>, I: IntoViewProvider<E> {
    fn mount_undo_manager(self, undo_manager: UndoManager) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        UndoManagerIVP {
            source: self,
            undo_manager,
            focused_only: false,
            phantom: Default::default(),
        }
    }

    fn mount_focused_undo_manager(self, undo_manager: UndoManager) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        UndoManagerIVP {
            source: self,
            undo_manager,
            focused_only: true,
            phantom: Default::default(),
        }
    }
}

struct UndoManagerIVP<E, I> where E: Environment, E::Const: AsRef<StandardConstEnv>, I: IntoViewProvider<E> {
    source: I,
    undo_manager: UndoManager,
    focused_only: bool,
    phantom: PhantomData<E>
}

impl<E, I> IntoViewProvider<E> for UndoManagerIVP<E, I> where E: Environment, E::Const: AsRef<StandardConstEnv>, I: IntoViewProvider<E> {
    type UpContext = I::UpContext;
    type DownContext = I::DownContext;

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        // set undo channels
        self.undo_manager.inner
            .borrow_mut(s)
            .undo.borrow_mut(s)
            .menu = Some(env.as_ref().channels.undo_menu.clone());

        self.undo_manager.inner
            .borrow_mut(s)
            .redo.borrow_mut(s)
            .menu = Some(env.as_ref().channels.redo_menu.clone());

        UndoManagerVP {
            source: self.source.into_view_provider(env, s),
            undo_manager: self.undo_manager,
            focused_only: self.focused_only,
            phantom: Default::default(),
        }
    }
}

struct UndoManagerVP<E, P> where E: Environment, E::Const: AsRef<StandardConstEnv>, P: ViewProvider<E> {
    source: P,
    undo_manager: UndoManager,
    focused_only: bool,
    phantom: PhantomData<E>
}

impl<E, P> ViewProvider<E> for UndoManagerVP<E, P> where E: Environment, E::Const: AsRef<StandardConstEnv>, P: ViewProvider<E> {
    type UpContext = P::UpContext;
    type DownContext = P::DownContext;

    fn intrinsic_size(&mut self, s: MSlock) -> Size {
        self.source.intrinsic_size(s)
    }

    fn xsquished_size(&mut self, s: MSlock) -> Size {
        self.source.xsquished_size(s)
    }

    fn xstretched_size(&mut self, s: MSlock) -> Size {
        self.source.xstretched_size(s)
    }

    fn ysquished_size(&mut self, s: MSlock) -> Size {
        self.source.ysquished_size(s)
    }

    fn ystretched_size(&mut self, s: MSlock) -> Size {
        self.source.ystretched_size(s)
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        self.source.up_context(s)
    }

    fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        if let Some((nv, bs)) = backing_source {
            self.source.init_backing(invalidator, subtree, Some((nv, bs.source)), env, s)
        }
        else {
            self.source.init_backing(invalidator, subtree, None, env, s)
        }
    }

    fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
        self.source.layout_up(subtree, env, s)
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        self.source.layout_down(subtree, frame, layout_context, env, s)
    }

    fn pre_show(&mut self, s: MSlock) {
        self.undo_manager.update_menus(s);
        self.source.pre_show(s)
    }

    fn post_show(&mut self, s: MSlock) {
        self.source.post_show(s)
    }

    fn pre_hide(&mut self, s: MSlock) {
        self.undo_manager.disable_menus(s);
        self.source.pre_hide(s)
    }

    fn post_hide(&mut self, s: MSlock) {
        self.source.post_hide(s)
    }

    fn focused(&self, rel_depth: u32, s: MSlock) {
        self.source.focused(rel_depth, s);
        if self.focused_only {
            self.undo_manager.disable_menus(s);
        }
    }

    fn unfocused(&self, rel_depth: u32, s: MSlock) {
        self.source.unfocused(rel_depth, s);
        if self.focused_only {
            self.undo_manager.disable_menus(s);
        }
    }

    fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        self.source.push_environment(env, s)
    }

    fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        self.source.pop_environment(env, s)
    }

    fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
        self.source.handle_event(e, s)
    }
}