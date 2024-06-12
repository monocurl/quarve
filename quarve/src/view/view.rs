use std::sync::{Arc, Weak};
use crate::core::{Environment, MSlock, Slock};
use crate::state::slock_cell::{MainSlockCell};
use crate::util::geo::{AlignedFrame, Point, Rect};
use crate::util::rust_util::EnsureSend;
use crate::view::inner_view::{InnerView, InnerViewBase};
use crate::view::view_provider::ViewProvider;

pub struct View<E, P>(pub(crate) Arc<MainSlockCell<InnerView<E, P>>>)
    where E: Environment, P: ViewProvider<E> + ?Sized;

impl<E, P> View<E, P> where E: Environment, P: ViewProvider<E> {
    pub fn take_backing(&mut self, from: Self, env: &mut EnvRef<E>, s: MSlock) {
        let mut other_inner = Arc::into_inner(from.0)
            .expect("Can only take backing from view which has been removed from its superview")
            .into_inner_main(s);

        other_inner.graph().clear_subviews(s);

        let source = other_inner.into_backing_and_provider();

        // init backing directly
        let arc = self.0.clone() as Arc<MainSlockCell<dyn InnerViewBase<E>>>;
        self.0.borrow_mut_main(s)
            .take_backing(&arc, source, env, s)
    }

}

impl<E, P> View<E, P> where E: Environment, P: ViewProvider<E, DownContext=()> {
    pub fn layout_down(&self, aligned_frame: AlignedFrame, at: Point, parent_environment: &mut EnvRef<E>, s: MSlock) -> Rect {
        self.layout_down_with_context(aligned_frame, at, &(), parent_environment, s)
    }
}

mod view_ref {
    use std::sync::Arc;
    use crate::core::{Environment, MSlock};
    use crate::state::slock_cell::MainSlockCell;
    use crate::util::geo::{AlignedFrame, Point, Rect, Size};
    use crate::view::{EnvRef, InnerViewBase, View, ViewProvider};
    use crate::view::util::SizeContainer;

    /* hides provider type */
    // FIXME lots of repeated code
    pub trait ViewRef<E> where E: Environment {
        type UpContext: 'static;
        type DownContext: 'static;

        fn sizes(&self, s: MSlock) -> SizeContainer;
        fn intrinsic_size(&self, s: MSlock) -> Size;
        fn xsquished_size(&self, s: MSlock) -> Size;
        fn ysquished_size(&self, s: MSlock) -> Size;
        fn xstretched_size(&self, s: MSlock) -> Size;
        fn ystretched_size(&self, s: MSlock) -> Size;

        fn up_context(&self, s: MSlock) -> Self::UpContext;

        fn layout_down_with_context(
            &self,
            aligned_frame: AlignedFrame,
            at: Point,
            layout_context: &Self::DownContext,
            parent_environment: &mut EnvRef<E>,
            s: MSlock
        ) -> Rect;
    }

    pub trait TrivialContextViewRef<E> where E: Environment {
        fn layout_down(
            &self,
            aligned_frame: AlignedFrame,
            at: Point,
            parent_environment: &mut EnvRef<E>,
            s: MSlock
        ) -> Rect;
    }

    impl<E, V> TrivialContextViewRef<E> for V
        where E: Environment, V: ViewRef<E, DownContext=()> + ?Sized {
        #[inline]
        fn layout_down(&self, aligned_frame: AlignedFrame, at: Point, parent_environment: &mut EnvRef<E>, s: MSlock) -> Rect {
            self.layout_down_with_context(aligned_frame, at, &(), parent_environment, s)
        }
    }

    impl<E, P> ViewRef<E> for View<E, P> where E: Environment, P: ViewProvider<E> {
        type UpContext = P::UpContext;

        type DownContext = P::DownContext;

        fn sizes(&self, s: MSlock) -> SizeContainer {
            self.0.borrow_mut_main(s)
                .sizes(s)
        }

        fn intrinsic_size(&self, s: MSlock) -> Size {
            self.0.borrow_mut_main(s).intrinsic_size(s)
        }

        fn xsquished_size(&self, s: MSlock) -> Size {
            self.0.borrow_mut_main(s).xsquished_size(s)
        }

        fn ysquished_size(&self, s: MSlock) -> Size {
            self.0.borrow_mut_main(s).ysquished_size(s)
        }

        fn xstretched_size(&self, s: MSlock) -> Size {
            self.0.borrow_mut_main(s).xstretched_size(s)
        }

        fn ystretched_size(&self, s: MSlock) -> Size {
            self.0.borrow_mut_main(s).ystretched_size(s)
        }

        fn up_context(&self, s: MSlock) -> P::UpContext {
            self.0.borrow_mut_main(s)
                .provider()
                .up_context(s)
        }
        fn layout_down_with_context(&self, aligned_frame: AlignedFrame, at: Point, context: &P::DownContext, parent_environment: &mut EnvRef<E>, s: MSlock) -> Rect {
            let arc = self.0.clone() as Arc<MainSlockCell<dyn InnerViewBase<E>>>;

            self.0.borrow_mut_main(s)
                .layout_down_with_context(&arc, aligned_frame, at, parent_environment.0, context, s)
        }
    }
}
pub use view_ref::*;

pub struct Invalidator<E> where E: Environment {
    pub(crate) view: Weak<MainSlockCell<dyn InnerViewBase<E>>>
}

impl<E> Invalidator<E> where E: Environment {
    pub fn upgrade(&self) -> Option<StrongInvalidator<E>> {
        self.view.upgrade()
            .map(|view| {
                StrongInvalidator {
                    view
                }
            })
    }
}

pub struct StrongInvalidator<E> where E: Environment {
    view: Arc<MainSlockCell<dyn InnerViewBase<E>>>
}

impl<E> StrongInvalidator<E> where E: Environment {
    pub fn invalidate(&self, s: Slock) {
        // invalidate just this
        // safety:
        // the only part of window and view that we're
        // touching is send
        // (in particular, for view we touch the dirty flag
        // and the window back pointer, whereas for window
        // it's just the list of invalidated views)
        // FIXME add better descriptions of safety
        unsafe {
            self.view.borrow_mut_non_main_non_send(s)
                .invalidate(Arc::downgrade(&self.view), s);
        }
    }

    fn dfs(curr: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, s: Slock) {
        // safety is same reason invalidate above is safe
        // (only touching send parts)
        unsafe {
            let mut borrow = curr.borrow_mut_non_main_non_send(s);
            borrow.invalidate(Arc::downgrade(curr), s);

            for subview in borrow.graph().subviews() {
                StrongInvalidator::dfs(subview, s);
            }
        }
    }

    pub fn invalidate_environment(&self, s: Slock) {
        StrongInvalidator::dfs(&self.view, s);
    }
}

pub struct EnvRef<'a, E>(pub(crate) &'a mut E) where E: Environment;

impl<'a, E> EnvRef<'a, E> where E: Environment {
    pub fn const_env<'b>(&'b self) -> &'a E::Const
        where 'b: 'a
    {
        self.0.const_env()
    }

    pub fn variable_env<'b>(&'b self) -> &'a E::Variable
        where 'b: 'a
    {
        self.0.variable_env()
    }
}

impl<E> EnsureSend for Invalidator<E> where E: Environment {

}