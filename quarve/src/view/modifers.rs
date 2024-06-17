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
    use crate::state::{FixedSignal, Signal};
    use crate::util::geo::{AlignedOriginRect, Rect, ScreenUnit, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

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

    struct Offset<U, V> where U: Signal<ScreenUnit>, V: Signal<ScreenUnit> {
        dx: U,
        dy: V
    }

    impl<E, U, D, S, T> ProviderModifier<E, U, D> for Offset<S, T>
        where E: Environment, U: 'static, D: 'static, S: Signal<ScreenUnit>, T: Signal<ScreenUnit>
    {
        fn init(&mut self, invalidator: &Invalidator<E>, _env: &E::Const, s: MSlock) {
            let weak_inv = invalidator.clone();
            self.dx.listen(move |val, s| {
                let Some(inv) = weak_inv.upgrade() else {
                    return false;
                };

                inv.invalidate(s);
                true
            }, s);

            let weak_inv = invalidator.clone();
            self.dy.listen(move |val, s| {
                let Some(inv) = weak_inv.upgrade() else {
                    return false;
                };

                inv.invalidate(s);
                true
            }, s);
        }

        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let (mut frame, exclusion) = src.layout_down(subtree, frame, layout_context, env, s);
            frame.x += *self.dx.borrow(s);
            frame.y += *self.dy.borrow(s);

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
                dx: FixedSignal::new(dx),
                dy: FixedSignal::new(dy),
            };

            self.provider_modifier(o)
        }

        fn offset_signal(self, dx: impl Signal<ScreenUnit>, dy: impl Signal<ScreenUnit>) -> ProviderIVPModifier<E, Self, impl ProviderModifier<E, Self::UpContext, Self::DownContext>> {
            let o = Offset { dx, dy };

            self.provider_modifier(o)
        }
    }
}
pub use provider_modifier::*;

mod layer_modifier {
    use std::ffi::c_void;
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::native;
    use crate::util::geo::{AlignedOriginRect, Point, Rect, ScreenUnit, Size};
    use crate::view::util::Color;
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, View, ViewProvider, ViewRef};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    pub struct Layer {
        background_color: Color,
        corner_radius: ScreenUnit,
        border_color: Color,
        border_width: ScreenUnit,
        opacity: f32
    }

    impl Default for Layer {
        fn default() -> Self {
            Layer {
                background_color: Default::default(),
                corner_radius: 0.0,
                border_color: Default::default(),
                border_width: 0.0,
                opacity: 1.0,
            }
        }
    }

    impl Layer {
        pub fn bg_color(mut self, color: Color) -> Layer {
            self.background_color = color;
            self
        }

        pub fn border_color(mut self, color: Color) -> Layer {
            self.border_color = color;
            self
        }

        pub fn radius(mut self, radius: ScreenUnit) -> Layer {
            self.corner_radius = radius;
            self
        }

        pub fn border_width(mut self, width: ScreenUnit) -> Layer {
            self.border_width = width;
            self
        }

        pub fn opacity(mut self, opacity: f32) -> Layer {
            self.opacity = opacity;
            self
        }
    }

    pub struct LayerIVP<E, I> where E: Environment, I: IntoViewProvider<E> {
        layer: Layer,
        ivp: I,
        phantom: PhantomData<E>
    }

    impl<E, I> IntoViewProvider<E> for LayerIVP<E, I>
        where E: Environment, I: IntoViewProvider<E>
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

    impl<E, I> ConditionalIVPModifier<E> for LayerIVP<E, I>
        where E: Environment, I: ConditionalIVPModifier<E>
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

    struct LayerVP<E, P> where E: Environment, P: ViewProvider<E> {
        layer: Layer,
        backing: *mut c_void,
        enabled: bool,
        view: View<E, P>
    }

    impl<E, P> ViewProvider<E> for LayerVP<E, P> where E: Environment, P: ViewProvider<E> {
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
            subtree.push_subview(&self.view, env, s);

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
                native::view::layer::update_layout_view(self.backing, self.layer.background_color, self.layer.border_color, self.layer.corner_radius as f64, self.layer.border_width as f64, self.layer.opacity, s);
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

    impl<E, P> ConditionalVPModifier<E> for LayerVP<E, P>
        where E: Environment, P: ConditionalVPModifier<E>
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
        fn layer(self, layer: impl FnOnce(Layer) -> Layer) -> LayerIVP<E, Self>;
    }

    impl<E, I> LayerModifiable<E> for I where E: Environment, I: IntoViewProvider<E>
    {
        fn layer(self, layer: impl FnOnce(Layer) -> Layer) -> LayerIVP<E, Self> {
            LayerIVP {
                layer: layer(Layer::default()),
                ivp: self,
                phantom: PhantomData
            }
        }
    }
}
pub use layer_modifier::*;

mod tree_modifier {
    use crate::core::Environment;
    use crate::view::{IntoViewProvider};

