use std::ffi::c_void;
use std::marker::PhantomData;
use std::sync::Arc;
use crate::core::{Environment, MSlock};
use crate::{native};
use crate::state::{Binding, Filterless, Store};
use crate::state::slock_cell::{MainSlockCell, SlockCell};
use crate::util::geo;
use crate::util::geo::{Rect, ScreenUnit, Size};
use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, NativeViewState, Subtree, View, ViewProvider, ViewRef};

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

    // all three are buggy at the moment
    #[allow(unused)]
    fn hoist_x_offset(
        self,
        x_offset: impl Binding<Filterless<ScreenUnit>> + Clone,
    ) -> impl IntoViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext> {
        self.hoist_offset(x_offset, Store::new(0.0).binding())
    }

    #[allow(unused)]
    fn hoist_y_offset(
        self,
        y_offset: impl Binding<Filterless<ScreenUnit>> + Clone,
    ) -> impl IntoViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext> {
        self.hoist_offset(Store::new(0.0).binding(), y_offset)
    }

    #[allow(unused)]
    fn hoist_offset(
        self,
        x_offset: impl Binding<Filterless<ScreenUnit>> + Clone,
        y_offset: impl Binding<Filterless<ScreenUnit>> + Clone
    ) -> impl IntoViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext> {
        ScrollViewBinding {
            vertical: self.vertical,
            horizontal: self.horizontal,
            binding_x: x_offset,
            binding_y: y_offset,
            content: self.content,
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
            content: self.content.into_view_provider(env, s).into_view(s),
            backing: 0 as *mut c_void,
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
            content: self.content.into_view_provider(env, s).into_view(s),
            backing: 0 as *mut c_void,
            phantom: Default::default(),
        }
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
    content: View<E, P>,
    backing: *mut c_void,
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

    fn init_backing(&mut self, _invalidator: Invalidator<E>, subtree: &mut Subtree<E>, _backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        subtree.push_subview(&self.content, env, s);

        let state = Arc::new(SlockCell::new(NativeViewState::default()));
        let weak_x = Arc::downgrade(&state);
        self.binding_x.listen(move |x, s| {
            let Some(strong) = weak_x.upgrade() else {
                return false;
            };

            strong.borrow_mut(s).offset_x = *x;
            true
        }, s);

        let weak_y = Arc::downgrade(&state);
        self.binding_y.listen(move |y, s| {
            let Some(strong) = weak_y.upgrade() else {
                return false;
            };

            strong.borrow_mut(s).offset_y = *y;
            true
        }, s);

        let mut nv = NativeView::new(native::view::scroll::init_scroll_view(self.vertical, self.horizontal, self.binding_y.clone(), self.binding_x.clone(), s), s);
        nv.set_clips_subviews();
        nv.set_state(state);
        nv
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