pub use general_layout::*;
pub use split::*;
pub use vec_layout::*;

mod general_layout {
    use std::marker::PhantomData;

    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::state::slock_cell::MainSlockCell;
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};

    pub trait LayoutProvider<E>: Sized + 'static where E: Environment {
        type UpContext: 'static;
        type DownContext: 'static;

        fn into_layout_view_provider(self) -> LayoutViewProvider<E, Self> {
            LayoutViewProvider(self, PhantomData)
        }

        fn intrinsic_size(&mut self, s: MSlock) -> Size;

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.intrinsic_size(s)
        }
        
        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.intrinsic_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.intrinsic_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.intrinsic_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext;

        fn init(
            &mut self,
            invalidator: WeakInvalidator<E>,
            subtree: &mut Subtree<E>,
            source_provider: Option<Self>,
            env: &mut EnvRef<E>,
            s: MSlock
        );

        fn layout_up(
            &mut self,
            subtree: &mut Subtree<E>,
            env: &mut EnvRef<E>,
            s: MSlock
        ) -> bool;

        fn layout_down(
            &mut self,
            subtree: &Subtree<E>,
            frame: Size,
            layout_context: &Self::DownContext,
            env: &mut EnvRef<E>,
            s: MSlock
        ) -> Rect;

        #[allow(unused_variables)]
        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            EventResult::NotHandled
        }
    }

    pub struct LayoutViewProvider<E, L>(L, PhantomData<MainSlockCell<E>>) where E: Environment, L: LayoutProvider<E>;

    impl<E, L> ViewProvider<E> for LayoutViewProvider<E, L>
        where E: Environment, L: LayoutProvider<E> {
        type UpContext = L::UpContext;
        type DownContext = L::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.0.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.0.xsquished_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.0.xstretched_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.0.ysquished_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.0.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.0.up_context(s)
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            if let Some(source) = backing_source {
                self.0.init(invalidator, subtree, Some(source.1.0), env, s);

                source.0
            } else {
                self.0.init(invalidator, subtree, None, env, s);

                NativeView::layout_view(s)
            }
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            self.0.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let rect = self.0.layout_down(subtree, frame, layout_context, env, s);
            (rect, rect)
        }

        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            self.0.handle_event(e, s)
        }
    }

    impl<E, L> IntoViewProvider<E> for LayoutViewProvider<E, L>
        where E: Environment, L: LayoutProvider<E>
    {
        type UpContext = L::UpContext;
        type DownContext = L::DownContext;

        fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl 'static + ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            self
        }
    }
}

mod split {
    use std::cell::Cell;
    use std::marker::PhantomData;
    use std::mem::swap;

    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventPayload, EventResult, MouseEvent};
    use crate::native::view::cursor::{init_cursor_view, pop_cursor, push_cursor};
    use crate::prelude::{Cursor, GRAY, LayoutProvider, Rect, ScreenUnit, Size};
    use crate::state::FixedSignal;
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, TrivialContextViewRef, View, ViewProvider, ViewRef, WeakInvalidator};
    use crate::view::color_view::ColorView;
    use crate::view::util::Color;

    const BAR_WIDTH: ScreenUnit = 1.0;
    const SPLITTER_WIDTH: ScreenUnit = 7.0;

    pub struct VSplit<E, L, R>
        where E: Environment,
              L: IntoViewProvider<E>,
              R: IntoViewProvider<E, DownContext=L::DownContext>,
    {
        top: L,
        bottom: R,
        phantom: PhantomData<E>,
        split_color: Color
    }

    impl<E, L, R> VSplit<E, L, R>
        where E: Environment,
              L: IntoViewProvider<E>,
              R: IntoViewProvider<E, DownContext=L::DownContext>,
    {
        pub fn new(top: L, bottom: R) -> Self {
            VSplit {
                top,
                bottom,
                phantom: Default::default(),
                split_color: GRAY
            }
        }

        pub fn split_color(mut self, color: Color) -> Self {
            self.split_color = color;
            self
        }
    }

    impl<E, L, R> IntoViewProvider<E> for VSplit<E, L, R>
        where E: Environment,
              L: IntoViewProvider<E>,
              R: IntoViewProvider<E, DownContext=L::DownContext>,
    {
        type UpContext = ();
        type DownContext = L::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            SplitViewVP::new(
                self.top.into_view_provider(env, s),
                self.split_color,
                self.bottom.into_view_provider(env, s),
                false, s
            ).into_layout_view_provider()
        }
    }

    pub struct HSplit<E, L, R>
        where E: Environment,
              L: IntoViewProvider<E>,
              R: IntoViewProvider<E, DownContext=L::DownContext>,
    {
        left: L,
        right: R,
        phantom: PhantomData<E>,
        split_color: Color
    }

    impl<E, L, R> HSplit<E, L, R>
        where E: Environment,
              L: IntoViewProvider<E>,
              R: IntoViewProvider<E, DownContext=L::DownContext>,
    {
        pub fn new(left: L, right: R) -> Self {
            HSplit {
                left,
                right,
                phantom: Default::default(),
                split_color: GRAY
            }
        }

        pub fn split_color(mut self, color: Color) -> Self {
            self.split_color = color;
            self
        }
    }

    impl<E, L, R> IntoViewProvider<E> for HSplit<E, L, R>
        where E: Environment,
              L: IntoViewProvider<E>,
              R: IntoViewProvider<E, DownContext=L::DownContext>,
    {
        type UpContext = ();
        type DownContext = L::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            SplitViewVP::new(
                self.left.into_view_provider(env, s),
                self.split_color,
                self.right.into_view_provider(env, s),
                true, s
            ).into_layout_view_provider()
        }
    }

    struct SplitterVP<E>
        where E: Environment
    {
        bar: View<E, ColorView<FixedSignal<Color>>>,
        width: ScreenUnit,
        is_horizontal: bool,
        invalidator: Option<WeakInvalidator<E>>,

        // mouse
        is_focused: Cell<bool>,
        virtual_position: Cell<ScreenUnit>,
        actual_position: ScreenUnit,
    }

    impl<E> ViewProvider<E> for SplitterVP<E>
        where E: Environment
    {
        type UpContext = ScreenUnit;
        type DownContext = ScreenUnit;

        fn intrinsic_size(&mut self, _s: MSlock) -> Size {
            Size::default()
        }

        fn xsquished_size(&mut self, _s: MSlock) -> Size {
            Size::default()
        }

        fn xstretched_size(&mut self, _s: MSlock) -> Size {
            Size::default()
        }

        fn ysquished_size(&mut self, _s: MSlock) -> Size {
            Size::default()
        }

        fn ystretched_size(&mut self, _s: MSlock) -> Size {
            Size::default()
        }

        fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
            self.virtual_position.get()
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            let nv = if let Some((nv, bs)) = backing_source {
                self.bar.take_backing(bs.bar, env, s);
                nv
            }
            else {
                unsafe {
                    NativeView::new(init_cursor_view(if self.is_horizontal { Cursor::HorizontalResize } else { Cursor::VerticalResize }, s), s)
                }
            };

            subtree.push_subview(&self.bar, env, s);

            self.invalidator = Some(invalidator);

            nv
        }

        fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) -> bool {
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.actual_position = *layout_context;

            let bar_frame = if self.is_horizontal {
                Rect::new(
                    frame.w / 2.0 - BAR_WIDTH / 2.0,
                    0.0,
                    BAR_WIDTH,
                    frame.h
                )
            } else {
                Rect::new(
                    0.0,
                    frame.h / 2.0 - BAR_WIDTH / 2.0,
                    frame.w,
                    BAR_WIDTH,
                )
            };

            self.bar.layout_down(bar_frame, env, s);

            (frame.full_rect(), frame.full_rect())
        }

        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            if let EventPayload::Mouse(event, at) = e.payload {
                match event {
                    MouseEvent::LeftDown => {
                        self.is_focused.set(true);
                        push_cursor(if self.is_horizontal { Cursor::HorizontalResize } else { Cursor::VerticalResize });
                        EventResult::FocusAcquire
                    }
                    MouseEvent::LeftDrag(_, _) => {
                        if self.is_focused.get() {
                            // update position
                            let delta = if self.is_horizontal {
                                at.x - self.width / 2.0
                            } else {
                                at.y - self.width / 2.0
                            };
                            self.virtual_position.set(
                                self.actual_position + delta
                            );
                            self.invalidator.as_ref().unwrap().try_upgrade_invalidate(s);
                        }
                        EventResult::Handled
                    }
                    MouseEvent::LeftUp => {
                        if e.for_focused {
                            pop_cursor();
                            self.is_focused.set(false);
                        }
                        EventResult::FocusRelease
                    }
                    _ => {
                        EventResult::NotHandled
                    }
                }
            }
            else {
                EventResult::NotHandled
            }
        }
    }

    impl<E, L, R> SplitViewVP<E, L, R>
        where E: Environment,
              L: ViewProvider<E>,
              R: ViewProvider<E, DownContext=L::DownContext>,
    {
        fn new(left: L, color: Color, right: R, is_horizontal: bool, s: MSlock) -> Self {
            let lead = left.into_view(s);
            let bar = ColorView::new(color).into_view(s);
            let splitter = SplitterVP {
                bar,
                width: SPLITTER_WIDTH,
                is_horizontal,
                invalidator: None,
                is_focused: Cell::new(false),
                virtual_position: Cell::new(0.0),
                actual_position: 0.0,
            }.into_view(s);
            let trail = right.into_view(s);

            SplitViewVP {
                lead,
                splitter,
                trail,
                splitter_size: SPLITTER_WIDTH,
                is_horizontal,
            }
        }
    }


    struct SplitViewVP<E, L, R>
        where E: Environment,
              L: ViewProvider<E>,
              R: ViewProvider<E, DownContext=L::DownContext>,
    {
        lead: View<E, L>,
        splitter: View<E, SplitterVP<E>>,
        trail: View<E, R>,
        splitter_size: ScreenUnit,
        is_horizontal: bool
    }

    impl<E, L, R> LayoutProvider<E> for SplitViewVP<E, L, R>
        where E: Environment,
              L: ViewProvider<E>,
              R: ViewProvider<E, DownContext=L::DownContext>,
    {
        type UpContext = ();
        type DownContext = L::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.combine_size(
                self.lead.intrinsic_size(s),
                self.trail.intrinsic_size(s)
            )
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.combine_size(
                self.lead.xsquished_size(s),
                self.trail.xsquished_size(s)
            )
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.combine_size(
                self.lead.xstretched_size(s),
                self.trail.xstretched_size(s)
            )
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.combine_size(
                self.lead.ysquished_size(s),
                self.trail.ysquished_size(s)
            )
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.combine_size(
                self.lead.ystretched_size(s),
                self.trail.ystretched_size(s)
            )
        }

        fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
            ()
        }

        fn init(&mut self, _invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<Self>, env: &mut EnvRef<E>, s: MSlock) {
            if let Some(source) = backing_source {
                self.lead.take_backing(source.lead, env, s);
                self.splitter.take_backing(source.splitter, env, s);
                self.trail.take_backing(source.trail, env, s);
            }

            subtree.push_subview(&self.lead, env, s);
            subtree.push_subview(&self.trail, env, s);
            subtree.push_subview(&self.splitter, env, s);
        }

        fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) -> bool {
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> Rect {
            let (min, max) = if self.is_horizontal {
                (self.lead.xsquished_size(s).w, self.lead.xstretched_size(s).w)
            } else {
                (self.lead.ysquished_size(s).w, self.lead.ystretched_size(s).w)
            };
            let pos = (self.splitter.up_context(s) - BAR_WIDTH / 2.0)
                .clamp(min, max);

            if self.is_horizontal {
                self.lead.layout_down_with_context(
                    Rect::new(0.0, 0.0, pos, frame.h),
                    layout_context, env, s);

                let ctx = pos + BAR_WIDTH / 2.0;
                self.splitter.layout_down_with_context(
                    Rect::new(pos + BAR_WIDTH / 2.0 - self.splitter_size / 2.0, 0.0, self.splitter_size, frame.h),
                   &ctx,
                    env, s
                );
                self.trail.layout_down_with_context(
                    Rect::new(pos + BAR_WIDTH, 0.0, frame.w - pos - BAR_WIDTH, frame.h),
                    layout_context, env, s);
            }
            else {
                self.lead.layout_down_with_context(
                    Rect::new(0.0, 0.0, frame.w, pos),
                    layout_context, env, s);
                let ctx = pos + BAR_WIDTH / 2.0;
                self.splitter.layout_down_with_context(
                    Rect::new(0.0,pos - self.splitter_size / 2.0, frame.w, self.splitter_size),
                    &ctx,
                    env, s
                );
                self.trail.layout_down_with_context(
                    Rect::new(0.0, pos + BAR_WIDTH, frame.w, frame.h - pos - BAR_WIDTH),
                    layout_context, env, s);
            }

            frame.full_rect()
        }
    }

    impl<E, L, R> SplitViewVP<E, L, R>
        where E: Environment,
              L: ViewProvider<E>,
              R: ViewProvider<E, DownContext=L::DownContext>,
    {
        fn combine_size(&self, mut u: Size, mut v: Size) -> Size {
            if self.is_horizontal {
                swap(&mut u.w, &mut u.h);
                swap(&mut v.w, &mut v.h);
            }

            let mut ret = Size::new(
                u.w.max(v.w),
                u.h + BAR_WIDTH + v.h
            );

            if self.is_horizontal {
                swap(&mut ret.w, &mut ret.h);
            }

            ret
        }
    }
}


