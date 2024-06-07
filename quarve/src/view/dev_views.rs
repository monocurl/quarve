use crate::core::{Environment, MSlock};
use crate::native;
use crate::state::Signal;
use crate::util::geo::{AlignedFrame, Point, Rect, Size};
use crate::util::Vector;
use crate::view::{Handle, IntoViewProvider, Invalidator, NativeView, Subtree, View, ViewProvider};

pub struct DebugView;
pub struct Layout<E: Environment, S: Signal<Vector<f32, 2>>>(pub View<E, DebugView>, pub View<E, DebugView>, pub S);
unsafe impl<E: Environment> ViewProvider<E> for DebugView {
    type LayoutContext = ();

    fn intrinsic_size(&self, _s: MSlock) -> Size {
        Size {
            w: 100.0,
            h: 100.0,
        }
    }

    fn xsquished_size(&self, _s: MSlock) -> Size {
        Size {
            w: 100.0,
            h: 100.0,
        }
    }

    fn ysquished_size(&self, s: MSlock) -> Size {
        Size {
            w: 100.0,
            h: 100.0,
        }
    }

    fn xstretched_size(&self, s: MSlock) -> Size {
        Size {
            w: 100.0,
            h: 100.0,
        }
    }

    fn ystretched_size(&self, s: MSlock) -> Size {
        Size {
            w: 100.0,
            h: 100.0,
        }
    }

    fn init_backing(&mut self, _invalidator: Invalidator<E>, _subviews: &mut Subtree<E>, _replaced_backing: Option<(NativeView, Self)>, _env: &mut Handle<E>, s: MSlock<'_>) -> NativeView {
        NativeView::new(native::view::debug_view_init(s))
    }

    fn layout_up(&mut self, _subviews: &mut Subtree<E>, _env: &mut Handle<E>, _s: MSlock<'_>) -> bool {
        false
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedFrame, _layout_context: &Self::LayoutContext, _env: &mut Handle<E>, _s: MSlock<'_>) -> Rect {
        frame.full_rect()
    }
}

unsafe impl<E: Environment, S: Signal<Vector<f32, 2>>> ViewProvider<E> for Layout<E, S> {
    type LayoutContext = ();

    fn intrinsic_size(&self, s: MSlock) -> Size {
        Size {
            w: 200.0,
            h: 200.0
        }
    }

    fn xsquished_size(&self, s: MSlock) -> Size {
        Size::new(100.0, 100.0)
    }

    fn ysquished_size(&self, s: MSlock) -> Size {
        Size::new(100.0, 100.0)
    }
    fn xstretched_size(&self, s: MSlock) -> Size {
        Size::new(1000.0, 400.0)
    }

    fn ystretched_size(&self, s: MSlock) -> Size {
        Size::new(1000.0, 400.0)
    }

    fn init_backing(&mut self, invalidator: Invalidator<E>, subviews: &mut Subtree<E>, _replaced_backing: Option<(NativeView, Self)>, env: &mut Handle<E>, s: MSlock<'_>) -> NativeView {
        subviews.push_subview(&self.0, env, s);
        subviews.push_subview(&self.1, env, s);

        self.2.listen(move |_, s| {
            let Some(invalidator) = invalidator.upgrade() else {
                return false;
            };

            invalidator.invalidate(s);
            true
        }, s);

        NativeView::new(native::view::init_layout_view(s))
    }

    fn layout_up(&mut self, subviews: &mut Subtree<E>, env: &mut Handle<E>, s: MSlock<'_>) -> bool {
        false
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedFrame, layout_context: &Self::LayoutContext, env: &mut Handle<E>, s: MSlock<'_>) -> Rect {
        let pos = self.2.borrow(s);
        self.0.layout_down(AlignedFrame {
            w: 100.0,
            h: 100.0,
            align: Default::default(),
        }, Point {
            x: 0.0,
            y: 0.0
        }, env, s);

        self.1.layout_down(AlignedFrame {
            w: 100.0,
            h: 100.0,
            align: Default::default(),
        }, Point {
            x: *pos.x(),
            y: *pos.y()
        }, env, s);

        frame.full_rect()
    }
}

impl<E: Environment> IntoViewProvider<E> for DebugView {
    fn into_view_provider(self, env: &E, s: MSlock) -> impl ViewProvider<E> {
        self
    }
}

impl<E: Environment, S: Signal<Vector<f32, 2>>> IntoViewProvider<E> for Layout<E, S> {
    fn into_view_provider(self, env: &E, s: MSlock) -> impl ViewProvider<E> {
        self
    }
}
