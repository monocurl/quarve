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

    impl<E, P> UnmodifiedIVP<E, P> where E: Environment, P: IntoViewProvider<E> {
        pub fn new(wrapping: P) -> Self {
            UnmodifiedIVP {
                source: wrapping,
                phantom: PhantomData
            }
        }
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
    use crate::state::{FixedSignal, Signal, SignalOrValue};
    use crate::util::geo::{AlignedOriginRect, Point, Rect, ScreenUnit, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    // Note that you should generally not
    // affect the subtree in any way
    pub trait ProviderModifier<E, U, D>: Sized + 'static
        where E: Environment, U: 'static, D: 'static {

        #[allow(unused_variables)]
        fn init(&mut self, invalidator: &Invalidator<E>, env: &E::Const, s: MSlock) {

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
            self.modifier.init(&invalidator, env.const_env(), s);
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
            if !self.enabled {
                self.enabled = true;
                self.provider.enable(s);
            }
        }

        fn disable(&mut self, s: MSlock) {
            if self.enabled {
                self.provider.disable(s);
                self.enabled = false;
            }
        }
    }

    pub struct Offset<U, V> where U: Signal<ScreenUnit>, V: Signal<ScreenUnit> {
        dx: SignalOrValue<ScreenUnit, U>,
        dy: SignalOrValue<ScreenUnit, V>,
    }

    impl<E, U, D, S, T> ProviderModifier<E, U, D> for Offset<S, T>
        where E: Environment, U: 'static, D: 'static, S: Signal<ScreenUnit>, T: Signal<ScreenUnit>
    {
        fn init(&mut self, invalidator: &Invalidator<E>, _env: &E::Const, s: MSlock) {
            self.dx.add_invalidator(invalidator, s);
            self.dy.add_invalidator(invalidator, s);
        }

        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let (mut frame, exclusion) = src.layout_down(subtree, frame, layout_context, env, s);
            let delta = Point::new(self.dx.inner(s), self.dy.inner(s));

            // translate current view
            frame = frame.translate(delta);
            // translate entire view subtree
            subtree.translate_post_layout_down(delta, s);

            (frame, exclusion)
        }
    }

    pub trait OffsetModifiable<E>: IntoViewProvider<E> where E: Environment {
        fn offset(self, dx: ScreenUnit, dy: ScreenUnit) -> ProviderIVPModifier<E, Self, impl ProviderModifier<E, Self::UpContext, Self::DownContext>>;
        fn offset_signal(self, dx: impl Signal<ScreenUnit>, dy: impl Signal<ScreenUnit>) -> ProviderIVPModifier<E, Self, impl ProviderModifier<E, Self::UpContext, Self::DownContext>>;
    }

    impl<E, I> OffsetModifiable<E> for I
        where E: Environment, I: IntoViewProvider<E>
    {
        fn offset(self, dx: ScreenUnit, dy: ScreenUnit) -> ProviderIVPModifier<E, Self, impl ProviderModifier<E, Self::UpContext, Self::DownContext>> {
            let o = Offset {
                dx: SignalOrValue::value(dx),
                dy: SignalOrValue::value(dy),
            };

            self.provider_modifier(o)
        }

        fn offset_signal(self, dx: impl Signal<ScreenUnit>, dy: impl Signal<ScreenUnit>) -> ProviderIVPModifier<E, Self, impl ProviderModifier<E, Self::UpContext, Self::DownContext>> {
            let o = Offset {
                dx: SignalOrValue::Signal(dx),
                dy: SignalOrValue::Signal(dy)
            };

            self.provider_modifier(o)
        }
    }

    pub struct Padding<S: Signal<ScreenUnit>> {
        amount: S,
        // sides:
    }

    // pub trait PaddingModifiable {
    //     fn padding(self, amount: ScreenUnit) -> ProviderIVPModifier<E, Self, Padding>
    //     fn padding_signal();
    //     fn padding_e
    // }
}
pub use provider_modifier::*;