    trait TreeModifier<E, U, D>
        where E: Environment, U: 'static, D: 'static
    {
        fn embed(self, ivp: impl IntoViewProvider<E, UpContext=U, DownContext=D>)
            -> impl IntoViewProvider<E, UpContext=U, DownContext=D>;
    }
}

// mod tree_modifier {
//     use std::marker::PhantomData;
//     use crate::core::{Environment, MSlock};
//     use crate::util::geo::{AlignedOriginRect, AlignedRect, Point, Rect, ScreenUnit, Size};
//     use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, TrivialContextViewRef, View, ViewProvider, ViewRef};
//     use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier, ProviderModifier};
//     use crate::view::util::Color;
//
//     // allocates a new backing
//     pub trait TreeModifier<E, U, D>: Sized + 'static where E: Environment {
//
//         fn embed(self, ivp: impl IntoViewProvider<E>) -> impl IntoViewProvider<E>;
//
//         fn intrinsic_size(&mut self, src: &View<E, impl ViewProvider<E, UpContext=U, DownContext=D>>, s: MSlock) -> Size {
//             src.intrinsic_size(s)
//         }
//
//         fn xsquished_size(&mut self, src: &View<E, impl ViewProvider<E, UpContext=U, DownContext=D>>, s: MSlock) -> Size {
//             src.xsquished_size(s)
//         }
//
//         fn xstretched_size(&mut self, src: &View<E, impl ViewProvider<E, UpContext=U, DownContext=D>>, s: MSlock) -> Size {
//             src.ysquished_size(s)
//         }
//
//         fn ysquished_size(&mut self, src: &View<E, impl ViewProvider<E, UpContext=U, DownContext=D>>, s: MSlock) -> Size {
//             src.ysquished_size(s)
//         }
//
//         fn ystretched_size(&mut self, src: &View<E, impl ViewProvider<E, UpContext=U, DownContext=D>>, s: MSlock) -> Size {
//             src.ystretched_size(s)
//         }
//
//         #[allow(unused_variables)]
//         fn layout_up(&mut self, src: &View<E, impl ViewProvider<E, UpContext=U, DownContext=D>>, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
//             false
//         }
//
//         #[allow(unused_variables)]
//         fn layout_down(&mut self, src: &View<E, impl ViewProvider<E, UpContext=U, DownContext=D>>, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
//             let rect = src.layout_down_with_context(AlignedRect::new_from_point_size(Point::default(), frame.size(), frame.align), layout_context, env, s);
//             (rect, rect)
//         }
//
//         #[allow(unused_variables)]
//         fn focused(&mut self, s: MSlock)  {
//
//         }
//
//         #[allow(unused_variables)]
//         fn unfocused(&mut self, s: MSlock)  {
//
//         }
//
//         #[allow(unused_variables)]
//         fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
//
//         }
//
//         #[allow(unused_variables)]
//         fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
//
//         }
//     }
//
//     pub struct TreeIVPModifier<E, P, M>
//         where E: Environment,
//               P: IntoViewProvider<E>,
//               M: TreeModifier<E, P::UpContext, P::DownContext>
//     {
//         provider: P,
//         modifier: M,
//         phantom: PhantomData<E>
//     }
//
//     impl<E, P, M> TreeIVPModifier<E, P, M>
//         where E: Environment,
//               P: IntoViewProvider<E>,
//               M: TreeModifier<E, P::UpContext, P::DownContext>
//     {
//         pub fn new(provider: P, modifier: M) -> Self {
//             TreeIVPModifier {
//                 provider,
//                 modifier,
//                 phantom: PhantomData
//             }
//         }
//     }
//
//     struct TreeVPModifier<E, P, M>
//         where E: Environment,
//               P: ViewProvider<E>,
//               M: TreeModifier<E, P::UpContext, P::DownContext>
//     {
//         subview: View<E, P>,
//         modifier: M,
//         enabled: bool,
//         last_env_push_was_enabled: bool,
//         phantom: PhantomData<E>
//     }
//
//     impl<E, P, M> IntoViewProvider<E> for TreeIVPModifier<E, P, M>
//         where E: Environment,
//               P: IntoViewProvider<E>,
//               M: TreeModifier<E, P::UpContext, P::DownContext>
//     {
//         type UpContext = P::UpContext;
//         type DownContext = P::DownContext;
//
//         fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
//             TreeVPModifier {
//                 subview: self.provider
//                     .into_view_provider(env, s)
//                     .into_view(s),
//                 modifier: self.modifier,
//                 enabled: true,
//                 last_env_push_was_enabled: true,
//                 phantom: PhantomData
//             }
//         }
//     }
//
//     impl<E, P, M> ViewProvider<E> for TreeVPModifier<E, P, M>
//         where E: Environment,
//               P: ViewProvider<E>,
//               M: TreeModifier<E, P::UpContext, P::DownContext>
//     {
//         type UpContext = P::UpContext;
//         type DownContext = P::DownContext;
//
//         fn intrinsic_size(&mut self, s: MSlock) -> Size {
//             if self.enabled {
//                 self.modifier.intrinsic_size(&self.subview, s)
//             } else {
//                 self.subview.intrinsic_size(s)
//             }
//         }
//
//         fn xsquished_size(&mut self, s: MSlock) -> Size {
//             if self.enabled {
//                 self.modifier.xsquished_size(&self.subview, s)
//             } else {
//                 self.subview.xsquished_size(s)
//             }
//         }
//
//         fn xstretched_size(&mut self, s: MSlock) -> Size {
//             if self.enabled {
//                 self.modifier.xstretched_size(&self.subview, s)
//             } else {
//                 self.subview.xstretched_size(s)
//             }
//         }
//
//         fn ysquished_size(&mut self, s: MSlock) -> Size {
//             if self.enabled {
//                 self.modifier.ysquished_size(&self.subview, s)
//             } else {
//                 self.subview.ysquished_size(s)
//             }
//         }
//
//         fn ystretched_size(&mut self, s: MSlock) -> Size {
//             if self.enabled {
//                 self.modifier.ystretched_size(&self.subview, s)
//             } else {
//                 self.subview.ystretched_size(s)
//             }
//         }
//
//         fn up_context(&mut self, s: MSlock) -> Self::UpContext {
//             self.subview.up_context(s)
//         }
//
//         fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
//             let mod_source = backing_source
//                 .map(|(nv, bs) | {
//                     self.subview.take_backing(bs.subview, env, s);
//                     (nv, bs.modifier)
//                 });
//
//             self.modifier.init_backing(&self.subview, invalidator, subtree, mod_source, env, s)
//         }
//
//         fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
//             self.modifier.layout_up(&self.subview, subtree, env, s)
//         }
//
//         fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedOriginRect, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
//             self.modifier.layout_down(&self.subview, subtree, frame, layout_context, env, s)
//         }
//
//         fn focused(&mut self, s: MSlock) {
//             self.modifier.focused(s);
//         }
//
//         fn unfocused(&mut self, s: MSlock) {
//             self.modifier.unfocused(s);
//         }
//
//         fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
//             if self.enabled {
//                 self.modifier.push_environment(env, s);
//             }
//
//             self.last_env_push_was_enabled = self.enabled;
//         }
//
//         fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
//             if self.last_env_push_was_enabled {
//                 self.modifier.pop_environment(env, s);
//             }
//         }
//     }
//
//     impl<E, P, M> ConditionalIVPModifier<E> for TreeIVPModifier<E, P, M>
//         where E: Environment,
//               P: ConditionalIVPModifier<E>,
//               M: TreeModifier<E, P::UpContext, P::DownContext>
//     {
//         type Modifying = P::Modifying;
//
//         fn into_conditional_view_provider(self, e: &E::Const, s: MSlock)
//                                           -> impl ConditionalVPModifier<E, UpContext=P::UpContext, DownContext=P::DownContext> {
//             TreeVPModifier {
//                 subview: self.provider
//                     .into_conditional_view_provider(e, s)
//                     .into_view(s),
//                 modifier: self.modifier,
//                 enabled: true,
//                 last_env_push_was_enabled: true,
//                 phantom: PhantomData,
//             }
//         }
//     }
//
//     impl<E, P, M> ConditionalVPModifier<E> for TreeVPModifier<E, P, M>
//         where E: Environment,
//               P: ConditionalVPModifier<E>,
//               M: TreeModifier<E, P::UpContext, P::DownContext>
//     {
//         fn enable(&mut self, s: MSlock) {
//             self.enabled = true;
//             self.subview.with_provider(|p| {
//                 p.enable(s)
//             }, s);
//             self.subview.invalidate(s);
//         }
//
//         fn disable(&mut self, s: MSlock) {
//             self.subview.with_provider(|p| {
//                 p.enable(s)
//             }, s);
//             self.subview.invalidate(s);
//             self.enabled = false;
//         }
//     }
//
//     pub struct Layer {
//         background_color: Color,
//         corner_radius: ScreenUnit,
//         border_color: Color,
//         border_width: ScreenUnit,
//     }
//
//     // impl<E, U, D> TreeModifier<E, U, D> for Layer where U: 'static, D: 'static {
//     //     fn init_backing(&mut self, src: &View<E, impl ViewProvider<E, UpContext=U, DownContext=D>>, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
//     //         subtree.push_subview(src, env, s);
//     //     }
//     //
//     //     fn enable(&mut self, s: MSlock) {
//     //         todo!()
//     //     }
//     //
//     //     fn disable(&mut self, s: MSlock) {
//     //         todo!()
//     //     }
//     // }
// }
// pub use tree_modifier::*;

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