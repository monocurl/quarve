use std::marker::PhantomData;
use crate::core::{Environment, MSlock};
use crate::state::{Signal};
use crate::view::{IntoViewProvider, View, ViewProvider};

pub struct IfIVP<S, E, P, N>
    where S: Signal<Target=bool>,
          E: Environment,
          P: IntoViewProvider<E>,
          N: IntoViewProvider<E, DownContext=P::DownContext>
{
    cond: S,
    curr: P,
    next: N,
    phantom: PhantomData<E>
}

pub struct NullNode<E, D> where E: Environment, D: 'static {
    phantom: PhantomData<(E, D)>
}

struct IfVP<S, E, P, N>
    where S: Signal<Target=bool>,
          E: Environment,
          P: ViewProvider<E>,
          N: ViewProvider<E, DownContext=P::DownContext>
{
    cond: S,
    curr: View<E, P>,
    next: N,
    phantom: PhantomData<E>
}

mod view_else_if {
    use std::marker::PhantomData;
    use crate::core::Environment;
    use crate::state::{FixedSignal, Signal};
    use crate::view::conditional::{IfIVP, NullNode};
    use crate::view::{IntoViewProvider};

    pub trait ViewElseIf<E>: IntoViewProvider<E> where E: Environment {
        fn view_else_if(self, when: impl Signal<Target=bool>, provider: impl IntoViewProvider<E, DownContext=Self::DownContext>)
                       -> IfIVP<
                           impl Signal<Target=bool>, E,
                           impl IntoViewProvider<E, DownContext=Self::DownContext>,
                           impl ViewElseIf<E, DownContext=Self::DownContext>,
                       >;

        fn view_else(self, provider: impl IntoViewProvider<E, DownContext=Self::DownContext>)
                    -> IfIVP<
                        impl Signal<Target=bool>,
                        E,
                        impl IntoViewProvider<E, DownContext=Self::DownContext>,
                        impl IntoViewProvider<E, DownContext=Self::DownContext>,
                    >;
    }

    impl<S, E, P, N> ViewElseIf<E> for IfIVP<S, E, P, N>
        where S: Signal<Target=bool>,
              E: Environment,
              P: IntoViewProvider<E>,
              N: ViewElseIf<E, DownContext=P::DownContext>
    {
        fn view_else_if(self, when: impl Signal<Target=bool>, provider: impl IntoViewProvider<E, DownContext=Self::DownContext>)
                       -> IfIVP<impl Signal<Target=bool>, E, impl IntoViewProvider<E, DownContext=Self::DownContext>, impl ViewElseIf<E, DownContext=Self::DownContext>> {
            IfIVP {
                cond: self.cond,
                curr: self.curr,
                next: self.next.view_else_if(when, provider),
                phantom: PhantomData
            }
        }

        fn view_else(self, provider: impl IntoViewProvider<E, DownContext=Self::DownContext>)
                    -> IfIVP<impl Signal<Target=bool>, E, impl IntoViewProvider<E, DownContext=Self::DownContext>, impl IntoViewProvider<E, DownContext=Self::DownContext>> {
            IfIVP {
                cond: self.cond,
                curr: self.curr,
                next: self.next.view_else(provider),
                phantom: PhantomData
            }
        }
    }

    impl<E, D> ViewElseIf<E> for NullNode<E, D>
        where E: Environment, D: 'static
    {
        fn view_else_if(self, when: impl Signal<Target=bool>, provider: impl IntoViewProvider<E, DownContext=Self::DownContext>) -> IfIVP<impl Signal<Target=bool>, E, impl IntoViewProvider<E, DownContext=Self::DownContext>, impl ViewElseIf<E, DownContext=Self::DownContext>> {
            IfIVP {
                cond: when,
                curr: provider,
                next: self,
                phantom: Default::default(),
            }
        }

        fn view_else(self, provider: impl IntoViewProvider<E, DownContext=Self::DownContext>) -> IfIVP<impl Signal<Target=bool>, E, impl IntoViewProvider<E, DownContext=Self::DownContext>, impl IntoViewProvider<E, DownContext=Self::DownContext>> {
            IfIVP {
                cond: FixedSignal::new(true),
                curr: provider,
                next: self,
                phantom: Default::default(),
            }
        }
    }
}
pub use view_else_if::*;