mod layer_modifier {
    use std::ffi::c_void;
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::native;
    use crate::state::{FixedSignal, Signal, SignalOrValue};
    use crate::util::geo::{AlignedOriginRect, Point, Rect, ScreenUnit, Size};
    use crate::view::util::Color;
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, View, ViewProvider, ViewRef};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    pub struct Layer<S1, S2, S3, S4, S5> where S1: Signal<Color>, S2: Signal<ScreenUnit>, S3: Signal<Color>, S4: Signal<ScreenUnit>, S5: Signal<f32> {
        background_color: SignalOrValue<Color, S1>,
        corner_radius: SignalOrValue<ScreenUnit, S2>,
        border_color: SignalOrValue<Color, S3>,
        border_width: SignalOrValue<ScreenUnit, S4>,
        opacity: SignalOrValue<f32, S5>
    }

    impl<S1, S2, S3, S4, S5> Layer<S1, S2, S3, S4, S5> where S1: Signal<Color>, S2: Signal<ScreenUnit>, S3: Signal<Color>, S4: Signal<ScreenUnit>, S5: Signal<f32> {
        pub fn bg_color(self, color: Color) -> Layer<FixedSignal<Color>, S2, S3, S4, S5> {
            Layer {
                background_color: SignalOrValue::value(color),
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn bg_color_signal<S: Signal<Color>>(self, color: S) -> Layer<S, S2, S3, S4, S5> {
            Layer {
                background_color: SignalOrValue::Signal(color),
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn border_color(mut self, color: Color) -> Layer<S1, S2, FixedSignal<Color>, S4, S5> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: SignalOrValue::value(color),
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn border_color_signal<S: Signal<Color>>(self, color: S) -> Layer<S1, S2, S, S4, S5> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: SignalOrValue::Signal(color),
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn radius(mut self, radius: ScreenUnit) -> Layer<S1, FixedSignal<ScreenUnit>, S3, S4, S5> {
            Layer {
                background_color: self.background_color,
                corner_radius: SignalOrValue::value(radius),
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn radius_signal<S: Signal<ScreenUnit>>(self, radius: S) -> Layer<S1, S, S3, S4, S5> {
            Layer {
                background_color: self.background_color,
                corner_radius: SignalOrValue::Signal(radius),
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn border_width(mut self, width: ScreenUnit) -> Layer<S1, S2, S3, FixedSignal<ScreenUnit>, S5> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: SignalOrValue::value(width),
                opacity: self.opacity,
            }
        }

        pub fn border_width_signal<S: Signal<ScreenUnit>>(self, width: S) -> Layer<S1, S2, S3, S, S5> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: SignalOrValue::Signal(width),
                opacity: self.opacity,
            }
        }

        pub fn opacity(mut self, opacity: f32) -> Layer<S1, S2, S3, S4, FixedSignal<f32>> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: SignalOrValue::value(opacity),
            }
        }

        pub fn opacity_signal<S: Signal<f32>>(self, opacity: S) -> Layer<S1, S2, S3, S4, S> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: SignalOrValue::Signal(opacity)
            }
        }

    }

    pub struct LayerIVP<E, I, S1, S2, S3, S4, S5>
        where E: Environment,
              I: IntoViewProvider<E>,
              S1: Signal<Color>,
              S2: Signal<ScreenUnit>,
              S3: Signal<Color>,
              S4: Signal<ScreenUnit>,
              S5: Signal<f32>
    {
        layer: Layer<S1, S2, S3, S4, S5>,
        ivp: I,
        phantom: PhantomData<E>
    }

    impl<E, I, S1, S2, S3, S4, S5> IntoViewProvider<E> for LayerIVP<E, I, S1, S2, S3, S4, S5>
        where E: Environment, I: IntoViewProvider<E>,
              S1: Signal<Color>,
              S2: Signal<ScreenUnit>,
              S3: Signal<Color>,
              S4: Signal<ScreenUnit>,
              S5: Signal<f32>
    {
        type UpContext = I::UpContext;
        type DownContext = I::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            LayerVP {
                layer: self.layer,
                backing: 0 as *mut c_void,
                enabled: true,
                view: self.ivp.into_view_provider(env, s)
                    .into_view(s)
            }
        }
    }

    impl<E, I, S1, S2, S3, S4, S5> ConditionalIVPModifier<E> for LayerIVP<E, I, S1, S2, S3, S4, S5>
        where E: Environment, I: ConditionalIVPModifier<E>,
              S1: Signal<Color>,
              S2: Signal<ScreenUnit>,
              S3: Signal<Color>,
              S4: Signal<ScreenUnit>,
              S5: Signal<f32>
    {
        type Modifying = I::Modifying;

        fn into_conditional_view_provider(self, e: &E::Const, s: MSlock) -> impl ConditionalVPModifier<E, UpContext=<Self::Modifying as IntoViewProvider<E>>::UpContext, DownContext=<Self::Modifying as IntoViewProvider<E>>::DownContext> {
            LayerVP {
                layer: self.layer,
                backing: 0 as *mut c_void,
                enabled: true,
                view: self.ivp
                    .into_conditional_view_provider(e, s)
                    .into_view(s)
            }
        }
    }

    struct LayerVP<E, P, S1, S2, S3, S4, S5>
        where E: Environment, P: ViewProvider<E>,
              S1: Signal<Color>,
              S2: Signal<ScreenUnit>,
              S3: Signal<Color>,
              S4: Signal<ScreenUnit>,
              S5: Signal<f32>
    {
        layer: Layer<S1, S2, S3, S4, S5>,
        backing: *mut c_void,
        enabled: bool,
        view: View<E, P>
    }

    impl<E, P, S1, S2, S3, S4, S5> ViewProvider<E> for LayerVP<E, P, S1, S2, S3, S4, S5>
        where E: Environment,
              P: ViewProvider<E>,
              S1: Signal<Color>,
              S2: Signal<ScreenUnit>,
              S3: Signal<Color>,
              S4: Signal<ScreenUnit>,
              S5: Signal<f32>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.view.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.view.xsquished_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.view.xstretched_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.view.ysquished_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.view.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.view.up_context(s)
        }

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            subtree.push_subview(&self.view, env, s);

            self.layer.opacity.add_invalidator(&invalidator, s);
            self.layer.border_width.add_invalidator(&invalidator, s);
            self.layer.border_color.add_invalidator(&invalidator, s);
            self.layer.corner_radius.add_invalidator(&invalidator, s);
            self.layer.background_color.add_invalidator(&invalidator, s);

            if let Some((nv, layer)) = backing_source {
                self.view.take_backing(layer.view, env, s);
                self.backing = nv.view();
                nv
            }
            else {
                let nv = NativeView::layer_view(s);
                self.backing = nv.view();
                nv
            }
        }

        fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.enabled {
                native::view::layer::update_layout_view(
                    self.backing,
                    self.layer.background_color.inner(s),
                    self.layer.border_color.inner(s),
                    self.layer.corner_radius.inner(s) as f64,
                    self.layer.border_width.inner(s) as f64,
                    self.layer.opacity.inner(s),
                    s
                );
            }
            else {
                native::view::layer::update_layout_view(self.backing, Color::transparent(), Color::transparent(), 0.0, 0.0, 1.0, s);
            }
            // generally only called if subview propagated to here
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let used = self.view.layout_down_with_context(frame.aligned_rect(Point::default()), layout_context, env, s);
            (used, used)
        }
    }

    impl<E, P, S1, S2, S3, S4, S5> ConditionalVPModifier<E> for LayerVP<E, P, S1, S2, S3, S4, S5>
        where E: Environment, P: ConditionalVPModifier<E>,
              S1: Signal<Color>,
              S2: Signal<ScreenUnit>,
              S3: Signal<Color>,
              S4: Signal<ScreenUnit>,
              S5: Signal<f32>
    {
        fn enable(&mut self, s: MSlock) {
            if !self.enabled {
                self.enabled = true;

                // FIXME way to do without this hack in the future?
                // would rather not invalidate entire subtree
                self.view.with_provider(|p| {
                    p.enable(s)
                }, s);
                self.view.invalidate(s)
            }
        }

        fn disable(&mut self, s: MSlock) {
            if self.enabled {
                self.enabled = false;

                self.view.with_provider(|p| {
                    p.disable(s)
                }, s);
                self.view.invalidate(s)
            }
        }
    }

    pub trait LayerModifiable<E>: IntoViewProvider<E>
        where E: Environment
    {
        fn layer<S1, S2, S3, S4, S5>(
            self,
            layer: impl FnOnce(Layer<FixedSignal<Color>, FixedSignal<ScreenUnit>, FixedSignal<Color>, FixedSignal<ScreenUnit>, FixedSignal<f32>>)
                -> Layer<S1, S2, S3, S4, S5>
        ) -> LayerIVP<E, Self, S1, S2, S3, S4, S5>
            where S1: Signal<Color>,
                  S2: Signal<ScreenUnit>,
                  S3: Signal<Color>,
                  S4: Signal<ScreenUnit>,
                  S5: Signal<f32>;
    }

    impl<E, I> LayerModifiable<E> for I where E: Environment, I: IntoViewProvider<E>
    {
        fn layer<S1, S2, S3, S4, S5>(self, layer: impl FnOnce(Layer<FixedSignal<Color>, FixedSignal<ScreenUnit>, FixedSignal<Color>, FixedSignal<ScreenUnit>, FixedSignal<f32>>) -> Layer<S1, S2, S3, S4, S5>) -> LayerIVP<E, Self, S1, S2, S3, S4, S5>
            where S1: Signal<Color>,
                  S2: Signal<ScreenUnit>,
                  S3: Signal<Color>,
                  S4: Signal<ScreenUnit>,
                  S5: Signal<f32>
        {
            LayerIVP {
                layer: layer(Layer {
                    background_color: SignalOrValue::value(Color::transparent()),
                    corner_radius: SignalOrValue::value(0.0),
                    border_color: SignalOrValue::value(Color::black()),
                    border_width: SignalOrValue::value(0.0),
                    opacity: SignalOrValue::value(1.0),
                }),
                ivp: self,
                phantom: PhantomData
            }
        }
    }
}
pub use layer_modifier::*;

