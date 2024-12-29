use std::ffi::c_void;
use std::marker::PhantomData;

use crate::core::{Environment, MSlock};
use crate::native;
use crate::native::view::scroll::{scroll_view_content, scroll_view_set_x, scroll_view_set_y};
use crate::state::{Bindable, Binding, Buffer, Filterless, Store};
use crate::util::geo;
use crate::util::geo::{Rect, ScreenUnit, Size};
use crate::view::{EnvRef, IntoViewProvider, NativeView, NativeViewState, Subtree, View, ViewProvider, ViewRef, WeakInvalidator};

pub struct ScrollView<E, I>
    where E: Environment,
          I: IntoViewProvider<E>,
{
    vertical: bool,
    horizontal: bool,
    content: I,
    phantom: PhantomData<E>
}

#[allow(unused)]
struct ScrollViewBinding<E, I, BX, BY>
    where E: Environment,
          I: IntoViewProvider<E>,
          BX: Binding<Filterless<ScreenUnit>> + Clone,
          BY: Binding<Filterless<ScreenUnit>> + Clone
{
    vertical: bool,
    horizontal: bool,
    binding_x: BX,
    binding_y: BY,
    content: I,
    phantom: PhantomData<E>
}

impl<E, I> ScrollView<E, I, > where E: Environment, I: IntoViewProvider<E> {
    pub fn vertical(content: I) -> Self {
        ScrollView {
            vertical: true,
            horizontal: false,
            content,
            phantom: PhantomData
        }
    }

