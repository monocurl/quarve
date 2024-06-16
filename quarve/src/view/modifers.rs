use crate::core::{Environment, MSlock};
use crate::view::{IntoViewProvider, ViewProvider};

// FIXME perhaps could do some better software architecture here

// Conceptually, whenever the view provider is disabled
// the scene should behave the same as if only a Modifying was in its place
pub trait ConditionalIVPModifier<E>:
    IntoViewProvider<E,
        UpContext=<Self::Modifying as IntoViewProvider<E>>::UpContext,
        DownContext=<Self::Modifying as IntoViewProvider<E>>::DownContext> where E: Environment {
    type Modifying: IntoViewProvider<E>;

    fn into_conditional_view_provider(self, e: &E::Const, s: MSlock)
        -> impl ConditionalVPModifier<E,
            UpContext=<Self::Modifying as IntoViewProvider<E>>::UpContext,
            DownContext=<Self::Modifying as IntoViewProvider<E>>::DownContext>;
}

// dont like how the syntax of the two is midly different
// (mainly because we don't need modifying once it reaches the vp stage)
pub trait ConditionalVPModifier<E>: ViewProvider<E> where E: Environment
{
    // enable and disabled calls must be called exactly before
    // the underlying layout up call of the view provider
    fn enable(&mut self, s: MSlock);
    fn disable(&mut self, s: MSlock);
}

mod identity_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::util::geo::{AlignedOriginRect, Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    pub struct UnmodifiedIVP<E, P> where E: Environment, P: IntoViewProvider<E> {
        source: P,
        phantom: PhantomData<E>
    }

    struct UnmodifiedVP<E, P> where E: Environment, P: ViewProvider<E> {
        source: P,
        phantom: PhantomData<E>
    }

    impl<E, P> IntoViewProvider<E> for UnmodifiedIVP<E, P> where E: Environment, P: IntoViewProvider<E> {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            UnmodifiedVP {
                source: self.source.into_view_provider(env, s),
                phantom: PhantomData
            }
        }
    }

    impl<E, P> ConditionalIVPModifier<E> for UnmodifiedIVP<E, P> where E: Environment, P: IntoViewProvider<E> {
        type Modifying = P;

        fn into_conditional_view_provider(self, e: &E::Const, s: MSlock) -> impl ConditionalVPModifier<E, UpContext=<Self::Modifying as IntoViewProvider<E>>::UpContext, DownContext=<Self::Modifying as IntoViewProvider<E>>::DownContext> {
            UnmodifiedVP {
                source: self.source.into_view_provider(e, s),
                phantom: PhantomData,
            }
        }
    }

    impl<E, P> ConditionalVPModifier<E> for UnmodifiedVP<E, P> where E: Environment, P: ViewProvider<E> {
        fn enable(&mut self, _s: MSlock) {
            /* no op */
        }

        fn disable(&mut self, _s: MSlock) {
            /* no op */
        }
    }

    impl<E, P> ViewProvider<E> for UnmodifiedVP<E, P>
        where E: Environment, P: ViewProvider<E>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.source.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.source.xsquished_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.source.xstretched_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.source.ysquished_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.source.ysquished_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.source.up_context(s)
        }

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.source.init_backing(invalidator, subtree, backing_source.map(|(nv, this)| (nv, this.source)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            self.source.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.source.layout_down(subtree, frame, layout_context, env, s)
        }

        fn pre_show(&mut self, s: MSlock) {
            self.source.pre_show(s)
        }

        fn post_show(&mut self, s: MSlock) {
            self.source.post_show(s)
        }

        fn pre_hide(&mut self, s: MSlock) {
            self.source.pre_hide(s)
        }

        fn post_hide(&mut self, s: MSlock) {
            self.source.post_hide(s)
        }

        fn focused(&mut self, s: MSlock) {
            self.source.focused(s)
        }

        fn unfocused(&mut self, s: MSlock) {
            self.source.unfocused(s)
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.push_environment(env, s)
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.pop_environment(env, s)
        }

        fn handle_event(&mut self, e: Event, s: MSlock) -> EventResult {
            self.source.handle_event(e, s)
        }
    }
}

mod provider_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::util::geo::{AlignedOriginRect, Rect, ScreenUnit, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    pub trait ProviderModifier<E, U, D>: Sized + 'static
        where E: Environment, U: 'static, D: 'static {
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

        fn layout_up(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            src.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            src.layout_down(subtree, frame, layout_context, env, s)
        }

        #[allow(unused_variables)]
        fn focused(&mut self, s: MSlock)  {

        }

        #[allow(unused_variables)]
        fn unfocused(&mut self, s: MSlock)  {

        }

        #[allow(unused_variables)]
        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {

        }

        #[allow(unused_variables)]
        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {

        }
    }