mod foreback_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::util::geo::{AlignedOriginRect, Point, Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, View, ViewProvider, ViewRef};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    pub struct ForeBackIVP<E, I, J> where E: Environment,
                                          I: IntoViewProvider<E>,
                                          J: IntoViewProvider<E, DownContext=I::DownContext> {
        is_foreground: bool,
        ivp: I,
        added_ivp: J,
        phantom: PhantomData<E>
    }

    impl<E, I, J> IntoViewProvider<E> for ForeBackIVP<E, I, J>
        where E: Environment,
              I: IntoViewProvider<E>,
              J: IntoViewProvider<E, DownContext=I::DownContext>
    {
        type UpContext = I::UpContext;
        type DownContext = I::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            ForeBackVP {
                prev_frame_enabled: false,
                enabled: true,
                is_foreground: self.is_foreground,
                view: self.ivp
                    .into_view_provider(env, s)
                    .into_view(s),
                attraction: self.added_ivp
                    .into_view_provider(env, s)
                    .into_view(s),
            }
        }
    }

    impl<E, I, J> ConditionalIVPModifier<E> for ForeBackIVP<E, I, J>
        where E: Environment,
              I: ConditionalIVPModifier<E>,
              J: IntoViewProvider<E, DownContext=I::DownContext>
    {
        type Modifying = I::Modifying;

        fn into_conditional_view_provider(self, e: &E::Const, s: MSlock) -> impl ConditionalVPModifier<E, UpContext=<Self::Modifying as IntoViewProvider<E>>::UpContext, DownContext=<Self::Modifying as IntoViewProvider<E>>::DownContext> {
            ForeBackVP {
                prev_frame_enabled: false,
                enabled: true,
                is_foreground: self.is_foreground,
                view: self.ivp
                    .into_conditional_view_provider(e, s)
                    .into_view(s),
                attraction: self.added_ivp
                    .into_view_provider(e, s)
                    .into_view(s),
            }
        }
    }

    struct ForeBackVP<E, P, Q>
        where E: Environment,
              P: ViewProvider<E>,
              Q: ViewProvider<E, DownContext=P::DownContext>
    {
        prev_frame_enabled: bool,
        enabled: bool,
        is_foreground: bool,
        view: View<E, P>,
        attraction: View<E, Q>
    }

    impl<E, P, Q> ViewProvider<E> for ForeBackVP<E, P, Q>
        where E: Environment,
              P: ViewProvider<E>,
              Q: ViewProvider<E, DownContext=P::DownContext>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.view.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.view.xsquished_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.view.xstretched_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.view.ysquished_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.view.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.view.up_context(s)
        }

        fn init_backing(&mut self, _invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            if let Some((nv, source)) = backing_source {
                self.view.take_backing(source.view, env, s);
                self.attraction.take_backing(source.attraction, env, s);
                subtree.push_subview(&self.view, env, s);

                nv
            }
            else {
                subtree.push_subview(&self.view, env, s);

                NativeView::layout_view(s)
            }
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.enabled != self.prev_frame_enabled {
                subtree.clear_subviews(s);
                subtree.push_subview(&self.view, env, s);

                if self.enabled {
                    let index = if self.is_foreground {
                        1
                    }
                    else {
                        0
                    };

                    subtree.insert_subview(&self.attraction, index, env, s);
                }
            }
            self.prev_frame_enabled = self.enabled;

            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let used = self.view.layout_down_with_context(frame.aligned_rect(Point::default()), layout_context, env, s);
            if self.enabled {
                // let pseudo_rect =
                self.attraction.layout_down_with_context(frame.aligned_rect(Point::default()), layout_context, env, s);
            }

            (used, used)
        }
    }

    impl<E, P, Q> ConditionalVPModifier<E> for ForeBackVP<E, P, Q>
        where E: Environment,
              P: ConditionalVPModifier<E>,
              Q: ViewProvider<E, DownContext=P::DownContext>
    {
        fn enable(&mut self, s: MSlock) {
            if !self.enabled {
                self.enabled = true;

                self.view.with_provider(|p| {
                    p.enable(s)
                }, s);
                self.view.invalidate(s)
            }
        }

        fn disable(&mut self, s: MSlock) {
            if self.enabled {
                self.enabled = false;

                self.view.with_provider(|p| {
                    p.disable(s)
                }, s);
                self.view.invalidate(s)
            }
        }
    }

    pub trait ForeBackModifiable<E>: IntoViewProvider<E> where E: Environment {
        fn foreground<I: IntoViewProvider<E, DownContext=Self::DownContext>>(self, attraction: I)
            -> ForeBackIVP<E, Self, I>;
        fn background<I: IntoViewProvider<E, DownContext=Self::DownContext>>(self, attraction: I)
            -> ForeBackIVP<E, Self, I>;
    }

    impl<E, J> ForeBackModifiable<E> for J where E: Environment, J: IntoViewProvider<E> {
        fn foreground<I: IntoViewProvider<E, DownContext=Self::DownContext>>(self, attraction: I) -> ForeBackIVP<E, Self, I> {
            ForeBackIVP {
                is_foreground: true,
                ivp: self,
                added_ivp: attraction,
                phantom: PhantomData
            }
        }

        fn background<I: IntoViewProvider<E, DownContext=Self::DownContext>>(self, attraction: I) -> ForeBackIVP<E, Self, I> {
            ForeBackIVP {
                is_foreground: false,
                ivp: self,
                added_ivp: attraction,
                phantom: PhantomData
            }
        }
    }
}
pub use foreback_modifier::*;

