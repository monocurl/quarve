use std::cell::Cell;
use std::collections::{VecDeque};
use std::marker::PhantomData;
use std::sync::{Arc, Weak};
use crate::core::{Environment, MSlock, run_main_async, Slock, slock_drop_listener, StandardConstEnv, StandardVarEnv};
use crate::event::{Event, EventResult};
use crate::state::{DirectlyInvertible, InverseListener, StoreContainer, UndoBarrier};
use crate::state::slock_cell::SlockCell;
use crate::util::geo::{Rect, Size};
use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};
use crate::view::menu::MenuChannel;
use crate::view::undo_manager::GroupState::Closed;

#[derive(PartialEq, Eq)]
enum GroupState {
    // after slock closes open -> open_prev_it, partially_closed -> closed
    Open,
    OpenPreviousIteration,
    PartiallyClosed,
    Closed,
}

struct History {
    menu: Option<MenuChannel>,
    callbacks: VecDeque<Box<dyn DirectlyInvertible>>,
    grouped_actions: VecDeque<(UndoBucket, usize)>, // number of events in each undo group, as well as bucket
    last_group_state: GroupState,
    mem_limit: usize,
}

impl History {
    fn new(limit: usize) -> History {
        History {
            menu: None,
            callbacks: VecDeque::new(),
            grouped_actions: VecDeque::new(),
            last_group_state: Closed,
            mem_limit: limit,
        }
    }

    fn register(&mut self, action: Box<dyn DirectlyInvertible>, bucket: UndoBucket) {
        self.callbacks.push_back(action);

        let needs_new = self.grouped_actions.is_empty() ||
            self.last_group_state == Closed ||
            // if it's different group numbers but same slock, keep them in same transaction
            // likewise, if they're different and opened previous iteration, we'll need a new one
            (self.grouped_actions.back().unwrap().0 != bucket &&
                self.last_group_state == GroupState::OpenPreviousIteration);

        if needs_new {
            // push new group
            self.grouped_actions.push_back((bucket, 1));
        }
        else {
            // realistically if you have different groups in same transaction
            // it's likely you just want them to be merged
            // but there still technically is a race condition
            // and the order in which groups are applied can matter
            // in practice, i dont think it's a large issue
            let back = self.grouped_actions.back_mut().unwrap();
            back.0 = bucket;
            back.1 += 1;
        }

        self.last_group_state = GroupState::Open;

        while self.grouped_actions.len() > self.mem_limit {
            let (_, drop_amount) = self.grouped_actions.pop_front().unwrap();
            for _ in 0..drop_amount {
                self.callbacks.pop_front();
            }
        }
    }

    fn clear(&mut self) {
        self.last_group_state = Closed;
        self.callbacks.clear();
        self.grouped_actions.clear();
    }
}

struct UndoManagerInner {
    is_undoing: Cell<bool>,
    is_redoing: Cell<bool>,
    undo: SlockCell<History>,
    redo: SlockCell<History>,
}