    pub fn vertical_with_binding(
        content: I,
        offset_y: impl Binding<Filterless<ScreenUnit>> + Clone
    ) -> impl IntoViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext> {
        ScrollViewBinding {
            vertical: true,
            horizontal: false,
            binding_x: Store::new(0.0).binding(),
            binding_y: offset_y,
            content,
            phantom: Default::default(),
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

    pub fn horizontal_with_binding(
        content: I,
        offset_x: impl Binding<Filterless<ScreenUnit>> + Clone
    ) -> impl IntoViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext> {
        ScrollViewBinding {
            vertical: false,
            horizontal: true,
            binding_x: offset_x,
            binding_y: Store::new(0.0).binding(),
            content,
            phantom: Default::default(),
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

    pub fn horizontal_and_vertical_with_binding(
        content: I,
        x_offset: impl Binding<Filterless<ScreenUnit>> + Clone,
        y_offset: impl Binding<Filterless<ScreenUnit>> + Clone
    ) -> impl IntoViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext> {
        ScrollViewBinding {
            vertical: true,
            horizontal: true,
            binding_x: x_offset,
            binding_y: y_offset,
            content,
            phantom: Default::default(),
        }
    }
}

impl<E, I, BX, BY> IntoViewProvider<E> for ScrollViewBinding<E, I, BX, BY>
    where E: Environment,
          I: IntoViewProvider<E>,
          BX: Binding<Filterless<ScreenUnit>> + Clone,
          BY: Binding<Filterless<ScreenUnit>> + Clone
{
    type UpContext = I::UpContext;
    type DownContext = I::DownContext;

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        ScrollViewVP {
            vertical: self.vertical,
            horizontal: self.horizontal,
            binding_x: self.binding_x,
            binding_y: self.binding_y,
            backing: 0 as *mut c_void,
            content: ScrollViewContent {
                subview: self.content.into_view_provider(env, s).into_view(s)
            }.into_view(s),
            phantom: Default::default(),
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
            binding_x: Store::new(0.0).binding(),
            binding_y: Store::new(0.0).binding(),
            backing: 0 as *mut c_void,
            content: ScrollViewContent {
                subview: self.content.into_view_provider(env, s).into_view(s)
            }.into_view(s),
            phantom: Default::default(),
        }
    }
}

pub(crate) struct ScrollViewContent<E, P> where E: Environment, P: ViewProvider<E> {
    pub(crate) subview: View<E, P>
}

impl<E, P> ViewProvider<E> for ScrollViewContent<E, P>
    where E: Environment, P: ViewProvider<E>
{
    type UpContext = P::UpContext;
    type DownContext = P::DownContext;

    fn intrinsic_size(&mut self, s: MSlock) -> Size {
        self.subview.intrinsic_size(s)
    }

    fn xsquished_size(&mut self, s: MSlock) -> Size {
        self.subview.xsquished_size(s)
    }

    fn xstretched_size(&mut self, s: MSlock) -> Size {
        self.subview.xstretched_size(s)
    }

    fn ysquished_size(&mut self, s: MSlock) -> Size {
        self.subview.ysquished_size(s)
    }

    fn ystretched_size(&mut self, s: MSlock) -> Size {
        self.subview.ystretched_size(s)
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        self.subview.up_context(s)
    }

    fn init_backing(&mut self, _invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        let ret = if let Some((view, bs)) = backing_source {
            self.subview.take_backing(bs.subview, env, s);
            view
        }
        else {
            unsafe {
                NativeView::new(scroll_view_content(s), s)
            }
        };

        subtree.push_subview(&self.subview, env, s);
        ret
    }

    fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) -> bool {
        true
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        let ret = self.subview.layout_down_with_context(frame.full_rect(), layout_context, env, s);
        (ret, ret)
    }
}

struct ScrollViewVP<E, P, BX, BY>
    where E: Environment,
          P: ViewProvider<E>,
          BX: Binding<Filterless<ScreenUnit>> + Clone,
          BY: Binding<Filterless<ScreenUnit>> + Clone
{
    vertical: bool,
    horizontal: bool,
    binding_x: BX,
    binding_y: BY,
    backing: *mut c_void,
    content: View<E, P>,
    phantom: PhantomData<E>
}

impl<E, P, BX, BY> ViewProvider<E> for ScrollViewVP<E, P, BX, BY>
    where E: Environment,
          P: ViewProvider<E>,
          BX: Binding<Filterless<ScreenUnit>> + Clone,
          BY: Binding<Filterless<ScreenUnit>> + Clone
{
    type UpContext = P::UpContext;
    type DownContext = P::DownContext;

    fn intrinsic_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn xsquished_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn xstretched_size(&mut self, _s: MSlock) -> Size {
        Size::new(geo::UNBOUNDED, geo::UNBOUNDED)
    }

    fn ysquished_size(&mut self, _s: MSlock) -> Size {
        Size::new(0.0, 0.0)
    }

    fn ystretched_size(&mut self, _s: MSlock) -> Size {
        Size::new(geo::UNBOUNDED, geo::UNBOUNDED)
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        self.content.up_context(s)
    }

    fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        let state = Buffer::new(NativeViewState::default());

        let mut nv = {
            if let Some((nv, bs)) = backing_source {
                self.content.take_backing(bs.content, env, s);
                nv
            }
            else {
                unsafe {
                    NativeView::new(native::view::scroll::init_scroll_view(self.vertical, self.horizontal, self.binding_y.clone(), self.binding_x.clone(), s), s)
                }
            }
        };
        nv.set_clips_subviews();
        self.backing = nv.backing();
        let backing = nv.backing() as usize;


        let weak_x = state.downgrade();
        let inv = invalidator.clone();
        self.binding_x.listen(move |x, s| {
            let Some(strong) = weak_x.upgrade() else {
                return false;
            };

            if let Some(s) = s.try_to_main_slock() {
                scroll_view_set_x(backing as *mut c_void, *x, s);
            }
            else {
                // buffer was valid so this will be too
                inv.upgrade().unwrap().invalidate(s);
            }

            strong.borrow_mut(s).offset_x = *x;
            true
        }, s);

        let weak_y = state.downgrade();
        self.binding_y.listen(move |y, s| {
            let Some(strong) = weak_y.upgrade() else {
                return false;
            };

            if let Some(s) = s.try_to_main_slock() {
                scroll_view_set_y(backing as *mut c_void, *y, s);
            }
            else {
                invalidator.upgrade().unwrap().invalidate(s);
            }

            strong.borrow_mut(s).offset_y = *y;
            true
        }, s);

        subtree.push_subview(&self.content, env, s);
        nv.set_state(state);
        nv
    }

    fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock) -> bool {
        scroll_view_set_x(self.backing, *self.binding_x.borrow(s), s);
        scroll_view_set_y(self.backing, *self.binding_y.borrow(s), s);
        true
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        let w = if self.horizontal { geo::UNBOUNDED } else { frame.w };
        let h = if self.vertical { geo::UNBOUNDED } else { frame.h };

        let unbounded = Rect::new(0.0, 0.0, w, h);

        self.content.layout_down_with_context(unbounded, layout_context, env, s);
        (frame.full_rect(), frame.full_rect())
    }
}