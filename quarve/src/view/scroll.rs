use std::ffi::c_void;
use std::marker::PhantomData;
use crate::core::{Environment, MSlock};
use crate::{native, util};
use crate::util::geo;
use crate::util::geo::{Direction, Rect, Size};
use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, View, ViewProvider, ViewRef};

pub struct ScrollView<E, I> where E: Environment, I: IntoViewProvider<E> {
    vertical: bool,
    horizontal: bool,
    content: I,
    phantom: PhantomData<E>
}
impl<E, I> ScrollView<E, I> where E: Environment, I: IntoViewProvider<E> {
    pub fn vertical(content: I) -> Self {
        ScrollView {
            vertical: true,
            horizontal: false,
            content,
            phantom: PhantomData
        }
    }

    pub fn horizontal(content: I) -> Self {
        ScrollView {
            vertical: false,
            horizontal: true,
            content,
            phantom: PhantomData
        }
    }

    pub fn horizontal_and_vertical(content: I) -> Self {
        ScrollView {
            vertical: true,
            horizontal: true,
            content,
            phantom: PhantomData
        }
    }
}

impl<E, I> IntoViewProvider<E> for ScrollView<E, I> where E: Environment, I: IntoViewProvider<E> {
    type UpContext = I::UpContext;
    type DownContext = I::DownContext;

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        ScrollViewVP {
            vertical: self.vertical,
            horizontal: self.horizontal,
            content: self.content.into_view_provider(env, s).into_view(s),
            backing: 0 as *mut c_void,
            phantom: Default::default(),
        }
    }
}

struct ScrollViewVP<E, P> where E: Environment, P: ViewProvider<E> {
    vertical: bool,
    horizontal: bool,
    content: View<E, P>,
    backing: *mut c_void,
    phantom: PhantomData<E>
}

impl<E, P> ViewProvider<E> for ScrollViewVP<E, P> where E: Environment, P: ViewProvider<E> {
    type UpContext = P::UpContext;
    type DownContext = P::DownContext;

    fn intrinsic_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn xsquished_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn xstretched_size(&mut self, _s: MSlock) -> Size {
        Size::new(util::geo::UNBOUNDED, util::geo::UNBOUNDED)
    }

    fn ysquished_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn ystretched_size(&mut self, _s: MSlock) -> Size {
        Size::new(util::geo::UNBOUNDED, util::geo::UNBOUNDED)
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        self.content.up_context(s)
    }

    fn init_backing(&mut self, _invalidator: Invalidator<E>, subtree: &mut Subtree<E>, _backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        subtree.push_subview(&self.content, env, s);
        NativeView::new(native::view::scroll::init_scroll_view(self.vertical, self.horizontal, s))
    }

    fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) -> bool {
        true
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        let unbounded = Rect::new(0.0, 0.0, geo::UNBOUNDED, geo::UNBOUNDED);
        self.content.layout_down_with_context(unbounded, layout_context, env, s);
        (frame.full_rect(), frame.full_rect())
    }
}