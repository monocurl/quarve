// TODO

use std::marker::PhantomData;
use crate::core::{Environment, MSlock};
use crate::state::{ActualDiffSignal, Buffer};
use crate::util::geo::{Rect, Size};
use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider, ViewRef};

pub struct ViewMatchIVP<E, S, U, D, F>
    where E: Environment, U: 'static, D: 'static,
          S: ActualDiffSignal, S::Target: Clone + PartialEq,
          F: FnMut(&S::Target, &mut Subtree<E>, &mut EnvRef<E>, MSlock) -> Box<dyn ViewRef<E, UpContext=U, DownContext=D>> + 'static
{
    signal: S,
    func: F,
    phantom: PhantomData<(E, D)>
}

struct ViewMatchVP<E, S, U, D, F>
    where E: Environment, U: 'static, D: 'static,
          S: ActualDiffSignal, S::Target: Clone + PartialEq,
          F: FnMut(&S::Target, &mut Subtree<E>, &mut EnvRef<E>, MSlock) -> Box<dyn ViewRef<E, UpContext=U, DownContext=D>> + 'static
{
    signal: S,
    view: Option<Box<dyn ViewRef<E, UpContext=U, DownContext=D>>>,
    dirty: Buffer<bool>,
    func: F,
    phantom: PhantomData<(E, U, D)>
}

impl<E, S, U, D, F> ViewMatchIVP<E, S, U, D, F>
    where E: Environment, U: 'static, D: 'static,
          S: ActualDiffSignal, S::Target: Clone + PartialEq,
          F: FnMut(&S::Target, &mut Subtree<E>, &mut EnvRef<E>, MSlock) -> Box<dyn ViewRef<E, UpContext=U, DownContext=D>> + 'static
{
    pub fn new(sig: S, f: F) -> Self {
        ViewMatchIVP {
            signal: sig,
            func: f,
            phantom: PhantomData
        }
    }
}

impl<E, S, U, D, F> IntoViewProvider<E> for ViewMatchIVP<E, S, U, D, F>
    where E: Environment, U: 'static, D: 'static,
          S: ActualDiffSignal, S::Target: Clone + PartialEq,
          F: FnMut(&S::Target, &mut Subtree<E>, &mut EnvRef<E>, MSlock) -> Box<dyn ViewRef<E, UpContext=U, DownContext=D>> + 'static
{
    type UpContext = U;
    type DownContext = D;

    fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        ViewMatchVP {
            signal: self.signal,
            view: None,
            func: self.func,
            dirty: Buffer::new(true),
            phantom: PhantomData
        }
    }
}

impl<E, S, U, D, F> ViewProvider<E> for ViewMatchVP<E, S, U, D, F>
    where E: Environment, U: 'static, D: 'static,
          S: ActualDiffSignal, S::Target: Clone + PartialEq,
          F: FnMut(&S::Target, &mut Subtree<E>, &mut EnvRef<E>, MSlock) -> Box<dyn ViewRef<E, UpContext=U, DownContext=D>> + 'static
{
    type UpContext = U;
    type DownContext = D;

    fn intrinsic_size(&mut self, s: MSlock) -> Size {
        self.view.as_ref().unwrap().intrinsic_size(s)
    }

    fn xsquished_size(&mut self, s: MSlock) -> Size {
        self.view.as_ref().unwrap().xsquished_size(s)
    }

    fn xstretched_size(&mut self, s: MSlock) -> Size {
        self.view.as_ref().unwrap().xstretched_size(s)
    }

    fn ysquished_size(&mut self, s: MSlock) -> Size {
        self.view.as_ref().unwrap().ysquished_size(s)
    }

    fn ystretched_size(&mut self, s: MSlock) -> Size {
        self.view.as_ref().unwrap().ystretched_size(s)
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        self.view.as_ref().unwrap().up_context(s)
    }

    fn init_backing(&mut self, invalidator: Invalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        let dirty = self.dirty.weak_buffer();
        self.signal.diff_listen(move |_val, s| {
            let (Some(invalidator), Some(mut dirty)) =
                (invalidator.upgrade(), dirty.upgrade()) else {
                return false;
            };
            dirty.replace(true, s);
            invalidator.invalidate(s);
            true
        }, s);

        if let Some((nv, _)) = backing_source {
            // we cant necessarily take backing since it may be of a different view type
            // so we will have to always reallocate
            nv
        }
        else {
            NativeView::layout_view(s)
        }
    }

    fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
        if self.dirty.take(s) {
            // function is responsible for adding and removing the subviews
            self.view = Some((self.func)(&*self.signal.borrow(s), subtree, env, s));
        }

        true
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        let used = self.view.as_ref().unwrap().layout_down_with_context(frame.full_rect(), layout_context, env, s);
        (used, used)
    }
}

#[macro_export]
macro_rules! view_match {
    ($sig: expr; $subtree: ident, $env: ident, $s: ident, __build: { $($p: pat => $v: expr,)* } $first_pat: pat => $first_view: expr $(, $tail_pat: pat => $tail_view: expr)*,) => {
        view_match! {
            $sig;
            $subtree, $env, $s,
            __build: {
                $($p => $v,)*
                $first_pat => {
                    $subtree.clear_subviews($s);
                    let view = $first_view.into_view_provider($env.const_env(), $s).into_view($s);
                    $subtree.push_subview(&view, $env, $s);
                    Box::new(view)
                },
            }
            $($tail_pat => $tail_view,)*
        }
    };
    ($sig: expr; $subtree: ident, $env: ident, $s: ident, __build: { $($p: pat => $v: expr,)* }) => {
        ViewMatchIVP::new($sig, |val, $subtree, $env, $s| {
            match val {
                $($p => $v,)*
            }
        })
    };
    ($sig: expr; $first_pat: pat => $first_view: expr $(, $tail_pat: pat => $tail_view: expr)* $(,)?) => {
        view_match! {
            $sig;
            subtree, env, s,
            __build: { }
            $first_pat => $first_view,
            $($tail_pat => $tail_view,)*
        }
    };
}

pub use view_match;