use std::sync::{Arc, Weak};
use crate::core::{Environment, MSlock, Slock};
use crate::state::slock_cell::{MainSlockCell};
use crate::util::rust_util::EnsureSend;
use crate::view::inner_view::{InnerView, InnerViewBase};
use crate::view::view_provider::ViewProvider;
use crate::util::marker::ThreadMarker;

pub struct View<E, P>(pub(crate) Arc<MainSlockCell<InnerView<E, P>>>)
    where E: Environment, P: ViewProvider<E> + ?Sized;

impl<E, P> View<E, P> where E: Environment, P: ViewProvider<E> {
    pub fn take_backing(&self, from: Self, env: &mut EnvRef<E>, s: MSlock) {
        let mut other_inner = Arc::into_inner(from.0)
            .expect("Can only take backing from view which has been removed from its superview")
            .into_inner_main(s);

        other_inner.mut_graph().clear_subviews(s);

        let source = other_inner.into_backing_and_provider();

        // init backing directly
        let arc = self.0.clone() as Arc<MainSlockCell<dyn InnerViewBase<E>>>;
        self.0.borrow_mut_main(s)
            .take_backing(&arc, source, env, s)
    }

    // there are some circumstances where it's nice to have
    // provider access (mainly in conditional modifiers)
    // but this method generally should be avoided)
    pub(crate) fn with_provider(&self, f: impl FnOnce(&mut P), s: MSlock) {
        f(self.0.borrow_mut_main(s).provider())
    }

    // again, generally should avoid super invalidating child
    // but it does become a bit hard to avoid with conditional modifiers
    // without significant overhead
    pub(crate) fn invalidate(&self, s: MSlock) {
        let weak = Arc::downgrade(&self.0) as Weak<MainSlockCell<dyn InnerViewBase<E>>>;
        self.0.borrow_mut_main(s).invalidate(weak, s.to_general_slock());
    }
}

mod view_ref {
    use std::sync::Arc;
    use crate::core::{Environment, MSlock};
    use crate::state::slock_cell::MainSlockCell;
    use crate::util::geo::{Point, Rect, Size};
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
            at: Rect,
            layout_context: &Self::DownContext,
            parent_environment: &mut EnvRef<E>,
            s: MSlock
        ) -> Rect;

        // Translates the entire view subtree
        // Should only be called directly (possibly more than once)
        // after layout_down was called on this subview,
        // but before the parent
        // has finished its own layout_down call
        fn translate_post_layout_down(&self, by: Point, s: MSlock);

        // only call after layout down
        fn used_rect(&self, s: MSlock) -> Rect;
        fn suggested_rect(&self, s: MSlock) -> Rect;
    }

    pub trait TrivialContextViewRef<E> where E: Environment {
        fn layout_down(
            &self,
            aligned_frame: Rect,
            parent_environment: &mut EnvRef<E>,
            s: MSlock
        ) -> Rect;
    }

    impl<E, V> TrivialContextViewRef<E> for V
        where E: Environment, V: ViewRef<E, DownContext=()> + ?Sized {
        #[inline]
        fn layout_down(&self, at: Rect, parent_environment: &mut EnvRef<E>, s: MSlock) -> Rect {
            self.layout_down_with_context(at, &(), parent_environment, s)
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
            self.0.borrow_mut_main(s)
                .intrinsic_size(s)
        }

        fn xsquished_size(&self, s: MSlock) -> Size {
            self.0.borrow_mut_main(s)
                .xsquished_size(s)
        }

        fn ysquished_size(&self, s: MSlock) -> Size {
            self.0.borrow_mut_main(s)
                .ysquished_size(s)
        }

        fn xstretched_size(&self, s: MSlock) -> Size {
            self.0.borrow_mut_main(s)
                .xstretched_size(s)
        }

        fn ystretched_size(&self, s: MSlock) -> Size {
            self.0.borrow_mut_main(s)
                .ystretched_size(s)
        }

        fn up_context(&self, s: MSlock) -> P::UpContext {
            let mut inner = self.0.borrow_mut_main(s);

            debug_assert!(!inner.needs_layout_up() && inner.depth() != u32::MAX,
                          "This method must only be called after the subview \
                           has been mounted to the parent. Also, this view cannot be in an invalidated/dirty state");

            inner.provider()
                .up_context(s)
        }

        fn layout_down_with_context(&self, rect: Rect, context: &P::DownContext, parent_environment: &mut EnvRef<E>, s: MSlock) -> Rect {
            let arc = self.0.clone() as Arc<MainSlockCell<dyn InnerViewBase<E>>>;
            let mut borrow = self.0.borrow_mut_main(s);
            borrow.push_environment(parent_environment.0, s);
            let ret = borrow.layout_down_with_context(&arc, rect, parent_environment.0, context, s);
            borrow.pop_environment(parent_environment.0, s);
            ret
        }

        fn translate_post_layout_down(&self, by: Point, s: MSlock) {
            self.0.borrow_mut_main(s)
                .translate(by, s)
        }

        fn used_rect(&self, s: MSlock) -> Rect {
            self.0.borrow_mut_main(s)
                .used_rect(s)
        }

        fn suggested_rect(&self, s: MSlock) -> Rect {
            self.0.borrow_mut_main(s)
                .suggested_rect(s)
        }
    }


    // For portals
    pub(crate) trait ToArcViewBase<E>: ViewRef<E> where E: Environment {
        fn to_view_base(&self) -> Arc<MainSlockCell<dyn InnerViewBase<E>>>;
    }

    impl<E, P> ToArcViewBase<E> for View<E, P> where E: Environment, P: ViewProvider<E> {
        fn to_view_base(&self) -> Arc<MainSlockCell<dyn InnerViewBase<E>>> {
            self.0.clone()
        }
    }
}
pub use view_ref::*;

