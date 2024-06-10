use std::sync::{Arc, Weak};
use crate::core::{Environment, MSlock, Slock};
use crate::state::slock_cell::{MainSlockCell, SlockCell};
use crate::util::geo::{AlignedFrame, Point, Rect, Size};
use crate::util::rust_util::EnsureSend;
use crate::view::inner_view::{InnerView, InnerViewBase};
use crate::view::util::SizeContainer;
use crate::view::view_provider::ViewProvider;

pub struct View<E, P>(pub(crate) Arc<MainSlockCell<InnerView<E, P>>>)
    where E: Environment, P: ViewProvider<E> + ?Sized;

impl<E, P> View<E, P> where E: Environment, P: ViewProvider<E> {
    pub fn take_backing(&mut self, from: Self, env: &mut EnvHandle<E>, s: MSlock) {
        let mut other_inner = Arc::into_inner(from.0)
            .expect("Can only take backing from view which has been removed from its superview")
            .into_inner_main(s);

        other_inner.subviews().clear_subviews(s);

        let source = other_inner.into_backing_and_provider();

        // init backing directly
        let weak_this = Arc::downgrade(&self.0) as Weak<MainSlockCell<dyn InnerViewBase<E>>>;
        self.0.borrow_mut_main(s)
            .take_backing(weak_this, source, env, s)
    }

    pub fn up_context(&self, s: MSlock) -> P::UpContext {
        self.0.borrow_mut_main(s)
            .provider()
            .up_context(s)
    }

    pub fn layout_down_with_context(&self, aligned_frame: AlignedFrame, at: Point, context: &P::DownContext, parent_environment: &mut EnvHandle<E>, s: MSlock) -> Rect {
        self.0.borrow_mut_main(s)
            .layout_down_with_context(aligned_frame, at, parent_environment.0, context, s)
    }

    pub fn intrinsic_size(&self, s: MSlock) -> Size {
        self.0.borrow_mut_main(s).intrinsic_size(s)
    }

    pub fn xsquished_size(&self, s: MSlock) -> Size {
        self.0.borrow_mut_main(s).xsquished_size(s)
    }

    pub fn xstretched_size(&self, s: MSlock) -> Size {
        self.0.borrow_mut_main(s).xstretched_size(s)
    }

    pub fn ysquished_size(&self, s: MSlock) -> Size {
        self.0.borrow_mut_main(s).ysquished_size(s)
    }

    pub fn ystretched_size(&self, s: MSlock) -> Size {
        self.0.borrow_mut_main(s).ystretched_size(s)
    }

    pub fn sizes(&self, s: MSlock) -> SizeContainer {
        self.0.borrow_mut_main(s)
            .provider().sizes(s)
    }
}

impl<E, P> View<E, P> where E: Environment, P: ViewProvider<E, DownContext=()> {
    pub fn layout_down(&self, aligned_frame: AlignedFrame, at: Point, parent_environment: &mut EnvHandle<E>, s: MSlock) -> Rect {
        self.layout_down_with_context(aligned_frame, at, &(), parent_environment, s)
    }
}

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

            for subview in borrow.subviews().subviews() {
                StrongInvalidator::dfs(subview, s);
            }
        }
    }

    pub fn invalidate_environment(&self, s: Slock) {
        StrongInvalidator::dfs(&self.view, s);
    }
}

pub struct EnvHandle<'a, E>(pub(crate) &'a mut E) where E: Environment;

impl<'a, E> EnvHandle<'a, E> where E: Environment {
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