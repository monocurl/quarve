use std::ffi::c_void;
use crate::core::{Environment, MSlock};
use crate::native;
use crate::state::{FixedSignal, Signal, SignalOrValue};
use crate::util::geo::{Rect, Size, UNBOUNDED};
use crate::view::util::Color;
use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};
use crate::view::modifers::{Frame, FrameModifiable};

pub struct ColorView<S>(SignalOrValue<Color, S>, *mut c_void) where S: Signal<Color>;

impl ColorView<FixedSignal<Color>> {
    pub fn new(color: Color) -> Self {
        ColorView(SignalOrValue::value(color), 0 as *mut c_void)
    }
}

impl<S> ColorView<S> where S: Signal<Color> {
    pub fn new_signal(color: S) -> Self {
        ColorView(SignalOrValue::Signal(color), 0 as *mut c_void)
    }
}

impl<E, S: Signal<Color>> ViewProvider<E> for ColorView<S> where E: Environment {
    type UpContext = ();
    type DownContext = ();

    fn intrinsic_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn xsquished_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn xstretched_size(&mut self, _s: MSlock) -> Size {
        Size::new(UNBOUNDED, UNBOUNDED)
    }

    fn ysquished_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn ystretched_size(&mut self, _s: MSlock) -> Size {
        Size::new(UNBOUNDED, UNBOUNDED)
    }

    fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
        ()
    }

    fn init_backing(&mut self, invalidator: Invalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        self.0.add_invalidator(&invalidator, s);

        let nv = if let Some((nv, _)) = backing_source {
            nv
        }
        else {
            NativeView::layer_view(s)
        };

        self.1 = nv.view();
        nv
    }

    fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock) -> bool {
        native::view::layer::update_layout_view(self.1, self.0.inner(s), Color::transparent(), 0.0, 0.0, 1.0, s);
        false
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, _s: MSlock) -> (Rect, Rect) {
        (frame.full_rect(), frame.full_rect())
    }
}

impl<E: Environment> IntoViewProvider<E> for Color {
    type UpContext = ();
    type DownContext = ();

    fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        ColorView::new(self)
    }
}

impl<E: Environment, S: Signal<Color>> IntoViewProvider<E> for ColorView<S> {
    type UpContext = ();
    type DownContext = ();

    fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        self
    }
}

pub struct EmptyView;

impl<E: Environment> IntoViewProvider<E> for EmptyView {
    type UpContext = ();
    type DownContext = ();

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        Color::transparent()
            .frame(Frame::default().intrinsic(0, 0))
            .into_view_provider(env, s)
    }
}