pub struct WeakInvalidator<E> where E: Environment {
    pub(crate) view: Weak<MainSlockCell<dyn InnerViewBase<E>>>
}

impl<E> WeakInvalidator<E> where E: Environment {
    pub fn try_upgrade_invalidate(&self, s: Slock<impl ThreadMarker>) -> bool {
        let Some(this) = self.upgrade() else {
            return false;
        };
        this.invalidate(s);
        true
    }

    pub fn upgrade(&self) -> Option<Invalidator<E>> {
        self.view.upgrade()
            .map(|view| {
                Invalidator {
                    view
                }
            })
    }
}

impl<E> PartialEq for WeakInvalidator<E> where E: Environment {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.view, &other.view)
    }
}

impl<E> Eq for WeakInvalidator<E> where E: Environment {
}

impl<E> Clone for WeakInvalidator<E> where E: Environment {
    fn clone(&self) -> Self {
        WeakInvalidator {
            view: self.view.clone(),
        }
    }
}

pub struct Invalidator<E> where E: Environment {
    view: Arc<MainSlockCell<dyn InnerViewBase<E>>>
}

impl<E> Invalidator<E> where E: Environment {
    pub fn invalidate(&self, s: Slock<impl ThreadMarker>) {
        // invalidate just this
        // safety:
        // the only part of window and view that we're
        // touching is send
        // (in particular, for view we touch the dirty flag
        // and the window back pointer, whereas for window
        // it's just the list of invalidated views)
        // FIXME add better descriptions of safety
        unsafe {
            self.view.borrow_non_main_non_send(s.to_general_slock())
                .invalidate(Arc::downgrade(&self.view), s.to_general_slock());
        }
    }

    fn dfs(curr: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, s: Slock<impl ThreadMarker>) {
        // safety is same reason invalidate above is safe
        // (only touching send parts)
        unsafe {
            let borrow = curr.borrow_non_main_non_send(s.to_general_slock());
            borrow.invalidate(Arc::downgrade(curr), s.to_general_slock());

            for subview in borrow.graph().subviews() {
                Invalidator::dfs(subview, s.to_general_slock());
            }
        }
    }

    pub fn invalidate_environment(&self, s: Slock<impl ThreadMarker>) {
        Invalidator::dfs(&self.view, s);
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

impl<E> EnsureSend for WeakInvalidator<E> where E: Environment {

}