impl UndoManagerInner {
    fn new(undo_limit: usize) -> Self {
        Self {
            is_undoing: Cell::new(false),
            is_redoing: Cell::new(false),
            undo: SlockCell::new(History::new(undo_limit)),
            redo: SlockCell::new(History::new(undo_limit))
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

            if !undo.grouped_actions.is_empty() {
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

            if !redo.grouped_actions.is_empty() {
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

        let (_, multiplicity) = undo.grouped_actions.pop_back()
            .expect("No actions to undo");

        let current_redo_count = self.redo.borrow(s)
            .callbacks.len();
        let expected_redo_count = current_redo_count + multiplicity;

        // ensure new events do not accidentally end up grouped together
        undo.last_group_state = Closed;
        self.redo.borrow_mut(s).last_group_state = Closed;

        self.is_undoing.set(true);
        for _ in 0..multiplicity {
            let mut action = undo.callbacks.pop_back().unwrap();
            action.invert(s);
        }
        self.is_undoing.set(false);

        assert_eq!(expected_redo_count, self.redo.borrow(s).callbacks.len());
    }

    fn redo(&self, s: MSlock) {
        let mut redo = self.redo.borrow_mut(s);

        let (_, multiplicity) = redo.grouped_actions.pop_back()
            .expect("No actions to redo");

        let current_undo_count = self.undo.borrow(s)
            .callbacks.len();
        let expected_undo_count = current_undo_count + multiplicity;

        // ensure new events do not accidentally end up grouped together
        self.undo.borrow_mut(s).last_group_state = Closed;
        redo.last_group_state = Closed;

        self.is_redoing.set(true);
        for _ in 0..multiplicity {
            let mut action = redo.callbacks.pop_back().unwrap();
            action.invert(s);
        }
        self.is_redoing.set(false);

        assert_eq!(expected_undo_count, self.undo.borrow(s).callbacks.len());
    }

    fn register_inverter(&self, action: Box<dyn DirectlyInvertible>, bucket: UndoBucket, s: Slock) {
        if self.is_undoing.get() {
            self.redo.borrow_mut(s)
                .register(action, bucket);
        }
        else if self.is_redoing.get() {
            self.undo.borrow_mut(s)
                .register(action, bucket);
        }
        else {
            self.redo.borrow_mut(s)
                .clear();

            self.undo.borrow_mut(s)
                .register(action, bucket);
        }
    }
}


#[derive(Clone)]
pub struct UndoManager {
    inner: Arc<SlockCell<UndoManagerInner>>
}

#[derive(Default, Copy, Clone, PartialEq, Eq)]
pub struct UndoBucket(usize);

impl UndoBucket {
    pub const GLOBAL: UndoBucket = UndoBucket(0);

    pub(crate) fn new(tag: usize) -> Self {
        Self(tag)
    }
}

#[derive(Clone)]
struct UMListener {
    weak: Weak<SlockCell<UndoManagerInner>>
}

impl InverseListener for UMListener {
    fn handle_inverse(&mut self, inverse_action: Box<dyn DirectlyInvertible>, bucket: UndoBucket, s: Slock) -> bool {
        let Some(strong) = self.weak.upgrade() else {
            return false;
        };

        strong.borrow(s)
            .register_inverter(inverse_action, bucket, s.to_general_slock());

        // FIXME Would be nice to elide most async_main calls
        // OTOH, it may be more efficient than an invalidator call?
        // (which is also tricky to position here given the short lifetime of stores)
        let weak = self.weak.clone();
        run_main_async(move |s| {
            if let Some(strong) = weak.upgrade() {
                let borrow = strong.borrow(s);
                borrow.update_menus(weak.clone(), s)
            }
        });
        true
    }

    fn undo_barrier(&mut self, undo_barrier_type: UndoBarrier, s: Slock) {
        let Some(strong) = self.weak.upgrade() else {
            return;
        };

        match undo_barrier_type {
            UndoBarrier::Weak => {
                let borrow = strong.borrow(s);
                let mut undo = borrow.undo.borrow_mut(s);
                undo.last_group_state = match undo.last_group_state {
                    GroupState::Open => {
                        GroupState::PartiallyClosed
                    }
                    GroupState::OpenPreviousIteration => {
                        Closed
                    }
                    GroupState::PartiallyClosed => {
                        GroupState::PartiallyClosed
                    }
                    Closed => {
                        Closed
                    }
                }
            }
            UndoBarrier::Strong => {
                // easy: just mark undo as closed
                strong.borrow(s).undo.borrow_mut(s)
                    .last_group_state = Closed;
            }
        }
    }
}

impl UndoManager {
    pub fn new(stores: &impl StoreContainer, s: MSlock) -> Self {
        UndoManager::new_with_limit(stores, 8192, s)
    }

    pub fn new_with_limit(stores: &impl StoreContainer, undo_limit: usize, s: MSlock) -> Self {
        let inner =
            Arc::new(SlockCell::new(UndoManagerInner::new(undo_limit)));

        let weak = Arc::downgrade(&inner);
        stores.subtree_inverse_listener(UMListener {
            weak
        }, s);

        // close undo groups after the slock is dropped
        let weak = Arc::downgrade(&inner);
        slock_drop_listener(move |s| {
            let Some(strong) = weak.upgrade() else {
                return false;
            };

            let borrow = strong.borrow(s);

            // if open
            // if the last group was the global group, then
            // close it no matter what
            let mut undo = borrow.undo.borrow_mut(s);
            if undo.grouped_actions.back_mut().is_some_and(|v| v.0 == UndoBucket::GLOBAL) {
                undo.last_group_state = Closed;
            }
            else {
                // standard downgrade
                undo.last_group_state = match undo.last_group_state {
                    GroupState::Open | GroupState::OpenPreviousIteration => {
                        GroupState::OpenPreviousIteration
                    }
                    GroupState::PartiallyClosed | Closed => {
                        Closed
                    }
                }
            }

            // redo can just be closed fully no matter what
            borrow.redo.borrow_mut(s)
                .last_group_state = Closed;

            true
        });

        UndoManager {
            inner,
        }
    }

    fn disable_menus(&self, s: MSlock) {
        let mut inner = self.inner.borrow_mut(s);
        inner.disable_menus(s);
    }

    fn update_menus(&self, s: MSlock) {
        let inner = self.inner.borrow_mut(s);
        inner.update_menus(Arc::downgrade(&self.inner), s);
    }
}

pub trait UndoManagerExt<E>: IntoViewProvider<E>
    where E: Environment,
          E::Const: AsRef<StandardConstEnv>,
          E::Variable: AsMut<StandardVarEnv>
{
    fn mount_undo_manager(self, undo_manager: UndoManager)
        -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
}

impl<E, I> UndoManagerExt<E> for I
    where E: Environment,
          E::Const: AsRef<StandardConstEnv>,
          E::Variable: AsMut<StandardVarEnv>,
          I: IntoViewProvider<E> {
    fn mount_undo_manager(self, undo_manager: UndoManager) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        UndoManagerIVP {
            source: self,
            undo_manager,
            phantom: Default::default(),
        }
    }
}

struct UndoManagerIVP<E, I>
    where E: Environment,
          E::Const: AsRef<StandardConstEnv>,
          E::Variable: AsMut<StandardVarEnv>,
          I: IntoViewProvider<E>
{
    source: I,
    undo_manager: UndoManager,
    phantom: PhantomData<E>
}

impl<E, I> IntoViewProvider<E> for UndoManagerIVP<E, I>
    where E: Environment,
          E::Const: AsRef<StandardConstEnv>,
          E::Variable: AsMut<StandardVarEnv>,
          I: IntoViewProvider<E>
{
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
            phantom: Default::default(),
        }
    }
}

struct UndoManagerVP<E, P> where E: Environment, E::Const: AsRef<StandardConstEnv>, P: ViewProvider<E> {
    source: P,
    undo_manager: UndoManager,
    phantom: PhantomData<E>
}

impl<E, P> ViewProvider<E> for UndoManagerVP<E, P>
    where E: Environment,
          E::Const: AsRef<StandardConstEnv>,
          E::Variable: AsMut<StandardVarEnv>,
          P: ViewProvider<E> {
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
    }

    fn unfocused(&self, rel_depth: u32, s: MSlock) {
        self.source.unfocused(rel_depth, s);
    }

    fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        env.as_mut().undo_manager.push(self.undo_manager.clone());
        self.source.push_environment(env, s)
    }

    fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        self.source.pop_environment(env, s);
        env.as_mut().undo_manager.pop();
    }

    fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
        self.source.handle_event(e, s)
    }
}