    pub struct ProviderIVPModifier<E, P, M>
        where E: Environment,
              P: IntoViewProvider<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        provider: P,
        modifier: M,
        phantom: PhantomData<E>
    }

    impl<E, P, M> ProviderIVPModifier<E, P, M>
        where E: Environment,
              P: IntoViewProvider<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        pub fn new(provider: P, modifier: M) -> Self {
            ProviderIVPModifier {
                provider,
                modifier,
                phantom: PhantomData
            }
        }
    }

    struct ProviderVPModifier<E, P, M>
        where E: Environment,
              P: ViewProvider<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        provider: P,
        modifier: M,
        enabled: bool,
        last_env_push_was_enabled: bool,
        phantom: PhantomData<E>
    }

    impl<E, P, M> IntoViewProvider<E> for ProviderIVPModifier<E, P, M>
        where E: Environment,
              P: IntoViewProvider<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            ProviderVPModifier {
                provider: self.provider.into_view_provider(env, s),
                modifier: self.modifier,
                enabled: true,
                last_env_push_was_enabled: true,
                phantom: PhantomData
            }
        }
    }

    impl<E, P, M> ViewProvider<E> for ProviderVPModifier<E, P, M>
        where E: Environment,
              P: ViewProvider<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            if self.enabled {
                self.modifier.intrinsic_size(&mut self.provider, s)
            } else {
                self.provider.intrinsic_size(s)
            }
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            if self.enabled {
                self.modifier.xsquished_size(&mut self.provider, s)
            } else {
                self.provider.xsquished_size(s)
            }
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            if self.enabled {
                self.modifier.xstretched_size(&mut self.provider, s)
            } else {
                self.provider.xstretched_size(s)
            }
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            if self.enabled {
                self.modifier.ysquished_size(&mut self.provider, s)
            } else {
                self.provider.ysquished_size(s)
            }
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            if self.enabled {
                self.modifier.ystretched_size(&mut self.provider, s)
            } else {
                self.provider.ystretched_size(s)
            }
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.provider.up_context(s)
        }

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.provider.init_backing(invalidator, subtree, backing_source.map(|(nv, bs)| (nv, bs.provider)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.enabled {
                self.modifier.layout_up(&mut self.provider, subtree, env, s)
            } else {
                self.provider.layout_up(subtree, env, s)
            }
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            if self.enabled {
                self.modifier.layout_down(&mut self.provider, subtree, frame, layout_context, env, s)
            } else {
                self.provider.layout_down(subtree, frame, layout_context, env, s)
            }
        }

        fn pre_show(&mut self, s: MSlock) {
            self.provider.pre_show(s)
        }

        fn post_show(&mut self, s: MSlock) {
            self.provider.post_show(s)
        }

        fn pre_hide(&mut self, s: MSlock) {
            self.provider.pre_hide(s)
        }

        fn post_hide(&mut self, s: MSlock) {
            self.provider.post_hide(s)
        }

        fn focused(&mut self, s: MSlock) {
            self.provider.focused(s);
            // currently it gets notifications regardless of enabled status
            // i think this makes most sense?
            self.modifier.focused(s);
        }

        fn unfocused(&mut self, s: MSlock) {
            self.modifier.unfocused(s);
            self.provider.unfocused(s);
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            if self.enabled {
                self.modifier.push_environment(env, s);
            }
            self.provider.push_environment(env, s);

            self.last_env_push_was_enabled = self.enabled;
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.provider.pop_environment(env, s);
            // enabled may change throughout an environment push
            if self.last_env_push_was_enabled {
                self.modifier.pop_environment(env, s);
            }
        }

        fn handle_event(&mut self, e: Event, s: MSlock) -> EventResult {
            self.provider.handle_event(e, s)
        }
    }


    impl<E, P, M> ConditionalIVPModifier<E> for ProviderIVPModifier<E, P, M>
        where E: Environment,
              P: ConditionalIVPModifier<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        type Modifying = P::Modifying;

        fn into_conditional_view_provider(self, e: &E::Const, s: MSlock)
            -> impl ConditionalVPModifier<E, UpContext=<Self::Modifying as IntoViewProvider<E>>::UpContext, DownContext=<Self::Modifying as IntoViewProvider<E>>::DownContext> {
            ProviderVPModifier {
                provider: self.provider.into_conditional_view_provider(e, s),
                modifier: self.modifier,
                enabled: true,
                last_env_push_was_enabled: true,
                phantom: PhantomData,
            }
        }
    }

    impl<E, P, M> ConditionalVPModifier<E> for ProviderVPModifier<E, P, M>
        where E: Environment,
              P: ConditionalVPModifier<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        fn enable(&mut self, s: MSlock) {
            self.enabled = true;
            self.provider.enable(s);
        }

        fn disable(&mut self, s: MSlock) {
            self.provider.disable(s);
            self.enabled = false;
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

    pub trait OffsetModifiable<E>: IntoViewProvider<E> where E: Environment {
        fn offset(self, dx: ScreenUnit, dy: ScreenUnit) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
    }

    impl<E, I> OffsetModifiable<E> for I
        where E: Environment, I: IntoViewProvider<E>
    {
        fn offset(self, dx: ScreenUnit, dy: ScreenUnit) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            let o = Offset {
                dx,
                dy,
            };

            self.provider_modifier(o)
        }
    }
}
pub use provider_modifier::*;

mod tree_modifier {
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
pub use tree_modifier::*;