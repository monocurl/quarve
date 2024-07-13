use std::marker::PhantomData;
use crate::core::{Environment, MSlock};
use crate::event::{Event, EventPayload, EventResult, MouseEvent};
use crate::state::{Binding, FixedSignal, SetAction, Signal, Store};
use crate::util::geo::{Rect, Size};
use crate::view::{EnvRef, IntoViewProvider, WeakInvalidator, NativeView, Subtree, ViewProvider, DummyProvider};
use crate::view::modifers::{Cursor, CursorModifiable, Layer, LayerModifiable, WhenModifiable};

pub struct Button {
    phantom_data: PhantomData<()>
}

impl Button {
    pub fn new<E>(text: String, action: impl Fn(MSlock) + 'static)
        -> impl IntoViewProvider<E, UpContext=(), DownContext=()>
        where E: Environment
    {
        todo!();
        return DummyProvider(PhantomData)
    }

    pub fn new_with_label<E, I>(view: I, action: impl Fn(MSlock) + 'static)
                            -> impl IntoViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext>
        where E: Environment, I: IntoViewProvider<E>
    {
        ButtonIVP {
            action,
            label: DefaultButtonLabel {
                source: view,
                phantom: Default::default(),
            },
            phantom: Default::default(),
        }
    }
}


pub struct ButtonIVP<E, A, L> where E: Environment, A: Fn(MSlock) + 'static, L: ButtonLabel<E> {
    action: A,
    label: L,
    phantom: PhantomData<E>
}

impl<E, A, L> IntoViewProvider<E> for ButtonIVP<E, A, L>
    where E: Environment,
          A: Fn(MSlock) + 'static,
          L: ButtonLabel<E>
{
    type UpContext = L::UpContext;
    type DownContext = L::DownContext;

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        self.label
            .into_button_view_provider(self.action, env, s)
    }
}

struct ButtonVP<E, A, P>
    where E: Environment,
          A: Fn(MSlock) + 'static,
          P: ViewProvider<E>
{
    is_hover: Store<bool>,
    is_click: Store<bool>,
    source: P,
    action: A,
    last_size: Size,
    phantom: PhantomData<E>
}

impl<E, A, P> ViewProvider<E> for ButtonVP<E, A, P>
    where E: Environment,
          A: Fn(MSlock) + 'static,
          P: ViewProvider<E>
{
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
        if let Some((nv, source)) = backing_source {
            self.source.init_backing(invalidator, subtree, Some((nv, source.source)), env, s)
        }
        else {
            self.source.init_backing(invalidator, subtree, None, env, s)
        }
    }

    fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
        self.source.layout_up(subtree, env, s)
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        let (ours, used) = self.source.layout_down(subtree, frame, layout_context, env, s);
        self.last_size = ours.size();
        (ours, used)
    }

    fn pre_show(&mut self, s: MSlock) {
        self.source.pre_show(s)
    }

    fn post_show(&mut self, s: MSlock) {
        self.source.post_show(s)
    }

    fn pre_hide(&mut self, s: MSlock) {
        self.source.pre_hide(s)
    }

    fn post_hide(&mut self, s: MSlock) {
        self.source.post_hide(s)
    }

    fn focused(&self, rel_depth: u32, s: MSlock) {
        self.source.focused(rel_depth, s)
    }

    fn unfocused(&self, rel_depth: u32, s: MSlock) {
        self.source.unfocused(rel_depth, s)
    }

    fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        self.source.push_environment(env, s)
    }

    fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        self.source.pop_environment(env, s)
    }

    fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
        if !e.is_mouse() {
            return self.source.handle_event(e, s);
        }

        let cursor = e.cursor();
        let inside = cursor.x >= 0.0 && cursor.x < self.last_size.w &&
            cursor.y >= 0.0 && cursor.y < self.last_size.h;
        if inside != *self.is_hover.borrow(s) {
            self.is_hover.apply(SetAction::Set(inside), s);
        }

        if !inside || matches!(&e.payload, EventPayload::Mouse(MouseEvent::LeftUp, _)) {
            if *self.is_click.borrow(s) {
                self.is_click.apply(SetAction::Set(false), s);
                return EventResult::Handled;
            }
        }
        else if inside && matches!(&e.payload, EventPayload::Mouse(MouseEvent::LeftDown, _)) {
            if !*self.is_click.borrow(s) {
                (self.action)(s);

                self.is_click.apply(SetAction::Set(true), s);
                return EventResult::Handled;
            }
        }

        self.source.handle_event(e, s)
    }
}

pub trait ButtonLabel<E> : Sized where E: Environment {
    type UpContext: 'static;
    type DownContext: 'static;

    fn into_button_view_provider(self, action: impl Fn(MSlock) + 'static, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>
    {
        let is_hover = Store::new(false);
        let is_click = Store::new(false);
        let source = self.styled(is_hover.signal(), is_click.signal(), env, s);

        ButtonVP {
            is_hover,
            is_click,
            source,
            action,
            last_size: Default::default(),
            phantom: Default::default(),
        }
    }

    fn styled(self, is_hover: impl Signal<Target=bool> + Clone, is_click: impl Signal<Target=bool> + Clone, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
}

struct DefaultButtonLabel<E, I> where E: Environment, I: IntoViewProvider<E> {
    source: I,
    phantom: PhantomData<E>
}

impl<E, I> ButtonLabel<E> for DefaultButtonLabel<E, I> where E: Environment, I: IntoViewProvider<E>
{
    type UpContext = I::UpContext;
    type DownContext = I::DownContext;

    fn styled(self, is_hover: impl Signal<Target=bool> + Clone, is_click: impl Signal<Target=bool> + Clone, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        self.source
            .when(is_hover, |l| {
                l.layer(Layer::default().opacity(0.3))
            })
            .when(is_click, |l| {
                l.layer(Layer::default().opacity(0.1))
            })
            .cursor(Cursor::Pointer)
            .into_view_provider(env, s)
    }
}