mod vec_layout {
    pub use binding_layout::*;
    pub use flex::*;
    pub use hetero_layout::*;
    pub use hstack::*;
    pub use impls::*;
    pub use macros::*;
    pub use signal_layout::*;
    pub use vstack::*;
    pub use zstack::*;

    use crate::core::{Environment, MSlock};
    use crate::util::FromOptions;
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, ViewProvider, ViewRef, WeakInvalidator};

    // workaround for TAIT
    fn into_view_provider<E, I>(i: I, e: &E::Const, s: MSlock)
                                -> impl ViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext> + 'static
        where E: Environment, I: IntoViewProvider<E>
    {
        fn _into_view_provider<E, I>(i: I, e: &'static E::Const, s: MSlock<'static>)
                                     -> impl ViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext> + 'static
            where E: Environment, I: IntoViewProvider<E>
        {
            i.into_view_provider(e, s)
        }
        // safety:
        // ViewProvider is static so we can rest assured that any implementation
        // cannot borrow from e or s
        // therefore, the return type is invariant with respect to the argument types
        // and since it's not borrowing from e or s
        // there is no need to worry about invalid references
        let (long_e, long_s) = unsafe {
            std::mem::transmute::<(&E::Const, MSlock), (&'static E::Const, MSlock<'static>)>((e, s))
        };

        _into_view_provider(i, long_e, long_s)
    }

    pub trait VecLayoutProvider<E>: Sized + FromOptions + 'static where E: Environment {
        type DownContext: 'static;
        type UpContext: 'static;
        type SubviewDownContext: 'static;
        type SubviewUpContext: 'static;

        #[allow(unused_variables)]
        fn init(&mut self, invalidator: WeakInvalidator<E>, s: MSlock) {

        }

        fn intrinsic_size(&mut self, s: MSlock) -> Size;
        fn xsquished_size(&mut self, s: MSlock) -> Size;
        fn ysquished_size(&mut self, s: MSlock) -> Size;
        fn xstretched_size(&mut self, s: MSlock) -> Size;
        fn ystretched_size(&mut self, s: MSlock) -> Size;

        fn up_context(&mut self, s: MSlock) -> Self::UpContext;

        fn layout_up<'a, P>(
            &mut self,
            subviews: impl Iterator<Item=&'a P> + Clone,
            env: &mut EnvRef<E>,
            s: MSlock
        ) -> bool where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a;

        fn layout_down<'a, P>(
            &mut self,
            subviews: impl Iterator<Item=&'a P> + Clone,
            frame: Size,
            context: &Self::DownContext,
            env: &mut EnvRef<E>,
            s: MSlock
        ) -> Rect where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a;

        fn hetero() -> HeteroIVP<E, impl HeteroIVPNode<E, Self::SubviewUpContext, Self::SubviewDownContext>, Self>
        {
            new_hetero_ivp(Self::from_options(Self::Options::default()))
        }

        fn hetero_options(options: Self::Options) -> HeteroIVP<E, impl HeteroIVPNode<E, Self::SubviewUpContext, Self::SubviewDownContext>, Self>
        {
            new_hetero_ivp(Self::from_options(options))
        }
    }

    mod macros {
        // https://github.com/rust-lang/rust/issues/35853#issuecomment-415993963
        #[macro_export]
        macro_rules! impl_hetero_layout {
            (__append_dollar_sign $($body:tt)*) => {
                macro_rules! __quarve_inner_with_dollar_sign { $($body)* }
                __quarve_inner_with_dollar_sign!($);
            };
            ($t: ty, $macro_name: ident) => {
                impl_hetero_layout! {
                    __append_dollar_sign
                    ($d:tt) => {
                        #[macro_export]
                        macro_rules! $macro_name {
                            () => {
                                $t::hetero()
                            };
                            (__build: $d built: expr;) => {
                                $d built
                            };
                            (__build: $d built: expr; $d first: expr; $d($d child: expr;)*) => {
                                $macro_name! {
                                    __build: $d built.push($first);
                                    $d($d child;)*
                                }
                            };
                            ($d first: expr $d(; $d child: expr )* $d(;)?) => {
                                $macro_name! {
                                    __build: $t::hetero();
                                    $d first;
                                    $d($d child;)*
                                }
                            };
                        }
                    }
                }
            };
        }
        pub use impl_hetero_layout;

        #[macro_export]
        macro_rules! impl_signal_layout_extension {
            (__declare_trait $t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident) => {
                pub trait $trait_name<T, S, E> where T: Send + 'static, S: Signal<Target=Vec<T>>, E: Environment {
                    fn $method_name<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static)
                        -> impl IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                        where P: IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext>,
                              P::UpContext: Into<<$t as VecLayoutProvider<E>>::SubviewUpContext>;
                fn $method_name_options<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static, options: <$t as FromOptions>::Options)
                        -> impl IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                        where P: IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext>,
                              P::UpContext: Into<<$t as VecLayoutProvider<E>>::SubviewUpContext>;
                }
            };
            (__impl_trait $t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident) => {
                fn $method_name<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static)
                    -> impl IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                        where P: IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext>,
                              P::UpContext: Into<<$t as VecLayoutProvider<E>>::SubviewUpContext>
                {
                    VecSignalLayout::new(self, map, <$t as FromOptions>::from_options(<$t as FromOptions>::Options::default()))
                }

                fn $method_name_options<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static, options: <$t as FromOptions>::Options)
                        -> impl IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                        where P: IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext>,
                              P::UpContext: Into<<$t as VecLayoutProvider<E>>::SubviewUpContext>
                {
                    VecSignalLayout::new(self, map, <$t as FromOptions>::from_options(options))
                }
            };

            ($t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident, where E: $env: path) => {
                impl_signal_layout_extension!(__declare_trait  $t, $trait_name, $method_name, $method_name_options);

                impl<E, T, S> $trait_name<T, S, E> for S where T: Send + 'static, S: Signal<Target=Vec<T>>, E: $env
                {
                    impl_signal_layout_extension!(__impl_trait $t, $trait_name, $method_name, $method_name_options);
                }
            };
            ($t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident, where E = $env: ty) => {
                mod {
                    type E = $env;
                    impl_signal_layout_extension!(__declare_trait  $t, $trait_name, $method_name, $method_name_options);

                    impl<T, S> $trait_name<T, S, E> for S where T: Send + 'static, S: Signal<Target=Vec<T>>
                    {
                        impl_signal_layout_extension!(__impl_trait $t, $trait_name, $method_name, $method_name_options);
                    }
                }
            }
        }
        pub use impl_signal_layout_extension;

        #[macro_export]
        macro_rules! impl_binding_layout_extension {
            (__declare_trait $t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident) => {
                pub trait $trait_name<T, F, S, E> where T: StoreContainer, F: StateFilter<Target=Vec<T>>, S: Binding<F>, E: Environment {
                    fn $method_name<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static)
                        -> impl IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                        where P: IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>;
                fn $method_name_options<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static, options: <$t as FromOptions>::Options)
                        -> impl IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                        where P: IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>;
                }
            };
            (__impl_trait $t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident) => {
                fn $method_name<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static)
                    -> impl IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                    where P: IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>
                {
                    VecBindingLayout::new(self, map, <$t as FromOptions>::from_options(<$t as FromOptions>::Options::default()))
                }
                fn $method_name_options<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static, options: <$t as FromOptions>::Options)
                    -> impl IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                    where P: IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>
                {
                    VecBindingLayout::new(self, map, <$t as FromOptions>::from_options(options))
                }
            };

            ($t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident, where E: $env: path) => {
                impl_binding_layout_extension!(__declare_trait  $t, $trait_name, $method_name, $method_name_options);

                impl<E, F, T, S> $trait_name<T, F, S, E> for S where T: StoreContainer, F: StateFilter<Target=Vec<T>>, S: Binding<F>, E: $env
                {
                    impl_binding_layout_extension!(__impl_trait $t, $trait_name, $method_name, $method_name_options);
                }
            };
            ($t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident, where E = $env: ty) => {
                mod {
                    type E = $env;
                    impl_binding_layout_extension!(__declare_trait  $t, $trait_name, $method_name, $method_name_options);

                    impl<F, T, S> $trait_name<T, F, S, E> for S where T: StoreContainer, F: StateFilter<Target=Vec<T>>, S: Binding<F>
                    {
                        impl_binding_layout_extension!(__impl_trait $t, $trait_name, $method_name, $method_name_options);
                    }
                }
            }
        }
        pub use impl_binding_layout_extension;

        #[macro_export]
        macro_rules! impl_iterator_layout_extension {
            (__declare_trait $t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident) => {
                pub trait $trait_name<E> : IntoIterator where E: Environment, Self::Item: Send + 'static {
                    fn $method_name<P>(self, map: impl FnMut(&Self::Item, MSlock) -> P + 'static)
                        -> impl IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                        where P: IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>;
                    fn $method_name_options<P>(self, map: impl FnMut(&Self::Item, MSlock) -> P + 'static, options: <$t as FromOptions>::Options)
                        -> impl IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                        where P: IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>;
                }
            };
            (__impl_trait $t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident) => {
                fn $method_name<P>(self, map: impl FnMut(&Self::Item, MSlock) -> P + 'static)
                    -> impl IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                    where P: IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>
                {
                    VecSignalLayout::new(FixedSignal::new(self.into_iter().collect()), map, <$t as FromOptions>::from_options(<$t as FromOptions>::Options::default()))
                }

                fn $method_name_options<P>(self, map: impl FnMut(&Self::Item, MSlock) -> P + 'static, options: <$t as FromOptions>::Options)
                    -> impl IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                    where P: IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>
                {
                    VecSignalLayout::new(FixedSignal::new(self.into_iter().collect()), map, <$t as FromOptions>::from_options(options))
                }
            };

            ($t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident, where E: $env: path) => {
                impl_iterator_layout_extension!(__declare_trait  $t, $trait_name, $method_name, $method_name_options);

                impl<E, I> $trait_name<E> for I where E: $env, I: IntoIterator, I::Item: Send + 'static
                {
                    impl_iterator_layout_extension!(__impl_trait $t, $trait_name, $method_name, $method_name_options);
                }
            };
            ($t: ty, $trait_name: ident, $method_name: ident, $method_name_options: ident, where E = $env: ty) => {
                mod {
                    type E = $env;
                    impl_iterator_layout_extension!(__declare_trait  $t, $trait_name, $method_name, $method_name_options);

                    impl<I> $trait_name<E> for I where I: IntoIterator, I::Item: Send + 'static
                    {
                        impl_binding_layout_extension!(__impl_trait $t, $trait_name, $method_name, $method_name_options);
                    }
                }
            }
        }
    }

    // FIXME could make more organized
    mod hetero_layout {
        use std::marker::PhantomData;

        use crate::core::{Environment, MSlock};
        use crate::util::geo::{Rect, Size};
        use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, UpContextAdapter, View, ViewProvider, ViewRef, WeakInvalidator};
        use crate::view::layout::VecLayoutProvider;

        pub trait HeteroIVPNode<E, U, D> : 'static where E: Environment, U: 'static, D: 'static {
            fn into_layout(self, env: &E::Const, build: impl HeteroVPNode<E, U, D>, s: MSlock) -> impl HeteroVPNode<E, U, D>;
        }

        pub trait HeteroVPNodeBase<E, U, D>: 'static where E: Environment, U: 'static, D: 'static {
            fn next(&self) -> &dyn HeteroVPNodeBase<E, U, D>;
            fn view(&self) -> Option<&dyn ViewRef<E, UpContext=U, DownContext=D>>;
        }

        pub trait HeteroVPNode<E, U, D>: HeteroVPNodeBase<E, U, D> where E: Environment, U: 'static, D: 'static
        {
            fn push_subviews(&self, tree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock);
            fn take_backing(&mut self, from: Self, env: &mut EnvRef<E>, s: MSlock);
        }

        struct NullNode;
        impl<E: Environment, U: 'static, D: 'static> HeteroIVPNode<E, U, D> for NullNode {
            fn into_layout(self, _env: &E::Const, build: impl HeteroVPNode<E, U, D>, _s: MSlock) -> impl HeteroVPNode<E, U, D> {
                 build
            }
        }

        impl<E: Environment, U: 'static, D: 'static> HeteroVPNodeBase<E, U, D> for NullNode {
            fn next(&self) -> &dyn HeteroVPNodeBase<E, U, D> {
                self
            }

            fn view(&self) -> Option<&dyn ViewRef<E, UpContext=U, DownContext=D>> {
                None
            }
        }
        impl<E: Environment, U: 'static, D: 'static> HeteroVPNode<E, U, D> for NullNode {
            fn push_subviews(&self, _tree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) {

            }

            fn take_backing(&mut self, _from: Self, _env: &mut EnvRef<E>, _s: MSlock) {

            }
        }


        struct HeteroIVPActualNode<E, U, D, P, N>
            where E: Environment,
                  P: IntoViewProvider<E, DownContext=D>,
                  P::UpContext: Into<U>,
                  N: HeteroIVPNode<E, U, D>,
                  U: 'static, D: 'static
        {
            next: N,
            provider: P,
            phantom: PhantomData<(E, U, D)>,
        }

        impl<E, U, D, P, N> HeteroIVPNode<E, U, D> for HeteroIVPActualNode<E, U, D, P, N>
            where E: Environment,
                  P: IntoViewProvider<E, DownContext=D>,
                  P::UpContext: Into<U>,
                  N: HeteroIVPNode<E, U, D>,
                  U: 'static, D: 'static
        {
            // reverses list
            fn into_layout(self, env: &E::Const, build: impl HeteroVPNode<E, U, D>, s: MSlock) -> impl HeteroVPNode<E, U, D> {
                self.next.into_layout(
                    env,
                    HeteroVPActualNode {
                        next: build,
                        view:
                            UpContextAdapter::new(
                                self.provider.into_view_provider(env, s)
                            ).into_view(s),
                        phantom: PhantomData
                    },
                    s
                )
            }
        }

        struct HeteroVPActualNode<E, U, D, P, N>
            where E: Environment,
                  P: ViewProvider<E, DownContext=D, UpContext=U>,
                  N: HeteroVPNode<E, U, D>,
                  U: 'static, D: 'static
        {
            next: N,
            view: View<E, P>,
            phantom: PhantomData<(U, D)>,
        }

        impl<E, U, D, P, N> HeteroVPNodeBase<E, U, D> for HeteroVPActualNode<E, U, D, P, N>
            where E: Environment,
                  P: ViewProvider<E, DownContext=D, UpContext=U>,
                  N: HeteroVPNode<E, U, D>,
                  U: 'static, D: 'static
        {
            fn next(&self) -> &dyn HeteroVPNodeBase<E, U, D> {
                &self.next
            }

            fn view(&self) -> Option<&dyn ViewRef<E, UpContext=U, DownContext=D>> {
                Some(&self.view)
            }
        }

        impl<E, U, D, P, N> HeteroVPNode<E, U, D> for HeteroVPActualNode<E, U, D, P, N>
            where E: Environment,
                  P: ViewProvider<E, DownContext=D, UpContext=U>,
                  N: HeteroVPNode<E, U, D>,
                  U: 'static, D: 'static
        {
            fn push_subviews(&self, tree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
                tree.push_subview(&self.view, env, s);
                self.next.push_subviews(tree, env, s);
            }

            fn take_backing(&mut self, from: Self, env: &mut EnvRef<E>, s: MSlock) {
                self.view.take_backing(from.view, env, s);
                self.next.take_backing(from.next, env, s);
            }
        }

        pub struct HeteroIVP<E, H, L>
            where E: Environment,
                  L: VecLayoutProvider<E>,
                  H: HeteroIVPNode<E, L::SubviewUpContext, L::SubviewDownContext>
        {
            root: H,
            layout: L,
            marker: PhantomData<E>
        }

        struct HeteroVP<E, H, L>
            where E: Environment,
                  L: VecLayoutProvider<E>,
                  H: HeteroVPNode<E, L::SubviewUpContext, L::SubviewDownContext>
        {
            root: H,
            layout: L,
            marker: PhantomData<E>
        }

        #[derive(Copy)]
        struct HeteroVPIterator<'a, E, L>(&'a dyn HeteroVPNodeBase<E, L::SubviewUpContext, L::SubviewDownContext>)
            where E: Environment,
                  L: VecLayoutProvider<E>;

        impl<'a, E, L> Clone for HeteroVPIterator<'a, E, L>
            where E: Environment,
                  L: VecLayoutProvider<E>
        {
            fn clone(&self) -> Self {
                HeteroVPIterator(self.0)
            }
        }

        impl<'a, E, L> Iterator for HeteroVPIterator<'a, E, L>
            where E: Environment,
                  L: VecLayoutProvider<E>,
        {
            type Item = &'a dyn ViewRef<E, UpContext=L::SubviewUpContext, DownContext=L::SubviewDownContext>;

            fn next(&mut self) -> Option<Self::Item> {
                let view = self.0.view();
                self.0 = self.0.next();

                view
            }
        }

        pub(crate) fn new_hetero_ivp<E: Environment, L: VecLayoutProvider<E>>(layout: L) -> HeteroIVP<E, impl HeteroIVPNode<E, L::SubviewUpContext, L::SubviewDownContext>, L> {
            HeteroIVP {
                root: NullNode,
                layout,
                marker: PhantomData
            }
        }

        impl<E, H, L> HeteroIVP<E, H, L>
            where E: Environment,
                  L: VecLayoutProvider<E>,
                  H: HeteroIVPNode<E, L::SubviewUpContext, L::SubviewDownContext>
        {
            pub fn push<P>(self, provider: P) -> HeteroIVP<E, impl HeteroIVPNode<E, L::SubviewUpContext, L::SubviewDownContext>, L>
                where P: IntoViewProvider<E, DownContext=L::SubviewDownContext>,
                      P::UpContext: Into<L::SubviewUpContext>
            {
                HeteroIVP {
                    root: HeteroIVPActualNode {
                        next: self.root,
                        provider,
                        phantom: PhantomData
                    },
                    layout: self.layout,
                    marker: Default::default(),
                }
            }

            pub fn options(mut self, options: L::Options) -> Self {
                *self.layout.options() = options;
                self
            }
        }
        impl<E, H, L> IntoViewProvider<E> for HeteroIVP<E, H, L>
            where E: Environment,
                  L: VecLayoutProvider<E>,
                  H: HeteroIVPNode<E, L::SubviewUpContext, L::SubviewDownContext>
        {
            type UpContext = L::UpContext;
            type DownContext = L::DownContext;

            fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
                HeteroVP {
                    root: self.root.into_layout(env, NullNode, s),
                    layout: self.layout,
                    marker: PhantomData
                }
            }
        }

        impl<E, H, L> ViewProvider<E> for HeteroVP<E, H, L>
            where E: Environment,
                  L: VecLayoutProvider<E>,
                  H: HeteroVPNode<E, L::SubviewUpContext, L::SubviewDownContext>
        {
            type UpContext = L::UpContext;
            type DownContext = L::DownContext;

            fn intrinsic_size(&mut self, s: MSlock) -> Size {
                self.layout.intrinsic_size(s)
            }

            fn xsquished_size(&mut self, s: MSlock) -> Size {
                self.layout.xsquished_size(s)
            }

            fn xstretched_size(&mut self, s: MSlock) -> Size {
                self.layout.xstretched_size(s)
            }

            fn ysquished_size(&mut self, s: MSlock) -> Size {
                self.layout.ysquished_size(s)
            }

            fn ystretched_size(&mut self, s: MSlock) -> Size {
                self.layout.ystretched_size(s)
            }

            fn up_context(&mut self, s: MSlock) -> Self::UpContext {
                self.layout.up_context(s)
            }

            fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
                self.layout.init(invalidator, s);

                if let Some(source) = backing_source {
                    self.root.take_backing(source.1.root, env, s);
                    self.root.push_subviews(subtree, env, s);

                    source.0
                }
                else {
                    self.root.push_subviews(subtree, env, s);
                    NativeView::layout_view(s)
                }
            }

            fn layout_up(&mut self, _subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
                let iterator: HeteroVPIterator<E, L> = HeteroVPIterator(&self.root);

                self.layout.layout_up(iterator, env, s)
            }

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
                let iterator: HeteroVPIterator<E, L> = HeteroVPIterator(&self.root);

                let used = self.layout.layout_down(iterator, frame, layout_context, env, s);
                (used, used)
            }
        }
    }

    mod binding_layout {
        use std::marker::PhantomData;

        use crate::core::{Environment, MSlock};
        use crate::state::{Binding, Buffer, GroupAction, GroupBasis, StateFilter, StoreContainer, VecActionBasis, Word};
        use crate::util::geo::{Rect, Size};
        use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, UpContextAdapter, View, ViewProvider, WeakInvalidator};
        use crate::view::layout::vec_layout::into_view_provider;
        use crate::view::layout::VecLayoutProvider;

        pub struct VecBindingLayout<E, S, F, B, M, U, P, L>
            where E: Environment,
                  S: StoreContainer,
                  F: StateFilter<Target=Vec<S>>,
                  B: Binding<F>,
                  M: FnMut(&S, MSlock) -> P + 'static,
                  U: Into<L::SubviewUpContext> + 'static,
                  P: IntoViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=U
                  >,
                  L: VecLayoutProvider<E>
        {
            binding: B,
            layout: L,
            map: M,
            phantom: PhantomData<(fn(S, &U) -> P, F, E, S)>,
        }

        impl<E, S, F, B, M, U, P, L> VecBindingLayout<E, S, F, B, M, U, P, L>
            where E: Environment,
                  S: StoreContainer,
                  F: StateFilter<Target=Vec<S>>,
                  B: Binding<F>,
                  M: FnMut(&S, MSlock) -> P + 'static,
                  U: Into<L::SubviewUpContext>,
                  P: IntoViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=U
                  >,
                  L: VecLayoutProvider<E> {
            pub fn new(binding: B, map: M, layout: L) -> Self {
                VecBindingLayout {
                    binding,
                    layout,
                    map,
                    phantom: Default::default(),
                }
            }
        }

        struct VecBindingViewProvider<E, S, F, B, M, P, L>
            where E: Environment,
                  S: StoreContainer,
                  F: StateFilter<Target=Vec<S>>,
                  B: Binding<F>,
                  M: FnMut(&S, &E::Const, MSlock) -> P + 'static,
                  P: ViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=L::SubviewUpContext
                  >,
                  L: VecLayoutProvider<E>
        {
            binding: B,
            layout: L,
            map: M,
            action_buffer: Buffer<Word<VecActionBasis<Option<View<E, P>>>>>,
            subviews: Vec<View<E, P>>,
            phantom: PhantomData<(fn(S) -> P, F, E, S)>,
        }

        impl<E, S, F, B, M, U, P, L> IntoViewProvider<E> for VecBindingLayout<E, S, F, B, M, U, P, L>
            where E: Environment,
                  S: StoreContainer,
                  F: StateFilter<Target=Vec<S>>,
                  B: Binding<F>,
                  M: FnMut(&S, MSlock) -> P + 'static,
                  U: Into<L::SubviewUpContext> + 'static,
                  P: IntoViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=U
                  >,
                  L: VecLayoutProvider<E> {
            type UpContext = L::UpContext;
            type DownContext = L::DownContext;

            fn into_view_provider(mut self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
                VecBindingViewProvider {
                    binding: self.binding,
                    layout: self.layout,
                    map: move |data, env, s| {
                        UpContextAdapter::new(into_view_provider((self.map)(data, s), env, s))
                    },
                    action_buffer: Buffer::new(Word::default()),
                    subviews: vec![],
                    phantom: Default::default(),
                }
            }
        }

        impl<E, S, F, B, M, P, L> ViewProvider<E> for VecBindingViewProvider<E, S, F, B, M, P, L>
            where E: Environment,
                  S: StoreContainer,
                  F: StateFilter<Target=Vec<S>>,
                  B: Binding<F>,
                  M: FnMut(&S, &E::Const, MSlock) -> P + 'static,
                  P: ViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=L::SubviewUpContext
                  >,
                  L: VecLayoutProvider<E> {
            type UpContext = L::UpContext;
            type DownContext = L::DownContext;

            fn intrinsic_size(&mut self, s: MSlock) -> Size {
                self.layout.intrinsic_size(s)
            }

            fn xsquished_size(&mut self, s: MSlock) -> Size {
                self.layout.xsquished_size(s)
            }

            fn xstretched_size(&mut self, s: MSlock) -> Size {
                self.layout.xstretched_size(s)
            }

            fn ysquished_size(&mut self, s: MSlock) -> Size {
                self.layout.ysquished_size(s)
            }

            fn ystretched_size(&mut self, s: MSlock) -> Size {
                self.layout.ystretched_size(s)
            }

            fn up_context(&mut self, s: MSlock) -> Self::UpContext {
                self.layout.up_context(s)
            }

            fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
                self.layout.init(invalidator.clone(), s);

                // register invalidator for binding
                let buffer = self.action_buffer.downgrade();
                self.binding.action_listen(move |_, a, s| {
                    let (Some(invalidator), Some(buffer)) = (invalidator.upgrade(), buffer.upgrade()) else {
                        return false;
                    };

                    let mut build = buffer.take(s);
                    let mapped: Vec<_> = a.iter()
                        .map(|a| {
                            match a {
                                VecActionBasis::Insert(_, at) => {
                                    VecActionBasis::Insert(None, *at)
                                }
                                VecActionBasis::Remove(at) => {
                                    VecActionBasis::Remove(*at)
                                }
                                VecActionBasis::InsertMany(src, at) => {
                                    let none_vec = src.iter()
                                        .map(|_| None)
                                        .collect();
                                    VecActionBasis::InsertMany(none_vec, *at)
                                }
                                VecActionBasis::RemoveMany(range) => {
                                    VecActionBasis::RemoveMany(range.clone())
                                }
                                VecActionBasis::Swap(u, v) => {
                                    VecActionBasis::Swap(*u, *v)
                                }
                            }
                        })
                        .rev()
                        .collect();
                    let mapped_word = Word::new(mapped);
                    build.left_multiply(mapped_word);

                    buffer.replace(build, s);
                    invalidator.invalidate(s);
                    true
                }, s);

                self.subviews = self.binding.borrow(s)
                    .iter()
                    .map(|r| {
                        (self.map)(r, env.const_env(), s).into_view(s)
                    })
                    .collect();
                if let Some((native, provider)) = backing_source {
                    for (dst, src) in std::iter::zip(self.subviews.iter(), provider.subviews.into_iter()) {
                        dst.take_backing(src, env, s);
                    }

                    self.subviews.iter()
                        .for_each(|sv| subtree.push_subview(sv, env, s));

                    native
                }
                else {
                    self.subviews.iter()
                        .for_each(|sv| subtree.push_subview(sv, env, s));

                    NativeView::layout_view(s)
                }
            }

            // FIXME, specialization in the case that the IVP is send would be so nice
            // in general this feels so close yet so ugly, hopefully real world performance is ok
            fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
                let action = self.action_buffer.take(s);

                if action.len() == 0 {
                    // no op
                }
                else if action.iter()
                    .all(|a| {
                            matches!(a, VecActionBasis::Remove( _) | VecActionBasis::RemoveMany(_))
                        }) {
                    // any number of removals are easy
                    for act in action {
                        match act {
                            VecActionBasis::Remove(at) => {
                                self.subviews.remove(at);
                                subtree.remove_subview_at(at, env, s);
                            },
                            VecActionBasis::RemoveMany(range) => {
                                self.subviews.splice(range.clone(), std::iter::empty());
                                for i in range.into_iter().rev() {
                                    subtree.remove_subview_at(i, env, s);
                                }
                            },
                            _ => unreachable!()
                        }
                    }
                }
                else if action.len() == 1 &&
                    matches!(action.iter().next().unwrap(), VecActionBasis::InsertMany(_, _) | VecActionBasis::Insert(_, _)) {
                    // single increases are easy
                    for act in action {
                        let binding = self.binding.borrow(s);
                        match act {
                            VecActionBasis::InsertMany(elems, at) => {
                                // hm technically n^2? we'll have to optimize at some point
                                for i in at .. at + elems.len() {
                                    let mapped = (self.map)(&binding[i], env.const_env(), s).into_view(s);
                                    subtree.insert_subview(&mapped, i, env, s);
                                    self.subviews.insert(i, mapped);
                                }
                            },
                            VecActionBasis::Insert(_elem, at) => {
                                let mapped = (self.map)(&binding[at], env.const_env(), s).into_view(s);
                                subtree.insert_subview(&mapped, at, env, s);
                                self.subviews.insert(at, mapped);
                            },
                            _ => unreachable!()
                        }
                    }
                }
                else {
                    // multiple insertions, or mixed with perms and removals
                    // are non-trivial so we basically recalculate everything
                    // this should hopefully be a cold branch
                    subtree.clear_subviews(s);

                    let mut view_buffer: Vec<_> = std::mem::take(&mut self.subviews)
                        .into_iter()
                        .map(|x| Some(x))
                        .collect();

                    action.apply(&mut view_buffer);

                    // fill in unfilled views
                    let current = self.binding.borrow(s);
                    self.subviews = std::iter::zip(view_buffer.into_iter(), current.iter())
                        .map(|(view, src)| {
                            if let Some(view) = view {
                                view
                            } else {
                                (self.map)(src, env.const_env(), s).into_view(s)
                            }
                        })
                        .collect();

                    // add new subviews
                    self.subviews.iter().for_each(|sv| subtree.push_subview(sv, env, s));
                }

                self.layout
                    .layout_up(self.subviews.iter(), env, s)
            }

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
                let used = self.layout
                    .layout_down(self.subviews.iter(), frame, layout_context, env, s);
                (used, used)
            }
        }
    }

    mod signal_layout {
        use std::marker::PhantomData;

        use crate::core::{Environment, MSlock};
        use crate::state::Signal;
        use crate::util::geo::{Rect, Size};
        use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, UpContextAdapter, View, ViewProvider, WeakInvalidator};
        use crate::view::layout::vec_layout::into_view_provider;
        use crate::view::layout::VecLayoutProvider;

        // FIXME make a view buffer to avoid over allocating
        pub struct VecSignalLayout<E, T, S, M, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Target=Vec<T>>,
                  M: FnMut(&T, MSlock) -> P + 'static,
                  P: IntoViewProvider<E, DownContext=L::SubviewDownContext>,
                  P::UpContext: Into<L::SubviewUpContext>,
                  L: VecLayoutProvider<E>
        {
            source: S,
            map: M,
            layout: L,
            phantom: PhantomData<(T, E, P)>
        }

        impl<E, T, S, M, P, L> VecSignalLayout<E, T, S, M, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Target=Vec<T>>,
                  M: FnMut(&T, MSlock) -> P + 'static,
                  P: IntoViewProvider<E, DownContext=L::SubviewDownContext>,
                  P::UpContext: Into<L::SubviewUpContext>,
                  L: VecLayoutProvider<E>
        {
            pub fn new(source: S, map: M, layout: L) -> Self {
                VecSignalLayout {
                    source,
                    map,
                    layout,
                    phantom: Default::default(),
                }
            }
        }

        pub struct VecSignalViewProvider<E, T, S, M, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Target=Vec<T>>,
                  M: FnMut(&T, &E::Const, MSlock) -> View<E, P> + 'static,
                  P: ViewProvider<E, UpContext=L::SubviewUpContext, DownContext=L::SubviewDownContext>,
                  L: VecLayoutProvider<E>
        {
            source: S,
            map: M,
            layout: L,
            subviews: Vec<View<E, P>>,
            phantom: PhantomData<T>
        }

        impl<E, T, S, M, P, L> IntoViewProvider<E> for VecSignalLayout<E, T, S, M, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Target=Vec<T>>,
                  M: FnMut(&T, MSlock) -> P + 'static,
                  P: IntoViewProvider<E, DownContext=L::SubviewDownContext>,
                  P::UpContext: Into<L::SubviewUpContext>,
                  L: VecLayoutProvider<E>
        {
            type UpContext = L::UpContext;
            type DownContext = L::DownContext;

            fn into_view_provider(mut self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
                VecSignalViewProvider {
                    source: self.source,
                    map: move |a: &T, env: &E::Const, s: MSlock| {
                        UpContextAdapter::new(
                            into_view_provider((self.map)(a, s), env, s)
                        ).into_view(s)
                    },
                    layout: self.layout,
                    subviews: vec![],
                    phantom: Default::default(),
                }
            }
        }

        impl<E, T, S, M, P, L> ViewProvider<E> for VecSignalViewProvider<E, T, S, M, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Target=Vec<T>>,
                  M: FnMut(&T, &E::Const, MSlock) -> View<E, P> + 'static,
                  P: ViewProvider<E, UpContext=L::SubviewUpContext, DownContext=L::SubviewDownContext>,
                  L: VecLayoutProvider<E>
        {
            type UpContext = L::UpContext;
            type DownContext = L::DownContext;

            fn intrinsic_size(&mut self, s: MSlock) -> Size {
                self.layout.intrinsic_size(s)
            }

            fn xsquished_size(&mut self, s: MSlock) -> Size {
                self.layout.xsquished_size(s)
            }

            fn xstretched_size(&mut self, s: MSlock) -> Size {
                self.layout.ysquished_size(s)
            }

            fn ysquished_size(&mut self, s: MSlock) -> Size {
                self.layout.ysquished_size(s)
            }

            fn ystretched_size(&mut self, s: MSlock) -> Size {
                self.layout.ysquished_size(s)
            }

            fn up_context(&mut self, s: MSlock) -> Self::UpContext {
                self.layout.up_context(s)
            }

            fn init_backing(&mut self, invalidator: WeakInvalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
                self.layout.init(invalidator.clone(), s);

                /* register listeners and try to steal whatever backing we can */
                self.source.listen(move |_, s| {
                    let Some(invalidator)  = invalidator.upgrade() else {
                        return false;
                    };

                    invalidator.invalidate(s);

                    true
                }, s);

                if let Some((view, wrapper)) = backing_source {
                    // will be used as backing buffer on the first layout up (soon after this call)
                    self.subviews = wrapper.subviews;

                    view
                }
                else {
                    NativeView::layout_view(s)
                }
            }

            fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
                subtree.clear_subviews(s);

                /* new subviews */
                let borrow = self.source.borrow(s);
                let mut views: Vec<_> = borrow.iter()
                    .map(|x| (self.map)(x, env.const_env(), s))
                    .collect();

                /* take backings if we can */
                for (dst, src) in
                std::iter::zip(views.iter_mut(), std::mem::take(&mut self.subviews)) {
                    dst.take_backing(src, env, s);
                }

                /* mark new subviews */
                self.subviews = views;
                for view in &self.subviews {
                    subtree.push_subview(view, env, s);
                }

                self.layout.layout_up(
                    self.subviews.iter(),
                    env,
                    s
                )
            }

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
                let used = self.layout.layout_down(
                    self.subviews.iter(),
                    frame,
                    layout_context,
                    env,
                    s
                );

                (used, used)
            }
        }
    }

    mod vstack {
        use crate::core::{Environment, MSlock};
        use crate::util::FromOptions;
        use crate::util::geo::{HorizontalAlignment, Point, Rect, ScreenUnit, Size, VerticalDirection};
        use crate::view::{EnvRef, TrivialContextViewRef, ViewRef};
        use crate::view::layout::VecLayoutProvider;
        use crate::view::util::SizeContainer;

        #[derive(Default)]
        pub struct VStack(SizeContainer, VStackOptions);

        #[derive(Copy, Clone)]
        pub struct VStackOptions {
            spacing: ScreenUnit,
            alignment: HorizontalAlignment,
            direction: VerticalDirection,
            stretch: bool
        }

        impl Default for VStackOptions {
            fn default() -> Self {
                VStackOptions {
                    spacing: 10.0,
                    alignment: HorizontalAlignment::Center,
                    direction: VerticalDirection::Down,
                    stretch: true
                }
            }
        }

        impl VStackOptions {
            pub fn spacing(mut self, spacing: ScreenUnit) -> Self {
                self.spacing = spacing;
                self
            }

            pub fn align(mut self, alignment: HorizontalAlignment) -> Self {
                self.alignment = alignment;
                self
            }

            pub fn direction(mut self, direction: VerticalDirection) -> Self {
                self.direction = direction;
                self
            }

            pub fn no_stretch(mut self) -> Self {
                self.stretch = false;
                self
            }
        }

        impl FromOptions for VStack {
            type Options = VStackOptions;

            fn from_options(options: Self::Options) -> Self {
                VStack(SizeContainer::default(), options)
            }

            fn options(&mut self) -> &mut Self::Options {
                &mut self.1
            }
        }

        impl<E> VecLayoutProvider<E> for VStack where E: Environment {
            type DownContext = ();
            type UpContext = ();
            type SubviewDownContext = ();
            type SubviewUpContext = ();

            fn intrinsic_size(&mut self, _s: MSlock) -> Size {
                self.0.intrinsic()
            }

            fn xsquished_size(&mut self, _s: MSlock) -> Size {
                self.0.xsquished()
            }

            fn ysquished_size(&mut self, _s: MSlock) -> Size {
                self.0.ysquished()
            }

            fn xstretched_size(&mut self, _s: MSlock) -> Size {
                self.0.xstretched()
            }

            fn ystretched_size(&mut self, _s: MSlock) -> Size {
                self.0.ystretched()
            }

            fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
                ()
            }

            fn layout_up<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P>, _env: &mut EnvRef<E>, s: MSlock) -> bool
                where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a{
                let mut new = subviews
                    .map(|v| v.sizes(s))
                    .reduce(|mut new, curr| {
                        for i in 0..SizeContainer::num_sizes() {
                            new[i].w = new[i].w.max(curr[i].w);
                            new[i].h += curr[i].h + self.1.spacing;
                        }
                        new
                    })
                    .unwrap_or_default();

                if !self.1.stretch {
                    *new.xstretched_mut() = new.intrinsic();
                    *new.ystretched_mut() = new.intrinsic();
                }

                if new != self.0 {
                    self.0 = new;
                    true
                }
                else {
                    false
                }
            }

            fn layout_down<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P> + Clone, frame: Size, _context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> Rect
                where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a {
                // determine relative stretch factor
                let suggested_height = frame.h;
                let our_min = self.0.ysquished().h;
                let our_intrinsic = self.0.intrinsic().h;
                let our_max = self.0.ystretched().h;
                let shrink = suggested_height < our_intrinsic;
                let factor = if shrink {
                    if our_intrinsic == our_min {
                        1.0
                    }
                    else {
                        (our_intrinsic - suggested_height) / (our_intrinsic - our_min)
                    }
                }
                else if self.1.stretch {
                    if our_intrinsic == our_max {
                        1.0
                    }
                    else {
                        (suggested_height - our_intrinsic) / (our_max - our_intrinsic)
                    }
                }
                else { 0.0 };

                let mut elapsed = 0.0;
                let mut total_w: f64 = 0.0;
                let sv_clone = subviews.clone();
                let mut extra_spacing = 0.0;
                for view in subviews {
                    let intrinsic = view.intrinsic_size(s);
                    let other = if shrink {
                        view.ysquished_size(s)
                    }
                    else {
                        view.ystretched_size(s)
                    };
                    let alotted = intrinsic.h * (1.0 - factor) + factor * other.h;
                    let used = view.layout_down(Rect::new(0.0, elapsed, frame.w, alotted), env, s);
                    elapsed += used.h + self.1.spacing;
                    total_w = total_w.max(used.w);
                    extra_spacing = self.1.spacing;
                }

                let total_h = elapsed - extra_spacing;
                elapsed = match self.1.direction {
                    VerticalDirection::Down => 0.0,
                    VerticalDirection::Up => elapsed
                };
                for view in sv_clone {
                    let used = view.used_rect(s);
                    let target_x = match self.1.alignment {
                        HorizontalAlignment::Leading => 0.0,
                        HorizontalAlignment::Center => total_w / 2.0 - used.w / 2.0,
                        HorizontalAlignment::Trailing => total_w - used.w,
                    };
                    match self.1.direction {
                        VerticalDirection::Up => {
                            elapsed -= used.h + self.1.spacing;
                            view.translate_post_layout_down(Point::new(target_x - used.x, elapsed - used.y), s);
                        }
                        VerticalDirection::Down => {
                            view.translate_post_layout_down(Point::new(target_x - used.x, elapsed - used.y), s);
                            elapsed += used.h + self.1.spacing;
                        }
                    }
                }

                Rect::new(0.0, 0.0, total_w, total_h)
            }
        }
    }

    mod hstack {
        use crate::core::{Environment, MSlock};
        use crate::util::FromOptions;
        use crate::util::geo::{HorizontalDirection, Point, Rect, ScreenUnit, Size, VerticalAlignment};
        use crate::view::{EnvRef, TrivialContextViewRef, ViewRef};
        use crate::view::layout::VecLayoutProvider;
        use crate::view::util::SizeContainer;

        pub struct HStack(SizeContainer, HStackOptions);

        #[derive(Copy, Clone)]
        pub struct HStackOptions {
            spacing: ScreenUnit,
            alignment: VerticalAlignment,
            direction: HorizontalDirection,
            stretch: bool
        }

        impl Default for HStackOptions {
            fn default() -> Self {
                HStackOptions {
                    spacing: 10.0,
                    alignment: VerticalAlignment::Center,
                    direction: HorizontalDirection::Right,
                    stretch: true
                }
            }
        }

        impl HStackOptions {
            pub fn spacing(mut self, spacing: ScreenUnit) -> Self {
                self.spacing = spacing;
                self
            }

            pub fn align(mut self, alignment: VerticalAlignment) -> Self {
                self.alignment = alignment;
                self
            }

            pub fn direction(mut self, direction: HorizontalDirection) -> Self {
                self.direction = direction;
                self
            }

            pub fn no_stretch(mut self) -> Self {
                self.stretch = false;
                self
            }
        }

        impl FromOptions for HStack {
            type Options = HStackOptions;

            fn from_options(options: Self::Options) -> Self {
                HStack(SizeContainer::default(), options)
            }

            fn options(&mut self) -> &mut Self::Options {
                &mut self.1
            }
        }

        impl<E> VecLayoutProvider<E> for HStack where E: Environment {
            type DownContext = ();
            type UpContext = ();
            type SubviewDownContext = ();
            type SubviewUpContext = ();

            fn intrinsic_size(&mut self, _s: MSlock) -> Size {
                self.0.intrinsic()
            }

            fn xsquished_size(&mut self, _s: MSlock) -> Size {
                self.0.xsquished()
            }

            fn ysquished_size(&mut self, _s: MSlock) -> Size {
                self.0.ysquished()
            }

            fn xstretched_size(&mut self, _s: MSlock) -> Size {
                self.0.xstretched()
            }

            fn ystretched_size(&mut self, _s: MSlock) -> Size {
                self.0.ystretched()
            }

            fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
                ()
            }

            fn layout_up<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P> + Clone, _env: &mut EnvRef<E>, s: MSlock) -> bool
                where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a{
                let mut new = subviews
                    .map(|v| v.sizes(s))
                    .reduce(|mut new, curr| {
                        for i in 0..SizeContainer::num_sizes() {
                            new[i].h = new[i].h.max(curr[i].h);
                            new[i].w += curr[i].w + self.1.spacing;
                        }
                        new
                    })
                    .unwrap_or_default();

                if !self.1.stretch {
                    *new.xstretched_mut() = new.intrinsic();
                    *new.ystretched_mut() = new.intrinsic();
                }

                if new != self.0 {
                    self.0 = new;
                    true
                }
                else {
                    false
                }
            }

            fn layout_down<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P> + Clone, frame: Size, _context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> Rect
                where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a {
                let suggested_width = frame.w;
                let our_min = self.0.xsquished().w;
                let our_intrinsic = self.0.intrinsic().w;
                let our_max = self.0.xstretched().w;
                let shrink = suggested_width < our_intrinsic;
                let factor = if shrink {
                    if our_intrinsic == our_min {
                        1.0
                    }
                    else {
                        (our_intrinsic - suggested_width) / (our_intrinsic - our_min)
                    }
                }
                else if self.1.stretch {
                    if our_intrinsic == our_max {
                        1.0
                    }
                    else {
                        (suggested_width - our_intrinsic) / (our_max - our_intrinsic)
                    }
                }
                else { 0.0 };

                let mut elapsed = 0.0;
                let mut total_h: f64 = 0.0;
                let sv_clone = subviews.clone();
                let mut extra_spacing = 0.0;
                for view in subviews {
                    let intrinsic = view.intrinsic_size(s);
                    let other = if shrink {
                        view.xsquished_size(s)
                    }
                    else {
                        view.xstretched_size(s)
                    };
                    let alotted = intrinsic.w * (1.0 - factor) + factor * other.w;
                    let used = view.layout_down(Rect::new(elapsed, 0.0, alotted, frame.h), env, s);
                    elapsed += used.w + self.1.spacing;
                    total_h = total_h.max(used.h);
                    extra_spacing = self.1.spacing;
                }

                let total_w = elapsed - extra_spacing;
                elapsed = match self.1.direction {
                    HorizontalDirection::Left => elapsed,
                    HorizontalDirection::Right => 0.0
                };
                for view in sv_clone {
                    let used = view.used_rect(s);
                    let target_y = match self.1.alignment {
                        VerticalAlignment::Top => 0.0,
                        VerticalAlignment::Center => total_h / 2.0 - used.h / 2.0,
                        VerticalAlignment::Bottom => total_h - used.h,
                    };
                    match self.1.direction {
                        HorizontalDirection::Left => {
                            elapsed -= used.w + self.1.spacing;
                            view.translate_post_layout_down(Point::new(elapsed - used.x, target_y - used.y), s);
                        }
                        HorizontalDirection::Right => {
                            view.translate_post_layout_down(Point::new(elapsed - used.x, target_y - used.y), s);
                            elapsed += used.w + self.1.spacing;
                        }
                    }
                }

                Rect::new(0.0, 0.0, total_w, total_h)
            }
        }
    }

    mod zstack {
        use crate::core::{Environment, MSlock};
        use crate::util::FromOptions;
        use crate::util::geo::{Alignment, HorizontalAlignment, Point, Rect, Size, VerticalAlignment};
        use crate::view::{EnvRef, TrivialContextViewRef, ViewRef};
        use crate::view::layout::VecLayoutProvider;
        use crate::view::util::SizeContainer;

        pub struct ZStack(SizeContainer, ZStackOptions);

        #[derive(Default, Copy, Clone)]
        pub struct ZStackOptions {
            alignment: Alignment
        }

        impl FromOptions for ZStack {
            type Options = ZStackOptions;

            fn from_options(options: Self::Options) -> Self {
                ZStack(SizeContainer::default(), options)
            }

            fn options(&mut self) -> &mut Self::Options {
                &mut self.1
            }
        }

        impl<E> VecLayoutProvider<E> for ZStack where E: Environment {
            type DownContext = ();
            type UpContext = ();
            type SubviewDownContext = ();
            type SubviewUpContext = ();

            fn intrinsic_size(&mut self, _s: MSlock) -> Size {
                self.0.intrinsic()
            }

            fn xsquished_size(&mut self, _s: MSlock) -> Size {
                self.0.xsquished()
            }

            fn ysquished_size(&mut self, _s: MSlock) -> Size {
                self.0.ysquished()
            }

            fn xstretched_size(&mut self, _s: MSlock) -> Size {
                self.0.xstretched()
            }

            fn ystretched_size(&mut self, _s: MSlock) -> Size {
                self.0.ystretched()
            }

            fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
                ()
            }

            fn layout_up<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P> + Clone, _env: &mut EnvRef<E>, s: MSlock) -> bool where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a {
                let mut new = SizeContainer::default();
                for subview in subviews {
                    let sizes = subview.sizes(s);
                    for i in 0 .. SizeContainer::num_sizes() {
                        new[i].w = new[i].w.max(sizes[i].w);
                        new[i].h = new[i].h.max(sizes[i].w);
                    }
                }

                if new != self.0 {
                    self.0 = new;
                    true
                }
                else {
                    false
                }
            }

            fn layout_down<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P> + Clone, frame: Size, _context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> Rect where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a {
                let mut used = Size::default();
                let sv_clone = subviews.clone();
                for subview in subviews {
                    let subused = subview.layout_down(frame.full_rect(), env, s);
                    used.w = used.w.max(subused.w);
                    used.h = used.h.max(subused.h);
                }

                for subview in sv_clone {
                    let subused = subview.used_rect(s);
                    let x = match self.1.alignment.horizontal() {
                        HorizontalAlignment::Leading => {
                            0.0
                        }
                        HorizontalAlignment::Center => {
                            used.w / 2.0 - subused.w / 2.0
                        }
                        HorizontalAlignment::Trailing => {
                            used.w - subused.w
                        }
                    };
                    let y = match self.1.alignment.vertical() {
                        VerticalAlignment::Top => {
                            0.0
                        }
                        VerticalAlignment::Center => {
                            used.h / 2.0 - subused.h / 2.0
                        }
                        VerticalAlignment::Bottom => {
                            used.h - subused.h
                        }
                    };
                    subview.translate_post_layout_down(Point::new(x - subused.x, y - subused.y), s);
                }

                used.full_rect()
            }
        }
    }

    mod flex {
        use crate::core::{Environment, MSlock};
        use crate::util::{FromOptions, geo};
        use crate::util::geo::{Direction, Point, Rect, ScreenUnit, Size};
        use crate::view::{EnvRef, IntoViewProvider, TrivialContextViewRef, UpContextSetter, ViewRef};
        use crate::view::layout::VecLayoutProvider;
        use crate::view::util::SizeContainer;

        pub struct FlexStack(SizeContainer, FlexStackOptions);
        pub struct FlexStackOptions {
            direction: Direction,
            align: FlexAlign,
            justify: FlexJustify,
            gap: ScreenUnit,
            cross_gap: ScreenUnit,
            wrap: bool
        }

        impl Default for FlexStackOptions {
            fn default() -> Self {
                FlexStackOptions {
                    direction: Direction::Right,
                    align: FlexAlign::Center,
                    justify: FlexJustify::Center,
                    gap: 10.0,
                    cross_gap: 0.0,
                    wrap: false,
                }
            }
        }

        impl FlexStackOptions {
            pub fn direction(mut self, direction: Direction) -> Self {
                self.direction = direction;
                self
            }

            pub fn gap(mut self, gap: ScreenUnit) -> Self {
                self.gap = gap;
                self
            }

            pub fn cross_gap(mut self, cross_gap: ScreenUnit) -> Self {
                self.cross_gap = cross_gap;
                self
            }

            pub fn align(mut self, align: FlexAlign) -> Self {
                self.align = align;
                self
            }

            pub fn justify(mut self, justify: FlexJustify) -> Self {
                self.justify = justify;
                self
            }

            pub fn wrap(mut self) -> Self {
                self.wrap = true;
                self
            }
        }

        impl FromOptions for FlexStack {
            type Options = FlexStackOptions;

            fn from_options(options: Self::Options) -> Self {
                FlexStack(SizeContainer::default(), options)
            }

            fn options(&mut self) -> &mut Self::Options {
                &mut self.1
            }
        }

        #[derive(Copy, Clone)]
        pub enum FlexJustify {
            Start,
            Center,
            End
        }

        #[derive(Copy, Clone, Debug)]
        pub enum FlexAlign {
            Start,
            Center,
            Stretch,
            End,
        }

        #[derive(Clone, Copy, Debug)]
        pub struct FlexContext {
            grow: f64,
            shrink: f64,
            align_self: Option<FlexAlign>,
        }

        impl Default for FlexContext {
            fn default() -> Self {
                FlexContext {
                    grow: 0.0,
                    shrink: 1.0,
                    align_self: None,
                }
            }
        }

        impl FlexContext {
            pub fn grow(mut self, grow: f64) -> Self {
                self.grow = grow;
                self
            }

            pub fn shrink(mut self, shrink: f64) -> Self {
                self.shrink = shrink;
                self
            }

            pub fn align(mut self, align: FlexAlign) -> Self {
                self.align_self = Some(align);
                self
            }
        }

        impl From<()> for FlexContext {
            fn from(_value: ()) -> Self {
                Self::default()
            }
        }

        pub trait FlexSubview<E>: IntoViewProvider<E> where E: Environment {
            fn flex(self, f: FlexContext)
                -> impl IntoViewProvider<E, DownContext=Self::DownContext, UpContext=FlexContext>;
        }

        impl<E, I> FlexSubview<E> for I where E: Environment, I: IntoViewProvider<E> {
            fn flex(self, f: FlexContext)
                    -> impl IntoViewProvider<E, DownContext=Self::DownContext, UpContext=FlexContext>
            {
                UpContextSetter::new(self, f)
            }
        }

        impl FlexStack {
            fn provider_squished<E: Environment>(
                &self,
                provider: &(impl ViewRef<E> + ?Sized),
                up: &FlexContext,
                is_horizontal: bool,
                s: MSlock
            ) -> ScreenUnit {
                if up.shrink == 0.0 {
                    self.provider_intrinsic(provider, is_horizontal, s)
                }
                else if is_horizontal {
                    let size = provider.xsquished_size(s);
                    size.w
                }
                else {
                    let size = provider.ysquished_size(s);
                    size.h
                }
            }

            fn provider_intrinsic<E: Environment>(
                &self,
                provider: &(impl ViewRef<E> + ?Sized),
                is_horizontal: bool,
                s: MSlock
            ) -> ScreenUnit {
                let size = provider.intrinsic_size(s);
                if is_horizontal {
                    size.w
                }
                else {
                    size.h
                }
            }

            fn provider_stretched<E: Environment>(
                &self,
                provider: &(impl ViewRef<E> + ?Sized),
                up: &FlexContext,
                is_horizontal: bool,
                s: MSlock
            ) -> ScreenUnit {
                if up.grow == 0.0 {
                    self.provider_intrinsic(provider, is_horizontal, s)
                }
                else if is_horizontal {
                    let size = provider.xstretched_size(s);
                    size.w
                }
                else {
                    let size = provider.ystretched_size(s);
                    size.h
                }
            }

            fn layout<'a, E, P>(
                &mut self,
                subviews: impl Iterator<Item=&'a P> + Clone,
                frame: Size,
                env: &mut EnvRef<E>,
                is_down: bool,
                allow_resizing: bool,
                s: MSlock
            ) -> Rect where P: ViewRef<E, DownContext=(), UpContext=FlexContext> + ?Sized + 'a, E: Environment {
                let horizontal = matches!(self.1.direction, Direction::Left | Direction::Right);

                // keep placing nodes until intrinsic overflows
                // if we can wrap, then wrap. Otherwise, we'll have to shrink as much as possible
                let mut spans = vec![];
                let mut iter = subviews;
                let mut largest_span_main_axis: ScreenUnit = 0.0;
                let mut span_main_axis = -self.1.gap;
                let alotted_main_axis = if horizontal { frame.w } else { frame.h };

                let mut span_cross_axis = 0.0;
                let mut cross_axis = -self.1.cross_gap;

                let mut follower = iter.clone();
                while let Some(curr) = iter.next() {
                    // try extending
                    let this_intrinsic = curr.intrinsic_size(s);
                    let this_main_axis = if horizontal { this_intrinsic.w } else { this_intrinsic.h };
                    let this_cross_axis = if horizontal { this_intrinsic.h } else { this_intrinsic.w };

                    let fits = this_main_axis + self.1.gap + span_main_axis <= alotted_main_axis;
                    if spans.is_empty() || (!fits && self.1.wrap) {
                        // finish previous
                        cross_axis += self.1.cross_gap + span_cross_axis;

                        // start new
                        spans.push((follower.clone(), 1usize));

                        span_main_axis = this_main_axis;
                        span_cross_axis = this_cross_axis;
                    } else {
                        // extend previous by 1
                        spans.last_mut().unwrap().1 += 1;

                        span_main_axis += self.1.gap + this_main_axis;
                        span_cross_axis = span_cross_axis.max(this_cross_axis);
                    }

                    largest_span_main_axis = largest_span_main_axis.max(span_main_axis);
                    follower.next();
                }
                // finish last
                cross_axis += span_cross_axis;
                cross_axis = cross_axis.max(0.0);

                // finalize exact main axis size based off of growth
                let mut finalized_main_axis: ScreenUnit = 0.0;
                for (start, count) in &spans {
                    let mut span_min_axis = -self.1.gap;
                    let mut span_max_axis = -self.1.gap;

                    for sv in start.clone().take(*count) {
                        let mut up = sv.up_context(s);
                        if !allow_resizing {
                            up.shrink = 0.0;
                            up.grow = 0.0;
                        }
                        let squished_main = self.provider_squished(sv, &up, horizontal, s);
                        let stretched_main = self.provider_stretched(sv, &up, horizontal, s);
                        span_min_axis += self.1.gap + squished_main;
                        span_max_axis += self.1.gap + stretched_main;
                    }

                    finalized_main_axis = finalized_main_axis
                        .max(alotted_main_axis.clamp(span_min_axis, span_max_axis));
                }

                // perform final layout
                if is_down {
                    self.actually_layout_down(env, s, horizontal, &mut spans, alotted_main_axis, finalized_main_axis, cross_axis);
                }

                if horizontal {
                    Rect::new(0.0, 0.0, finalized_main_axis, cross_axis)
                }
                else {
                    Rect::new(0.0, 0.0, cross_axis, finalized_main_axis)
                }
            }

            fn actually_layout_down<'a, E, P>(
                &mut self, env: &mut EnvRef<E>,
                s: MSlock,
                horizontal: bool,
                spans: &Vec<(impl Iterator<Item=&'a P> + Clone + Sized, usize)>,
                alotted_main_axis: ScreenUnit,
                finalized_main_axis: f64,
                finalized_cross_axis: f64
            )
                where P: ViewRef<E, DownContext=(), UpContext=FlexContext> + ?Sized + 'a, E: Environment
            {
                let (main_pos, mut cross_pos) = match self.1.direction {
                    Direction::Left => {
                        (finalized_main_axis, 0.0)
                    }
                    Direction::Right => {
                        (0.0, 0.0)
                    }
                    Direction::Up => {
                        (finalized_cross_axis, 0.0)
                    }
                    Direction::Down => {
                        (0.0, 0.0)
                    }
                };

                for (start, count) in spans.iter() {
                    // maybe we cache this?
                    let mut span_main_axis = -self.1.gap;
                    for sv in start.clone().take(*count) {
                        span_main_axis += self.provider_intrinsic(sv, horizontal, s) + self.1.gap;
                    }

                    let is_shrink = span_main_axis > alotted_main_axis;

                    // in words: the ones that "run out of room" first will be handled first
                    let mut total_factor = 0.0;
                    let mut max_cross_axis: ScreenUnit = 0.0;
                    let mut ordered: Vec<_> = start.clone()
                        .take(*count)
                        .map(|p| {
                            let up = p.up_context(s);
                            max_cross_axis = max_cross_axis.max(self.provider_intrinsic(p, !horizontal, s));
                            if is_shrink && up.shrink == 0.0 || !is_shrink && up.grow == 0.0 {
                                return (p, -1.0);
                            }

                            let intr = self.provider_intrinsic(p, horizontal, s);

                            let (other, factor) = if is_shrink {
                                (self.provider_squished(p, &up, horizontal, s), up.shrink)
                            } else {
                                (self.provider_stretched(p, &up, horizontal, s), up.grow)
                            };
                            total_factor += factor;
                            let diff = (intr - other).abs();
                            (p, diff / factor)
                        })
                        .collect::<Vec<_>>();

                    ordered.sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                    let mut remaining = (finalized_main_axis - span_main_axis).abs();
                    for sv in ordered {
                        // doesnt want to resize at all
                        if sv.1 <= 0.0 {
                            let size = sv.0.intrinsic_size(s);
                            sv.0.layout_down(Rect::new(0.0, 0.0, size.w, size.h), env, s);
                            continue;
                        }

                        let up = sv.0.up_context(s);
                        let (other, factor) = if is_shrink {
                            (self.provider_squished(sv.0, &up, horizontal, s), up.shrink)
                        } else {
                            (self.provider_stretched(sv.0, &up, horizontal, s), up.grow)
                        };
                        let intrinsic = self.provider_intrinsic(sv.0, horizontal, s);
                        let diff = (intrinsic - other).abs();
                        let delta = (remaining * factor / total_factor).min(diff);
                        let final_main = intrinsic + if is_shrink { -delta } else { delta };

                        let child_align = up.align_self.unwrap_or(self.1.align);
                        let cross = if matches!(child_align, FlexAlign::Stretch) {
                            max_cross_axis
                        } else {
                            self.provider_intrinsic(sv.0, !horizontal, s)
                        };

                        let align_offset = match child_align {
                            FlexAlign::Start | FlexAlign::Stretch => 0.0,
                            FlexAlign::Center => {
                                (max_cross_axis - cross) / 2.0
                            }
                            FlexAlign::End => {
                                max_cross_axis - cross
                            }
                        };

                        match self.1.direction {
                            Direction::Left => {
                                sv.0.layout_down(Rect::new(0.0, align_offset, final_main, cross), env, s);
                            }
                            Direction::Right => {
                                sv.0.layout_down(Rect::new(0.0, align_offset, final_main, cross), env, s);
                            }
                            Direction::Up => {
                                sv.0.layout_down(Rect::new(align_offset, 0.0, cross, final_main), env, s);
                            }
                            Direction::Down => {
                                sv.0.layout_down(Rect::new(align_offset, 0.0, cross, final_main), env, s);
                            }
                        }

                        remaining -= delta;
                        total_factor -= factor;
                    }

                    // in this case, we use the suggested rect, rather than the used rect like
                    // most other layouts. This generally improves layout flow
                    let mut span_main_pos = main_pos;
                    for sv in start.clone().take(*count) {
                        let suggested = sv.suggested_rect(s);
                        let (pos_x, pos_y) = match self.1.direction {
                            Direction::Left => {
                                let ret = (span_main_pos - suggested.w, cross_pos);
                                span_main_pos -= self.1.gap + suggested.w;
                                ret
                            }
                            Direction::Right => {
                                let ret = (span_main_pos, cross_pos);
                                span_main_pos += self.1.gap + suggested.w;
                                ret
                            }
                            Direction::Up => {
                                let ret = (cross_pos, span_main_pos - suggested.h);
                                span_main_pos -= suggested.h + self.1.gap;
                                ret
                            }
                            Direction::Down => {
                                let ret = (cross_pos, span_main_pos);
                                span_main_pos += suggested.h + self.1.gap;
                                ret
                            }
                        };

                        sv.translate_post_layout_down(Point::new(pos_x, pos_y), s);
                    }

                    let adjusted_span_pos = match self.1.direction {
                        Direction::Left | Direction::Up => span_main_pos + self.1.gap,
                        Direction::Right | Direction::Down=> span_main_pos - self.1.gap
                    };

                    let justification_delta = match self.1.justify {
                        FlexJustify::Start => 0.0,
                        FlexJustify::Center => finalized_main_axis / 2.0 - (adjusted_span_pos + main_pos) / 2.0,
                        FlexJustify::End => (finalized_main_axis - main_pos) - adjusted_span_pos,
                    };

                    if horizontal {
                        for sv in start.clone().take(*count) {
                            sv.translate_post_layout_down(Point::new(justification_delta, 0.0), s);
                        }
                    }
                    else {
                        for sv in start.clone().take(*count) {
                            sv.translate_post_layout_down(Point::new(0.0, justification_delta), s);
                        }
                    }
                    cross_pos += max_cross_axis + self.1.cross_gap;
                }
            }
        }

        impl<E> VecLayoutProvider<E> for FlexStack where E: Environment {
            type DownContext = ();
            type UpContext = ();
            type SubviewDownContext = ();
            type SubviewUpContext = FlexContext;

            fn intrinsic_size(&mut self, _s: MSlock) -> Size {
                self.0.intrinsic()
            }

            fn xsquished_size(&mut self, _s: MSlock) -> Size {
                self.0.xsquished()
            }

            fn ysquished_size(&mut self, _s: MSlock) -> Size {
                self.0.ysquished()
            }

            fn xstretched_size(&mut self, _s: MSlock) -> Size {
                self.0.xstretched()
            }

            fn ystretched_size(&mut self, _s: MSlock) -> Size {
                self.0.ystretched()
            }

            fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
                ()
            }

            fn layout_up<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P> + Clone, env: &mut EnvRef<E>, s: MSlock) -> bool where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a {
                let sv1 = subviews.clone();
                let sv2 = subviews.clone();
                let sv3 = subviews.clone();
                let sv4 = subviews.clone();
                let new = SizeContainer::new(
                    self.layout(subviews, Size::new(geo::UNBOUNDED, geo::UNBOUNDED), env, false, false, s).size(),
                    self.layout(sv1, Size::new(0.0, geo::UNBOUNDED), env, false, true, s).size(),
                    self.layout(sv2, Size::new(geo::UNBOUNDED, 0.0), env, false, true, s).size(),
                    self.layout(sv3, Size::new(geo::UNBOUNDED, 0.0), env, false, true, s).size(),
                    self.layout(sv4, Size::new(0.0, geo::UNBOUNDED), env, false, true, s).size(),
                );

                if new != self.0 {
                    self.0 = new;
                    true
                }
                else {
                    false
                }
            }

            fn layout_down<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P> + Clone, frame: Size, _context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> Rect where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a {
                self.layout(subviews, frame, env, true, true, s)
            }
        }
    }

    mod impls {
        use crate::{impl_binding_layout_extension, impl_hetero_layout, impl_iterator_layout_extension, impl_signal_layout_extension};
        use crate::core::Environment;
        use crate::core::MSlock;
        use crate::state::{Binding, FixedSignal, Signal, StateFilter, StoreContainer};
        use crate::util::FromOptions;
        use crate::view::IntoViewProvider;
        use crate::view::layout::{FlexStack, HStack, VecBindingLayout, VecLayoutProvider, VecSignalLayout, VStack, ZStack};

        impl_signal_layout_extension!(VStack, SignalVMap, sig_vmap, sig_vmap_options, where E: Environment);
        impl_binding_layout_extension!(VStack, BindingVMap, binding_vmap, binding_vmap_options, where E: Environment);
        impl_iterator_layout_extension!(VStack, IteratorVMap, vmap, vmap_options, where E: Environment);

        impl_signal_layout_extension!(HStack, SignalHMap, sig_hmap, sig_hmap_options, where E: Environment);
        impl_binding_layout_extension!(HStack, BindingHMap, binding_hmap, binding_hmap_options, where E: Environment);
        impl_iterator_layout_extension!(HStack, IteratorHMap, hmap, hmap_options, where E: Environment);

        impl_signal_layout_extension!(ZStack, SignalZMap, sig_zmap, sig_zmap_options, where E: Environment);
        impl_binding_layout_extension!(ZStack, BindingZMap, binding_zmap, binding_zmap_options, where E: Environment);
        impl_iterator_layout_extension!(ZStack, IteratorZMap, zmap, zmap_options, where E: Environment);

        impl_signal_layout_extension!(FlexStack, SignalFlexMap, sig_flexmap, sig_flexmap_options, where E: Environment);
        impl_binding_layout_extension!(FlexStack, BindingFlexMap, binding_flexmap, binding_flexmap_options, where E: Environment);
        impl_iterator_layout_extension!(FlexStack, IteratorFlexMap, flexmap, flexmap_options, where E: Environment);

        impl_hetero_layout!(VStack, vstack);
        pub use vstack;

        impl_hetero_layout!(HStack, hstack);
        pub use hstack;

        impl_hetero_layout!(ZStack, zstack);
        pub use zstack;

        impl_hetero_layout!(FlexStack, flexstack);
        pub use flexstack;
    }
}
