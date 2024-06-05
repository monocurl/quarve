use std::ffi::c_void;
use crate::core::{Environment, MSlock};
use crate::native;
use crate::state::Signal;
use crate::util::geo::{AlignedFrame, Point, Rect, Size};
use crate::view::{Handle, Invalidator, Subviews, View, ViewProvider};

pub struct DebugView;
pub struct Layout<E: Environment, S: Signal<f32>>(pub View<E, DebugView>, pub View<E, DebugView>, pub S);
unsafe impl<E: Environment> ViewProvider<E> for DebugView {
    type LayoutContext = ();

    fn intrinsic_size(&self, _s: MSlock) -> Size {
        Size {
            w: 100.0,
            h: 100.0,
        }
    }

    fn xsquished_size(&self, _s: MSlock) -> Size {
        todo!()
    }

    fn ysquished_size(&self, _s: MSlock) -> Size {
        todo!()
    }

    fn xstretched_size(&self, _s: MSlock) -> Size {
        todo!()
    }

    fn ystretched_size(&self, _s: MSlock) -> Size {
        todo!()
    }

    fn init_backing(&mut self, _invalidator: Invalidator<E>, _subviews: &mut Subviews<E>, _replaced_backing: Option<*mut c_void>, _replaced_provider: Option<Self>, _env: &mut Handle<E>, s: MSlock<'_>) -> *mut c_void {
        native::view::debug_view_init(s)
    }

    fn layout_up(&mut self, _subviews: &mut Subviews<E>, _env: &mut Handle<E>, _s: MSlock<'_>) -> bool {
        false
    }

    fn layout_down(&mut self, frame: AlignedFrame, _layout_context: &Self::LayoutContext, _env: &mut Handle<E>, _s: MSlock<'_>) -> Rect {
        frame.full_rect()
    }
}

unsafe impl<E: Environment, S: Signal<f32>> ViewProvider<E> for Layout<E, S> {
    type LayoutContext = ();

    fn intrinsic_size(&self, s: MSlock) -> Size {
        Size {
            w: 200.0,
            h: 200.0
        }
    }

    fn xsquished_size(&self, s: MSlock) -> Size {
        todo!()
    }

    fn ysquished_size(&self, s: MSlock) -> Size {
        todo!()
    }
    fn xstretched_size(&self, s: MSlock) -> Size {
        todo!()
    }

    fn ystretched_size(&self, s: MSlock) -> Size {
        todo!()
    }

    fn init_backing(&mut self, invalidator: Invalidator<E>, subviews: &mut Subviews<E>, replaced_backing: Option<*mut c_void>, replaced_provider: Option<Self>, env: &mut Handle<E>, s: MSlock<'_>) -> *mut c_void {
        subviews.push(&self.0, env, s);
        subviews.push(&self.1, env, s);

        self.2.listen(move |_, s| {
            let Some(invalidator) = invalidator.upgrade() else {
                return false;
            };

            invalidator.invalidate(s);
            true
        }, s);

        native::view::init_layout_view(s)
    }

    fn layout_up(&mut self, subviews: &mut Subviews<E>, env: &mut Handle<E>, s: MSlock<'_>) -> bool {
        false
    }

    fn layout_down(&mut self, frame: AlignedFrame, layout_context: &Self::LayoutContext, env: &mut Handle<E>, s: MSlock<'_>) -> Rect {
        let pos = self.2.borrow(s);
        self.0.layout_down(AlignedFrame {
            w: 100.0,
            h: 100.0,
            align: Default::default(),
        }, Point {
            x: 0.0,
            y: 0.0
        }, env, s);

        println!("Pos {:?}", *pos);
        self.1.layout_down(AlignedFrame {
            w: 100.0,
            h: 100.0,
            align: Default::default(),
        }, Point {
            x: 20.0 * *pos,
            y: 100.0
        }, env, s);

        frame.full_rect()
    }
}

struct VStack;
