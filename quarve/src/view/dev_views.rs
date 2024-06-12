use crate::core::{Environment, MSlock};
use crate::native;
use crate::state::Signal;
use crate::util::geo::{AlignedFrame, Point, Rect, Size};
use crate::util::Vector;
use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, View, ViewProvider};


pub struct DebugView;
pub struct Layout<E: Environment, S: Signal<Vector<f32, 2>>>(pub View<E, DebugView>, pub View<E, DebugView>, pub S);
impl<E: Environment> ViewProvider<E> for DebugView {
    type UpContext = ();
    type DownContext = ();

    fn intrinsic_size(&mut self, s: MSlock) -> Size {
        Size {
            w: 100.0,
            h: 100.0,
        }
    }

    fn xsquished_size(&mut self, s: MSlock) -> Size {
        Size {
            w: 100.0,
            h: 100.0,
        }
    }

    fn xstretched_size(&mut self, s: MSlock) -> Size {
        Size {
            w: 400.0,
            h: 1000.0,
        }
    }

    fn ysquished_size(&mut self, s: MSlock) -> Size {
        Size {
            w: 100.0,
            h: 100.0,
        }
    }

    fn ystretched_size(&mut self, s: MSlock) -> Size {
        Size {
            w: 400.0,
            h: 1000.0,
        }
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        ()
    }

    fn init_backing(&mut self, _invalidator: Invalidator<E>, _subviews: &mut Subtree<E>, _replaced_backing: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock<'_>) -> NativeView {
        unsafe {
            NativeView::new(native::view::debug_view_init(s))
        }
    }

    fn layout_up(&mut self, _subviews: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock<'_>) -> bool {
        false
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedFrame, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, s: MSlock<'_>) -> Rect {
        frame.full_rect()
    }
}

impl<E: Environment, S: Signal<Vector<f32, 2>>> ViewProvider<E> for Layout<E, S> {
    type UpContext = ();
    type DownContext = ();

    fn intrinsic_size(&mut self, s: MSlock) -> Size {
        Size {
            w: 200.0,
            h: 200.0
        }
    }

    fn xsquished_size(&mut self, s: MSlock) -> Size {
        Size::new(100.0, 100.0)
    }

    fn xstretched_size(&mut self, s: MSlock) -> Size {
        Size::new(1000.0, 400.0)
    }
    fn ysquished_size(&mut self, s: MSlock) -> Size {
        Size::new(100.0, 100.0)
    }

    fn ystretched_size(&mut self, s: MSlock) -> Size {
        Size::new(1000.0, 400.0)
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        ()
    }

    fn init_backing(&mut self, invalidator: Invalidator<E>, subviews: &mut Subtree<E>, _replaced_backing: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock<'_>) -> NativeView {
        subviews.push_subview(&self.0, env, s);
        subviews.push_subview(&self.1, env, s);

        self.2.listen(move |_, s| {
            let Some(invalidator) = invalidator.upgrade() else {
                return false;
            };

            invalidator.invalidate(s);
            true
        }, s);

        NativeView::layout_view(s)
    }

    fn layout_up(&mut self, subviews: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock<'_>) -> bool {
        false
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedFrame, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock<'_>) -> Rect {
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
    type UpContext = ();
    type DownContext = ();

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, DownContext=(), UpContext=()> {
        self
    }
}

impl<E: Environment, S: Signal<Vector<f32, 2>>> IntoViewProvider<E> for Layout<E, S> {
    type UpContext = ();
    type DownContext = ();

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, DownContext=(), UpContext=()> {
        self
    }
}
