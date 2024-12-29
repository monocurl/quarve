use std::ffi::c_void;

use crate::core::{Environment, MSlock};
use crate::native;
use crate::native::view::layer::set_layer_view_frame;
use crate::state::{FixedSignal, Signal, SignalOrValue};
use crate::util::geo::{Rect, Size, UNBOUNDED};
use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};
use crate::view::util::Color;

pub struct ColorView<S>(SignalOrValue<S>, *mut c_void) where S: Signal<Target=Color>;

impl ColorView<FixedSignal<Color>> {
    pub fn new(color: Color) -> Self {
        ColorView(SignalOrValue::value(color), 0 as *mut c_void)
    }
}

impl<S> ColorView<S> where S: Signal<Target=Color> {
    pub fn new_signal(color: S) -> Self {
        ColorView(SignalOrValue::Signal(color), 0 as *mut c_void)
    }
}

impl<E, S> ViewProvider<E> for ColorView<S> where E: Environment, S: Signal<Target=Color> {
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

    fn init_backing(&mut self, invalidator: WeakInvalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        self.0.add_invalidator(&invalidator, s);

        let nv = if let Some((nv, _)) = backing_source {
            nv
        }
        else {
            NativeView::layer_view(s)
        };

        self.1 = nv.backing();
        nv
    }

    fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock) -> bool {
        native::view::layer::update_layer_view(self.1, self.0.inner(s), Color::clear(), 0.0, 0.0, 1.0, s);
        false
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, _s: MSlock) -> (Rect, Rect) {
        (frame.full_rect(), frame.full_rect())
    }

    fn finalize_frame(&self, frame: Rect, s: MSlock) {
        set_layer_view_frame(self.1, frame, s);
    }
}

impl<E: Environment> IntoViewProvider<E> for Color {
    type UpContext = ();
    type DownContext = ();

    fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        ColorView::new(self)
    }
}

impl<E, S> IntoViewProvider<E> for ColorView<S> where E: Environment, S: Signal<Target=Color> {
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

    fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        EmptyVP
    }
}

struct EmptyVP;

impl<E> ViewProvider<E> for EmptyVP where E: Environment {
    type UpContext = ();
    type DownContext = ();

    fn intrinsic_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn xsquished_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn xstretched_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn ysquished_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn ystretched_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
        ()
    }

    fn init_backing(&mut self, _invalidator: WeakInvalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        let nv = if let Some((nv, _)) = backing_source {
            nv
        }
        else {
            NativeView::layout_view(s)
        };

        nv
    }

    fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) -> bool {
        false
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, _frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, _s: MSlock) -> (Rect, Rect) {
        (Rect::new(0., 0., 0., 0.), Rect::new(0., 0., 0., 0.))
    }
}