mod tree_modifier {
    // TODO once portals are done
}

mod when_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::state::Signal;
    use crate::util::geo::{AlignedOriginRect, Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier, ProviderModifier};
    use crate::view::modifers::identity_modifier::UnmodifiedIVP;

    // FIXME, if Associated impl trait was allowed
    // we would be able to use ProviderModifier
    // must be a way to re use code in general
    pub struct WhenIVP<E, S, P>
        where E: Environment, S: Signal<bool>, P: ConditionalIVPModifier<E> {
        enabled: S,
        provider: P,
        phantom: PhantomData<E>
    }

    impl<E, S, P> IntoViewProvider<E> for WhenIVP<E, S, P>
        where E: Environment, S: Signal<bool>, P: ConditionalIVPModifier<E>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            WhenVP {
                enabled: self.enabled,
                parent_enabled: true,
                provider: self.provider.into_conditional_view_provider(env, s),
                phantom: PhantomData,
            }
        }
    }

    impl<E, S, P> ConditionalIVPModifier<E> for WhenIVP<E, S, P>
        where E: Environment, S: Signal<bool>, P: ConditionalIVPModifier<E>
    {
        type Modifying = P::Modifying;

        fn into_conditional_view_provider(self, env: &E::Const, s: MSlock)
            -> impl ConditionalVPModifier<E, UpContext=<Self::Modifying as IntoViewProvider<E>>::UpContext, DownContext=<Self::Modifying as IntoViewProvider<E>>::DownContext> {
            WhenVP {
                enabled: self.enabled,
                parent_enabled: true,
                provider: self.provider.into_conditional_view_provider(env, s),
                phantom: PhantomData,
            }
        }
    }

    struct WhenVP<E, S, P> where E: Environment, S: Signal<bool>, P: ConditionalVPModifier<E> {
        enabled: S,
        parent_enabled: bool,
        provider: P,
        phantom: PhantomData<E>
    }

    impl<E, S, P> WhenVP<E, S, P>
        where E: Environment, S: Signal<bool>, P: ConditionalVPModifier<E>
    {
        fn fully_enabled(&self, s: MSlock) -> bool {
            self.parent_enabled && *self.enabled.borrow(s)
        }
    }

    impl<E, S, P> ViewProvider<E> for WhenVP<E, S, P>
        where E: Environment,
              S: Signal<bool>,
              P: ConditionalVPModifier<E>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.provider.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.provider.xsquished_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.provider.xstretched_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.provider.ysquished_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.provider.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.provider.up_context(s)
        }

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            let inv = invalidator.clone();
            self.enabled.listen(move |_, s| {
                let Some(inv) = inv.upgrade() else {
                    return false;
                };

                inv.invalidate(s);
                true
            }, s);

            self.provider.init_backing(invalidator, subtree, backing_source.map(|(nv, bs)| (nv, bs.provider)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            // might be redundant in some cases
            // though i don't think it should be a major issue
            if self.fully_enabled(s) {
                self.provider.enable(s);
            }
            else {
                self.provider.disable(s);
            }

            self.provider.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.provider.layout_down(subtree, frame, layout_context, env, s)
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
        }

        fn unfocused(&mut self, s: MSlock) {
            self.provider.unfocused(s);
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.provider.push_environment(env, s);
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.provider.pop_environment(env, s);
        }

        fn handle_event(&mut self, e: Event, s: MSlock) -> EventResult {
            self.provider.handle_event(e, s)
        }
    }

    impl<E, S, P> ConditionalVPModifier<E> for WhenVP<E, S, P>
        where E: Environment, S: Signal<bool>, P: ConditionalVPModifier<E>
    {
        fn enable(&mut self, s: MSlock) {
            let old = self.fully_enabled(s);
            self.parent_enabled = true;
            let new = self.fully_enabled(s);
            if !old && new {
                self.provider.enable(s);
            }
        }

        fn disable(&mut self, s: MSlock) {
            let old = self.fully_enabled(s);
            self.parent_enabled = false;
            let new = self.fully_enabled(s);

            if old && !new {
                self.provider.disable(s);
            }
        }
    }

    pub trait WhenModifiable<E>: IntoViewProvider<E> where E: Environment {
        fn when<S: Signal<bool>, C: ConditionalIVPModifier<E>>(
            self,
            cond: S,
            modifier: impl FnOnce(UnmodifiedIVP<E, Self>) -> C,
        ) -> WhenIVP<E, S, C>;
    }

    impl<E, I> WhenModifiable<E> for I
        where E: Environment, I: IntoViewProvider<E> {
        fn when<S, C>(self, cond: S, modifier: impl FnOnce(UnmodifiedIVP<E, Self>) -> C) -> WhenIVP<E, S, C> where S: Signal<bool>, C: ConditionalIVPModifier<E> {
            WhenIVP {
                enabled: cond,
                provider: modifier(UnmodifiedIVP::new(self)),
                phantom: Default::default(),
            }
        }
    }
}
pub use when_modifier::*;