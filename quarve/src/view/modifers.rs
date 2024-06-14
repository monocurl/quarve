mod provider_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::util::geo::{AlignedOriginRect, Rect, ScreenUnit, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};

    pub trait ProviderModifier<E, U, D>: Sized + 'static
        where E: Environment, U: 'static, D: 'static {
        fn modify_ivp<Q>(self, wrapping: Q) -> impl IntoViewProvider<E, UpContext=U, DownContext=D>
            where Q: IntoViewProvider<E, UpContext=U, DownContext=D>
        {
            ProviderModifierIVP {
                provider: wrapping,
                modifier: self,
                phantom: PhantomData,
            }
        }

        fn intrinsic_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            src.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            src.xsquished_size(s)
        }

        fn xstretched_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            src.ysquished_size(s)
        }

        fn ysquished_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            src.ysquished_size(s)
        }

        fn ystretched_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            src.ystretched_size(s)
        }

        fn up_context(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> U {
            src.up_context(s)
        }

        fn init_backing<P>(&mut self, src: &mut P, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self, P)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView
            where P: ViewProvider<E, UpContext=U, DownContext=D>
        {
            src.init_backing(invalidator, subtree, backing_source.map(|bs| (bs.0, bs.2)), env, s)
        }

        fn layout_up(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            src.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            src.layout_down(subtree, frame, layout_context, env, s)
        }

        fn pre_show(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) {
            src.pre_show(s)
        }

        fn post_show(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) {
            src.post_show(s)
        }

        fn pre_hide(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) {
            src.pre_hide(s)
        }

        fn post_hide(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) {
            src.post_hide(s)
        }

        fn focused(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) {
            src.focused(s)
        }

        fn unfocused(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) {
            src.unfocused(s)
        }

        fn push_environment(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, env: &mut E::Variable, s: MSlock) {
            src.push_environment(env, s);
        }

        fn pop_environment(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, env: &mut E::Variable, s: MSlock) {
            src.pop_environment(env, s);
        }

        fn handle_event(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, e: Event, s: MSlock) -> EventResult {
            src.handle_event(e, s)
        }
    }

    struct ProviderModifierIVP<E, P, M>
        where E: Environment,
              P: IntoViewProvider<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        provider: P,
        modifier: M,
        phantom: PhantomData<E>
    }

    impl<E, P, M> IntoViewProvider<E> for ProviderModifierIVP<E, P, M>
        where E: Environment,
              P: IntoViewProvider<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            ProviderModifierVP {
                provider: self.provider.into_view_provider(env, s),
                modifier: self.modifier,
                phantom: PhantomData
            }
        }
    }

    struct ProviderModifierVP<E, P, M>
        where E: Environment,
              P: ViewProvider<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        provider: P,
        modifier: M,
        phantom: PhantomData<E>
    }

    impl<E, P, M> ViewProvider<E> for ProviderModifierVP<E, P, M>
        where E: Environment,
              P: ViewProvider<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.modifier.intrinsic_size(&mut self.provider, s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.modifier.xsquished_size(&mut self.provider, s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.modifier.xstretched_size(&mut self.provider, s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.modifier.ysquished_size(&mut self.provider, s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.modifier.ystretched_size(&mut self.provider, s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.provider.up_context(s)
        }

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.modifier.init_backing(&mut self.provider, invalidator, subtree, backing_source.map(|bs| (bs.0, bs.1.modifier, bs.1.provider)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            self.modifier.layout_up(&mut self.provider, subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.modifier.layout_down(&mut self.provider, subtree, frame, layout_context, env, s)
        }

        fn pre_show(&mut self, s: MSlock) {
            self.modifier.pre_show(&mut self.provider, s)
        }

        fn post_show(&mut self, s: MSlock) {
            self.modifier.post_show(&mut self.provider, s)
        }

        fn pre_hide(&mut self, s: MSlock) {
            self.modifier.pre_hide(&mut self.provider, s)
        }

        fn post_hide(&mut self, s: MSlock) {
            self.modifier.post_hide(&mut self.provider, s)
        }

        fn focused(&mut self, s: MSlock) {
            self.modifier.focused(&mut self.provider, s)
        }

        fn unfocused(&mut self, s: MSlock) {
            self.modifier.unfocused(&mut self.provider, s)
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.modifier.push_environment(&mut self.provider, env, s)
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.modifier.pop_environment(&mut self.provider, env, s)
        }

        fn handle_event(&mut self, e: Event, s: MSlock) -> EventResult {
            self.modifier.handle_event(&mut self.provider, e, s)
        }
    }

    struct Offset {
        dx: ScreenUnit,
        dy: ScreenUnit
    }

    impl<E, U, D> ProviderModifier<E, U, D> for Offset
        where E: Environment, U: 'static, D: 'static
    {
        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let (mut frame, exclusion) = src.layout_down(subtree, frame, layout_context, env, s);
            frame.x += self.dx;
            frame.y += self.dy;

            (frame, exclusion)
        }
    }


    pub trait OffsetModifiable<E> where E: Environment {
        type UpContext: 'static;
        type DownContext: 'static;

        fn offset(self, dx: ScreenUnit, dy: ScreenUnit) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
    }

    impl<E, I> OffsetModifiable<E> for I
        where E: Environment, I: IntoViewProvider<E>
    {
        type UpContext = I::UpContext;
        type DownContext = I::DownContext;

        fn offset(self, dx: ScreenUnit, dy: ScreenUnit) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            let o = Offset {
                dx,
                dy,
            };

            o.modify_ivp(self)
        }
    }
}
pub use provider_modifier::*;

mod backing_modifer {
    use crate::util::geo::ScreenUnit;
    use crate::view::util::Color;

    // allocates a new backing
    pub trait BackingModifier {}

    pub struct Layer {
        background_color: Color,
        corner_radius: ScreenUnit,
        border_color: Color,
        border_width: ScreenUnit,
    }


}
pub use backing_modifer::*;