impl<S, E, P, N> IntoViewProvider<E> for IfIVP<S, E, P, N>
    where S: Signal<Target=bool>,
          E: Environment,
          P: IntoViewProvider<E>,
          N: IntoViewProvider<E, DownContext=P::DownContext>
{
    type UpContext = ();
    type DownContext = P::DownContext;

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        IfVP {
            cond: self.cond,
            curr: self.curr.into_view_provider(env, s).into_view(s),
            next: self.next.into_view_provider(env, s),
            phantom: PhantomData
        }
    }
}

impl<E, D> IntoViewProvider<E> for NullNode<E, D> where E: Environment, D: 'static {
    type UpContext = ();
    type DownContext = D;

    fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        self
    }
}

mod conditional_vp {
    use crate::core::{Environment, MSlock};
    use crate::state::{ActualDiffSignal, Signal};
    use crate::util::geo::{Rect, Size};
    use crate::view::conditional::{IfVP, NullNode};
    use crate::view::{EnvRef, Invalidator, NativeView, Subtree, ViewProvider, ViewRef};

    impl<S, E, P, N> ViewProvider<E> for IfVP<S, E, P, N>
        where S: Signal<Target=bool>,
              E: Environment,
              P: ViewProvider<E>,
              N: ViewProvider<E, DownContext=P::DownContext>
    {
        type UpContext = ();
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            if *self.cond.borrow(s) {
                self.curr.intrinsic_size(s)
            }
            else {
                self.next.intrinsic_size(s)
            }
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            if *self.cond.borrow(s) {
                self.curr.xsquished_size(s)
            }
            else {
                self.next.xsquished_size(s)
            }
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            if *self.cond.borrow(s) {
                self.curr.xstretched_size(s)
            }
            else {
                self.next.xstretched_size(s)
            }
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            if *self.cond.borrow(s) {
                self.curr.ysquished_size(s)
            }
            else {
                self.next.ysquished_size(s)
            }
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            if *self.cond.borrow(s) {
                self.curr.ystretched_size(s)
            }
            else {
                self.next.ystretched_size(s)
            }
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            ()
        }

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            let inv = invalidator.clone();

            self.cond.diff_listen(move |_, s| {
                let Some(invalidator) = invalidator.upgrade() else {
                    return false;
                };
                invalidator.invalidate(s);
                true
            }, s);

            if let Some((nv, this)) = backing_source {
                self.curr.take_backing(this.curr, env, s);
                self.next.init_backing(inv, subtree, Some((nv, this.next)), env, s)
            }
            else {
                self.next.init_backing(inv, subtree, None, env, s)
            }
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if *self.cond.borrow(s) {
                if !subtree.contains(&self.curr) {
                    subtree.clear_subviews(s);
                    subtree.push_subview(&self.curr, env, s);
                }
            }
            else {
                if subtree.contains(&self.curr) {
                    subtree.clear_subviews(s);
                }

                self.next.layout_up(subtree, env, s);
            }

            true
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            if *self.cond.borrow(s) {
                let used = self.curr.layout_down_with_context(frame.full_rect(), layout_context, env, s);
                (used, used)
            }
            else {
                self.next.layout_down(subtree, frame, layout_context, env, s)
            }
        }
    }

    impl<E, D> ViewProvider<E> for NullNode<E, D> where E: Environment, D: 'static {
        type UpContext = ();
        type DownContext = D;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            Size::new(0.0, 0.0)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
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

        fn init_backing(&mut self, _invalidator: Invalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            backing_source
                .map(|(nv, this)| nv)
                .unwrap_or_else(|| NativeView::layout_view(s))
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if subtree.len() != 0 {
                subtree.clear_subviews(s);
                true
            }
            else {
                false
            }
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, _frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, _s: MSlock) -> (Rect, Rect) {
            (Rect::new(0.0, 0.0, 0.0, 0.0),
             Rect::new(0.0, 0.0, 0.0, 0.0))
        }
    }
}
pub use conditional_vp::*;

pub fn view_if<S, E, P>(cond: S, view: P) -> IfIVP<S, E, P, NullNode<E, P::DownContext>> where S: Signal<Target=bool>, E: Environment, P: IntoViewProvider<E> {
    IfIVP {
        cond,
        curr: view,
        next: NullNode { phantom: PhantomData },
        phantom: PhantomData
    }
}