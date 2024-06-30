use crate::core::{Environment, MSlock};
use crate::view::{EnvRef, IntoViewProvider, Subtree, ViewProvider};

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

// dont like how the syntax of the two ivp/vp is mildly different
// (mainly because we don't need modifying once it reaches the vp stage)
pub trait ConditionalVPModifier<E>: ViewProvider<E> where E: Environment
{
    // enable and disabled calls must be called exactly before
    // the underlying layout up call of the view provider
    fn enable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock);
    fn disable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock);
}

mod identity_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::util::geo::{Rect, Size};
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
        fn enable(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) {
            /* no op */
        }

        fn disable(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) {
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

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
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
pub use identity_modifier::*;

mod provider_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock, Slock};
    use crate::event::{Event, EventResult};
    use crate::state::{FixedSignal, Signal, SignalOrValue};
    use crate::util::geo;
    use crate::util::geo::{Alignment, HorizontalAlignment, Point, Rect, ScreenUnit, Size, UNBOUNDED, VerticalAlignment};
    use crate::util::markers::ThreadMarker;
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    // Note that you should generally not
    // affect the subtree in any way
    pub trait ProviderModifier<E, U, D>: Sized + 'static
        where E: Environment, U: 'static, D: 'static {

        #[allow(unused_variables)]
        fn init(&mut self, invalidator: &Invalidator<E>, source: Option<Self>, s: MSlock) {

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

        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: Size, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            src.layout_down(subtree, frame, layout_context, env, s)
        }

        #[allow(unused_variables)]
        fn focused(&mut self, s: MSlock)  {

        }

        #[allow(unused_variables)]
        fn unfocused(&mut self, s: MSlock)  {

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
            if let Some((nv, m)) = backing_source {
                self.modifier.init(&invalidator, Some(m.modifier), s);
                self.provider.init_backing(invalidator, subtree, Some((nv, m.provider)), env, s)
            }
            else {
                self.modifier.init(&invalidator, None, s);
                self.provider.init_backing(invalidator, subtree, None, env, s)
            }

        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.enabled {
                self.modifier.layout_up(&mut self.provider, subtree, env, s)
            } else {
                self.provider.layout_up(subtree, env, s)
            }
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
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
            self.provider.push_environment(env, s);
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.provider.pop_environment(env, s);
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
                phantom: PhantomData,
            }
        }
    }

    impl<E, P, M> ConditionalVPModifier<E> for ProviderVPModifier<E, P, M>
        where E: Environment,
              P: ConditionalVPModifier<E>,
              M: ProviderModifier<E, P::UpContext, P::DownContext>
    {
        fn enable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            if !self.enabled {
                self.enabled = true;
                self.provider.enable(subtree, env, s);
            }
        }

        fn disable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            if self.enabled {
                self.provider.disable(subtree, env, s);
                self.enabled = false;
            }
        }
    }

    pub struct Offset<U, V> where U: Signal<Target=ScreenUnit>, V: Signal<Target=ScreenUnit> {
        dx: SignalOrValue<U>,
        dy: SignalOrValue<V>,
        last_dx: ScreenUnit,
        last_dy: ScreenUnit,
    }

    impl<E, U, D, S, T> ProviderModifier<E, U, D> for Offset<S, T>
        where E: Environment, U: 'static, D: 'static, S: Signal<Target=ScreenUnit>, T: Signal<Target=ScreenUnit>
    {
        fn init(&mut self, invalidator: &Invalidator<E>, _source: Option<Self>, s: MSlock) {
            self.dx.add_invalidator(invalidator, s);
            self.dy.add_invalidator(invalidator, s);
        }

        fn layout_up(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            let ret = src.layout_up(subtree, env, s) || self.dx.inner(s) != self.last_dx || self.dy.inner(s) != self.last_dy;
            self.last_dx = self.dx.inner(s);
            self.last_dy = self.dy.inner(s);
            ret
        }

        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: Size, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
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
        fn offset(self, dx: impl Into<ScreenUnit>, dy: impl Into<ScreenUnit>) -> ProviderIVPModifier<E, Self, impl ProviderModifier<E, Self::UpContext, Self::DownContext>>;
        fn offset_signal(self, dx: impl Signal<Target=ScreenUnit>, dy: impl Signal<Target=ScreenUnit>) -> ProviderIVPModifier<E, Self, impl ProviderModifier<E, Self::UpContext, Self::DownContext>>;
    }

    impl<E, I> OffsetModifiable<E> for I
        where E: Environment, I: IntoViewProvider<E>
    {
        fn offset(self, dx: impl Into<ScreenUnit>, dy: impl Into<ScreenUnit>) -> ProviderIVPModifier<E, Self, impl ProviderModifier<E, Self::UpContext, Self::DownContext>> {
            let o = Offset {
                dx: SignalOrValue::value(dx.into()),
                dy: SignalOrValue::value(dy.into()),
                last_dx: ScreenUnit::NAN,
                last_dy: ScreenUnit::NAN
            };

            ProviderIVPModifier::new(self, o)
        }

        fn offset_signal(self, dx: impl Signal<Target=ScreenUnit>, dy: impl Signal<Target=ScreenUnit>) -> ProviderIVPModifier<E, Self, impl ProviderModifier<E, Self::UpContext, Self::DownContext>> {
            let o = Offset {
                dx: SignalOrValue::Signal(dx),
                dy: SignalOrValue::Signal(dy),
                last_dx: ScreenUnit::NAN,
                last_dy: ScreenUnit::NAN
            };

            ProviderIVPModifier::new(self, o)
        }
    }

    pub struct Padding<S> where S: Signal<Target=ScreenUnit> {
        amount: SignalOrValue<S>,
        edges: u8,
        last_amount: ScreenUnit
    }

    impl<S> Padding<S> where S: Signal<Target=ScreenUnit> {
        fn apply_general(&self, to: Size, invert: bool, s: Slock<impl ThreadMarker>) -> Size {
            let amount = if invert {
                -self.amount.inner(s)
            } else {
                self.amount.inner(s)
            };
            let mut w = to.w;
            let mut h = to.h;
            if self.edges & geo::edge::LEFT != 0 {
                w += amount;
            }
            if self.edges & geo::edge::RIGHT != 0 {
                w += amount;
            }
            if self.edges & geo::edge::UP != 0 {
                h += amount;
            }
            if self.edges & geo::edge::DOWN != 0 {
                h += amount;
            }

            Size::new(w, h)
        }

        fn apply(&self, to: Size, s: Slock<impl ThreadMarker>) -> Size {
            self.apply_general(to, false, s)
        }
    }
    
    impl<E, U, D, S> ProviderModifier<E, U, D> for Padding<S> where E: Environment, U: 'static, D: 'static, S: Signal<Target=ScreenUnit> {
        fn init(&mut self, invalidator: &Invalidator<E>, _source: Option<Self>, s: MSlock) {
            self.amount.add_invalidator(invalidator, s);
        }

        fn xsquished_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            self.apply(src.xsquished_size(s), s)
        }

        fn xstretched_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            self.apply(src.xstretched_size(s), s)
        }

        fn ysquished_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            self.apply(src.ysquished_size(s), s)
        }

        fn ystretched_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            self.apply(src.ystretched_size(s), s)
        }

        fn layout_up(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            let ret = src.layout_up(subtree, env, s) || self.amount.inner(s) != self.last_amount;
            self.last_amount = self.amount.inner(s);
            ret
        }

        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: Size, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let amnt = self.amount.inner(s);
            let inner_size = self.apply_general(frame, true, s);
            let (mut ours, mut total) = src.layout_down(subtree, inner_size, layout_context, env, s);

            let mut subtree_translation = Point::new(0.0, 0.0);
            if self.edges & geo::edge::LEFT != 0 {
                subtree_translation.x += amnt;
                total.w += amnt;
            }

            if self.edges & geo::edge::RIGHT != 0 {
                total.w += amnt;
            }

            if self.edges & geo::edge::UP != 0 {
                total.h += amnt;
            }

            if self.edges & geo::edge::DOWN != 0 {
                subtree_translation.y += amnt;
                total.h += amnt;
            }

            ours = ours.translate(subtree_translation);
            subtree.translate_post_layout_down(subtree_translation, s);

            (ours, total)
        }
    }

    pub trait PaddingModifiable<E>: IntoViewProvider<E>
        where E: Environment
    {
        fn padding(self, amount: impl Into<ScreenUnit>) -> ProviderIVPModifier<E, Self, Padding<FixedSignal<ScreenUnit>>>;
        fn padding_signal<S: Signal<Target=ScreenUnit>>(self, amount: S) -> ProviderIVPModifier<E, Self, Padding<S>>;
        fn padding_edge(self, amount: impl Into<ScreenUnit>, edges: u8) -> ProviderIVPModifier<E, Self, Padding<FixedSignal<ScreenUnit>>>;
        fn padding_edge_signal<S: Signal<Target=ScreenUnit>>(self, amount: S, edges: u8) -> ProviderIVPModifier<E, Self, Padding<S>>;
    }
    
    impl<E, I> PaddingModifiable<E> for I where E: Environment, I: IntoViewProvider<E> {
        fn padding(self, amount: impl Into<ScreenUnit>) -> ProviderIVPModifier<E, Self, Padding<FixedSignal<ScreenUnit>>> {
            let padding = Padding {
                amount: SignalOrValue::value(amount.into()),
                edges: geo::edge::ALL,
                last_amount: ScreenUnit::NAN
            };

            ProviderIVPModifier::new(self, padding)
        }

        fn padding_signal<S>(self, amount: S) -> ProviderIVPModifier<E, Self, Padding<S>> where S: Signal<Target=ScreenUnit> {
            let padding = Padding {
                amount: SignalOrValue::Signal(amount),
                edges: geo::edge::ALL,
                last_amount: ScreenUnit::NAN
            };

            ProviderIVPModifier::new(self, padding)
        }

        fn padding_edge(self, amount: impl Into<ScreenUnit>, edges: u8) -> ProviderIVPModifier<E, Self, Padding<FixedSignal<ScreenUnit>>> {
            let padding = Padding {
                amount: SignalOrValue::value(amount.into()),
                edges,
                last_amount: ScreenUnit::NAN
            };

            ProviderIVPModifier::new(self, padding)
        }

        fn padding_edge_signal<S>(self, amount: S, edges: u8) -> ProviderIVPModifier<E, Self, Padding<S>> where S: Signal<Target=ScreenUnit> {
            let padding = Padding {
                amount: SignalOrValue::Signal(amount),
                edges,
                last_amount: ScreenUnit::NAN
            };

            ProviderIVPModifier::new(self, padding)
        }
    }

    #[derive(Default, Copy, Clone)]
    pub struct Frame {
        squished_w: Option<ScreenUnit>,
        squished_h: Option<ScreenUnit>,
        intrinsic: Option<Size>,
        stretched_w: Option<ScreenUnit>,
        stretched_h: Option<ScreenUnit>,
        alignment: Alignment
    }

    impl Frame {
        pub fn align(mut self, alignment: Alignment) -> Frame {
            self.alignment = alignment;
            self
        }

        pub fn intrinsic(mut self, w: impl Into<ScreenUnit>, h: impl Into<ScreenUnit>) -> Frame {
            self.intrinsic = Some(Size::new(w.into(), h.into()));
            self
        }

        pub fn squished(mut self, w: impl Into<ScreenUnit>, h: impl Into<ScreenUnit>) -> Frame {
            self.squished_w = Some(w.into());
            self.squished_h = Some(h.into());
            self
        }

        pub fn stretched(mut self, w: impl Into<ScreenUnit>, h: impl Into<ScreenUnit>) -> Frame {
            self.stretched_w = Some(w.into());
            self.stretched_h = Some(h.into());
            self
        }

        pub fn unlimited_stretch(self) -> Frame {
            self.stretched(UNBOUNDED, UNBOUNDED)
        }

        pub fn unlimited_width(mut self) -> Frame {
            self.stretched_w = Some(UNBOUNDED);
            self
        }

        pub fn unlimited_height(mut self) -> Frame {
            self.stretched_h = Some(UNBOUNDED);
            self
        }
    }

    impl<E, U, D> ProviderModifier<E, U, D> for Frame where E: Environment, U: 'static, D: 'static {
        fn intrinsic_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            self.intrinsic.unwrap_or(src.intrinsic_size(s))
        }

        fn xsquished_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            let w = self.squished_w.unwrap_or(self.intrinsic_size(src, s).w);
            let h = self.squished_h.unwrap_or(self.intrinsic_size(src, s).h);
            Size::new(w, h)
        }

        fn xstretched_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            let w = self.stretched_w.unwrap_or(self.intrinsic_size(src, s).w);
            let h = self.stretched_h.unwrap_or(self.intrinsic_size(src, s).h);
            Size::new(w, h)
        }

        fn ysquished_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            self.xsquished_size(src, s)
        }

        fn ystretched_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            self.xstretched_size(src, s)
        }

        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: Size, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let min = self.xsquished_size(src, s);
            let max = self.xstretched_size(src, s);

            let chosen = Size::new(
                frame.w.clamp(min.w, max.w),
                frame.h.clamp(min.h, max.h)
            );

            // reposition
            let (view, used) = src.layout_down(subtree, chosen, layout_context, env, s);
            let mut translation = Point::new(0.0,0.0);
            match self.alignment.horizontal() {
                HorizontalAlignment::Leading => {
                    translation.x = -used.x;
                }
                HorizontalAlignment::Center => {
                    translation.x = chosen.w / 2.0 - (used.x + used.w) / 2.0;
                }
                HorizontalAlignment::Trailing => {
                    translation.x = chosen.w - (used.x + used.w);
                }
            }

            match self.alignment.vertical() {
                VerticalAlignment::Bottom => {
                    translation.y = -used.y;
                }
                VerticalAlignment::Center => {
                    translation.y = chosen.h / 2.0 - (used.y + used.h) / 2.0;
                }
                VerticalAlignment::Top => {
                    translation.y = chosen.h - (used.y + used.h);
                }
            }

            subtree.translate_post_layout_down(translation, s);
            (view.translate(translation), chosen.full_rect())
        }
    }

    pub trait FrameModifiable<E>: IntoViewProvider<E>
        where E: Environment
    {
        fn intrinsic(self, w: impl Into<ScreenUnit>, h: impl Into<ScreenUnit>) -> ProviderIVPModifier<E, Self, Frame>;
        fn frame(self, f: Frame) -> ProviderIVPModifier<E, Self, Frame>;
    }

    impl<E, I> FrameModifiable<E> for I where E: Environment, I: IntoViewProvider<E>
    {
        fn intrinsic(self, w: impl Into<ScreenUnit>, h: impl Into<ScreenUnit>) -> ProviderIVPModifier<E, Self, Frame> {
            self.frame(Frame::default().intrinsic(w, h))
        }

        fn frame(self, f: Frame) -> ProviderIVPModifier<E, Self, Frame> {
            ProviderIVPModifier {
                provider: self,
                modifier: f,
                phantom: Default::default(),
            }
        }
    }
}
pub use provider_modifier::*;

mod layer_modifier {
    use std::ffi::c_void;
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::native;
    use crate::state::{FixedSignal, Signal, SignalOrValue};
    use crate::util::geo::{Rect, ScreenUnit, Size};
    use crate::view::util::Color;
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, View, ViewProvider, ViewRef};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    pub struct Layer<S1, S2, S3, S4, S5>
        where S1: Signal<Target=Color>, S2: Signal<Target=ScreenUnit>, S3: Signal<Target=Color>, S4: Signal<Target=ScreenUnit>, S5: Signal<Target=f32> {
        background_color: SignalOrValue<S1>,
        corner_radius: SignalOrValue<S2>,
        border_color: SignalOrValue<S3>,
        border_width: SignalOrValue<S4>,
        opacity: SignalOrValue<S5>
    }

    impl Default for Layer<FixedSignal<Color>, FixedSignal<ScreenUnit>, FixedSignal<Color>, FixedSignal<ScreenUnit>, FixedSignal<f32>>
    {
        fn default() -> Self {
            Layer {
                background_color: SignalOrValue::value(Color::transparent()),
                corner_radius: SignalOrValue::value(0.0),
                border_color: SignalOrValue::value(Color::transparent()),
                border_width: SignalOrValue::value(0.0),
                opacity: SignalOrValue::value(1.0),
            }
        }
    }

    impl<S1, S2, S3, S4, S5> Layer<S1, S2, S3, S4, S5>
        where S1: Signal<Target=Color>, S2: Signal<Target=ScreenUnit>, S3: Signal<Target=Color>, S4: Signal<Target=ScreenUnit>, S5: Signal<Target=f32> {
        pub fn bg_color(self, color: Color) -> Layer<FixedSignal<Color>, S2, S3, S4, S5> {
            Layer {
                background_color: SignalOrValue::value(color),
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn bg_color_signal<S>(self, color: S) -> Layer<S, S2, S3, S4, S5> where S: Signal<Target=Color> {
            Layer {
                background_color: SignalOrValue::Signal(color),
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn border(self, color: Color, width: impl Into<ScreenUnit>) -> Layer<S1, S2, FixedSignal<Color>, FixedSignal<ScreenUnit>, S5>
        {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: SignalOrValue::value(color),
                border_width: SignalOrValue::value(width.into()),
                opacity: self.opacity,
            }
        }

        pub fn border_color(self, color: Color) -> Layer<S1, S2, FixedSignal<Color>, S4, S5> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: SignalOrValue::value(color),
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn border_color_signal<S>(self, color: S) -> Layer<S1, S2, S, S4, S5> where S: Signal<Target=Color> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: SignalOrValue::Signal(color),
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn radius(self, radius: impl Into<ScreenUnit>) -> Layer<S1, FixedSignal<ScreenUnit>, S3, S4, S5> {
            Layer {
                background_color: self.background_color,
                corner_radius: SignalOrValue::value(radius.into()),
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn radius_signal<S>(self, radius: S) -> Layer<S1, S, S3, S4, S5> where S: Signal<Target=ScreenUnit> {
            Layer {
                background_color: self.background_color,
                corner_radius: SignalOrValue::Signal(radius),
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: self.opacity,
            }
        }

        pub fn border_width(self, width: impl Into<ScreenUnit>) -> Layer<S1, S2, S3, FixedSignal<ScreenUnit>, S5> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: SignalOrValue::value(width.into()),
                opacity: self.opacity,
            }
        }

        pub fn border_width_signal<S>(self, width: S) -> Layer<S1, S2, S3, S, S5> where S: Signal<Target=ScreenUnit> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: SignalOrValue::Signal(width),
                opacity: self.opacity,
            }
        }

        pub fn opacity(self, opacity: f32) -> Layer<S1, S2, S3, S4, FixedSignal<f32>> {
            Layer {
                background_color: self.background_color,
                corner_radius: self.corner_radius,
                border_color: self.border_color,
                border_width: self.border_width,
                opacity: SignalOrValue::value(opacity),
            }
        }

        pub fn opacity_signal<S>(self, opacity: S) -> Layer<S1, S2, S3, S4, S> where S: Signal<Target=f32> {
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
              S1: Signal<Target=Color>,
              S2: Signal<Target=ScreenUnit>,
              S3: Signal<Target=Color>,
              S4: Signal<Target=ScreenUnit>,
              S5: Signal<Target=f32>
    {
        layer: Layer<S1, S2, S3, S4, S5>,
        ivp: I,
        phantom: PhantomData<E>
    }

    impl<E, I, S1, S2, S3, S4, S5> IntoViewProvider<E> for LayerIVP<E, I, S1, S2, S3, S4, S5>
        where E: Environment, I: IntoViewProvider<E>,
              S1: Signal<Target=Color>,
              S2: Signal<Target=ScreenUnit>,
              S3: Signal<Target=Color>,
              S4: Signal<Target=ScreenUnit>,
              S5: Signal<Target=f32>
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
              S1: Signal<Target=Color>,
              S2: Signal<Target=ScreenUnit>,
              S3: Signal<Target=Color>,
              S4: Signal<Target=ScreenUnit>,
              S5: Signal<Target=f32>
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
              S1: Signal<Target=Color>,
              S2: Signal<Target=ScreenUnit>,
              S3: Signal<Target=Color>,
              S4: Signal<Target=ScreenUnit>,
              S5: Signal<Target=f32>
    {
        layer: Layer<S1, S2, S3, S4, S5>,
        backing: *mut c_void,
        enabled: bool,
        view: View<E, P>
    }

    impl<E, P, S1, S2, S3, S4, S5> ViewProvider<E> for LayerVP<E, P, S1, S2, S3, S4, S5>
        where E: Environment,
              P: ViewProvider<E>,
              S1: Signal<Target=Color>,
              S2: Signal<Target=ScreenUnit>,
              S3: Signal<Target=Color>,
              S4: Signal<Target=ScreenUnit>,
              S5: Signal<Target=f32>
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

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let used = self.view.layout_down_with_context(frame.full_rect(), layout_context, env, s);
            (used, used)
        }
    }

    impl<E, P, S1, S2, S3, S4, S5> ConditionalVPModifier<E> for LayerVP<E, P, S1, S2, S3, S4, S5>
        where E: Environment, P: ConditionalVPModifier<E>,
              S1: Signal<Target=Color>,
              S2: Signal<Target=ScreenUnit>,
              S3: Signal<Target=Color>,
              S4: Signal<Target=ScreenUnit>,
              S5: Signal<Target=f32>
    {
        fn enable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            if !self.enabled {
                self.enabled = true;

                // FIXME way to do without this hack in the future?
                // typically invalidation should be done by the view, not the superview
                self.view.with_provider(|p| {
                    p.enable(subtree, env, s)
                }, s);
                self.view.invalidate(s)
            }
        }

        fn disable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            if self.enabled {
                self.enabled = false;

                self.view.with_provider(|p| {
                    p.disable(subtree, env, s)
                }, s);
                self.view.invalidate(s)
            }
        }
    }

    pub trait LayerModifiable<E>: IntoViewProvider<E>
        where E: Environment
    {
        fn layer<S1, S2, S3, S4, S5>(self, layer: Layer<S1, S2, S3, S4, S5>) -> LayerIVP<E, Self, S1, S2, S3, S4, S5>
            where S1: Signal<Target=Color>,
                  S2: Signal<Target=ScreenUnit>,
                  S3: Signal<Target=Color>,
                  S4: Signal<Target=ScreenUnit>,
                  S5: Signal<Target=f32>;
    }

    impl<E, I> LayerModifiable<E> for I where E: Environment, I: IntoViewProvider<E>
    {
        fn layer<S1, S2, S3, S4, S5>(self, layer: Layer<S1, S2, S3, S4, S5>) -> LayerIVP<E, Self, S1, S2, S3, S4, S5>
            where S1: Signal<Target=Color>,
                  S2: Signal<Target=ScreenUnit>,
                  S3: Signal<Target=Color>,
                  S4: Signal<Target=ScreenUnit>,
                  S5: Signal<Target=f32>
        {
            LayerIVP {
                layer,
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
    use crate::util::geo::{Rect, Size};
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

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let used = self.view.layout_down_with_context(frame.full_rect(), layout_context, env, s);
            if self.enabled {
                // let pseudo_rect =
                self.attraction.layout_down_with_context(used, layout_context, env, s);
            }

            (used, used)
        }
    }

    impl<E, P, Q> ConditionalVPModifier<E> for ForeBackVP<E, P, Q>
        where E: Environment,
              P: ConditionalVPModifier<E>,
              Q: ViewProvider<E, DownContext=P::DownContext>
    {
        fn enable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            if !self.enabled {
                self.enabled = true;

                self.view.with_provider(|p| {
                    p.enable(subtree, env, s)
                }, s);
                self.view.invalidate(s)
            }
        }

        fn disable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            if self.enabled {
                self.enabled = false;

                self.view.with_provider(|p| {
                    p.disable(subtree, env, s)
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

mod when_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::state::{ActualDiffSignal, Signal};
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};
    use crate::view::modifers::identity_modifier::UnmodifiedIVP;

    // FIXME, if Associated impl trait was allowed
    // we would be able to use ProviderModifier
    // must be a way to re use code in general
    pub struct WhenIVP<E, S, P>
        where E: Environment, S: Signal<Target=bool>, P: ConditionalIVPModifier<E> {
        enabled: S,
        provider: P,
        phantom: PhantomData<E>
    }

    impl<E, S, P> IntoViewProvider<E> for WhenIVP<E, S, P>
        where E: Environment, S: Signal<Target=bool>, P: ConditionalIVPModifier<E>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            WhenVP {
                enabled: self.enabled,
                last_enabled: true,
                parent_enabled: true,
                provider: self.provider.into_conditional_view_provider(env, s),
                phantom: PhantomData,
            }
        }
    }

    impl<E, S, P> ConditionalIVPModifier<E> for WhenIVP<E, S, P>
        where E: Environment, S: Signal<Target=bool>, P: ConditionalIVPModifier<E>
    {
        type Modifying = P::Modifying;

        fn into_conditional_view_provider(self, env: &E::Const, s: MSlock)
            -> impl ConditionalVPModifier<E, UpContext=<Self::Modifying as IntoViewProvider<E>>::UpContext, DownContext=<Self::Modifying as IntoViewProvider<E>>::DownContext> {
            WhenVP {
                enabled: self.enabled,
                last_enabled: true,
                parent_enabled: true,
                provider: self.provider.into_conditional_view_provider(env, s),
                phantom: PhantomData,
            }
        }
    }

    struct WhenVP<E, S, P> where E: Environment, S: Signal<Target=bool>, P: ConditionalVPModifier<E> {
        enabled: S,
        parent_enabled: bool,
        last_enabled: bool,
        provider: P,
        phantom: PhantomData<E>
    }

    impl<E, S, P> WhenVP<E, S, P>
        where E: Environment, S: Signal<Target=bool>, P: ConditionalVPModifier<E>
    {
        fn fully_enabled(&self, s: MSlock) -> bool {
            self.parent_enabled && *self.enabled.borrow(s)
        }
    }

    impl<E, S, P> ViewProvider<E> for WhenVP<E, S, P>
        where E: Environment,
              S: Signal<Target=bool>,
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
            self.enabled.diff_listen(move |_, s| {
                let Some(inv) = inv.upgrade() else {
                    return false;
                };

                inv.invalidate(s);
                true
            }, s);

            self.provider.init_backing(invalidator, subtree, backing_source.map(|(nv, bs)| (nv, bs.provider)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.last_enabled != self.fully_enabled(s) {
                if self.fully_enabled(s) {
                    self.provider.enable(subtree, env, s);
                } else {
                    self.provider.disable(subtree, env, s);
                }

                self.last_enabled = self.fully_enabled(s);
            }

            self.provider.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
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
        where E: Environment, S: Signal<Target=bool>, P: ConditionalVPModifier<E>
    {
        fn enable(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) {
            self.parent_enabled = true;
        }

        fn disable(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>,  _s: MSlock) {
            self.parent_enabled = false;
            // will not call layout up and relay changes (since that will be done immediately
            // after this call)
        }
    }

    pub trait WhenModifiable<E>: IntoViewProvider<E> where E: Environment {
        fn when<S: Signal<Target=bool>, C: ConditionalIVPModifier<E>>(
            self,
            cond: S,
            modifier: impl FnOnce(UnmodifiedIVP<E, Self>) -> C,
        ) -> WhenIVP<E, S, C>;
    }

    impl<E, I> WhenModifiable<E> for I
        where E: Environment, I: IntoViewProvider<E> {
        fn when<S, C>(self, cond: S, modifier: impl FnOnce(UnmodifiedIVP<E, Self>) -> C) -> WhenIVP<E, S, C> where S: Signal<Target=bool>, C: ConditionalIVPModifier<E> {
            WhenIVP {
                enabled: cond,
                provider: modifier(UnmodifiedIVP::new(self)),
                phantom: Default::default(),
            }
        }
    }
}
pub use when_modifier::*;

mod env_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    pub trait EnvironmentModifier<E>: 'static where E: Environment {
        fn init(&mut self, invalidator: Invalidator<E>, s: MSlock);
        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock);
        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock);
    }

    pub struct EnvModifierIVP<E, I, M> where E: Environment, I: IntoViewProvider<E>, M: EnvironmentModifier<E> {
        wrapping: I,
        modifier: M,
        phantom: PhantomData<E>
    }

    impl<E, I, M> IntoViewProvider<E> for EnvModifierIVP<E, I, M> where E: Environment, I: IntoViewProvider<E>, M: EnvironmentModifier<E>
    {
        type UpContext = I::UpContext;
        type DownContext = I::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            EnvModifierVP {
                wrapping: self.wrapping.into_view_provider(env, s),
                modifier: self.modifier,
                enabled: true,
                enabled_during_last_push: false,
                phantom: PhantomData
            }
        }
    }

    impl<E, I, M> ConditionalIVPModifier<E> for EnvModifierIVP<E, I, M>
        where E: Environment, I: ConditionalIVPModifier<E>, M: EnvironmentModifier<E>
    {
        type Modifying = I;

        fn into_conditional_view_provider(self, env: &E::Const, s: MSlock) -> impl ConditionalVPModifier<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            EnvModifierVP {
                wrapping: self.wrapping.into_conditional_view_provider(env, s),
                modifier: self.modifier,
                enabled: true,
                enabled_during_last_push: false,
                phantom: PhantomData
            }
        }
    }

    struct EnvModifierVP<E, P, M> where E: Environment, P: ViewProvider<E>, M: EnvironmentModifier<E> {
        wrapping: P,
        modifier: M,
        enabled: bool,
        enabled_during_last_push: bool,
        phantom: PhantomData<E>
    }

    impl<E, P, M> ViewProvider<E> for EnvModifierVP<E, P, M> where E: Environment, P: ViewProvider<E>, M: EnvironmentModifier<E>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.wrapping.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.wrapping.xsquished_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.wrapping.xstretched_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.wrapping.ysquished_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.wrapping.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.wrapping.up_context(s)
        }

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.modifier.init(invalidator.clone(), s);

            self.wrapping.init_backing(invalidator, subtree, backing_source.map(|(nv, bs)| (nv, bs.wrapping)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            self.wrapping.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.wrapping.layout_down(subtree, frame, layout_context, env, s)
        }

        fn pre_show(&mut self, s: MSlock) {
            self.wrapping.pre_show(s)
        }

        fn post_show(&mut self, s: MSlock) {
            self.wrapping.post_show(s)
        }

        fn pre_hide(&mut self, s: MSlock) {
            self.wrapping.pre_hide(s)
        }

        fn post_hide(&mut self, s: MSlock) {
            self.wrapping.post_hide(s)
        }

        fn focused(&mut self, s: MSlock) {
            self.wrapping.focused(s)
        }

        fn unfocused(&mut self, s: MSlock) {
            self.wrapping.unfocused(s)
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            if self.enabled {
                self.modifier.push_environment(env, s);
                self.enabled_during_last_push = true;
            }
            self.wrapping.push_environment(env, s)
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.wrapping.pop_environment(env, s);
            if self.enabled_during_last_push {
                self.modifier.pop_environment(env, s);
            }
        }

        fn handle_event(&mut self, e: Event, s: MSlock) -> EventResult {
            self.wrapping.handle_event(e, s)
        }
    }

    impl<E, P, M> ConditionalVPModifier<E> for EnvModifierVP<E, P, M>
        where E: Environment, P: ConditionalVPModifier<E>, M: EnvironmentModifier<E>
    {
        fn enable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            // FIXME, we should make it clear whether or not there can be duplicate
            // enable calls, rather than having all of these separate checks
            if !self.enabled {
                self.enabled = true;
                self.wrapping.enable(subtree, env, s);
                self.pop_environment(env.0.variable_env_mut(), s);
                self.push_environment(env.0.variable_env_mut(), s);
                subtree.invalidate_subtree(env, s);
            }
        }

        fn disable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            if self.enabled {
                self.enabled = false;
                self.wrapping.disable(subtree, env, s);
                self.pop_environment(env.0.variable_env_mut(), s);
                self.push_environment(env.0.variable_env_mut(), s);
                subtree.invalidate_subtree(env, s);
            }
        }
    }

    pub trait EnvModifiable<E>: IntoViewProvider<E> where E: Environment {
        fn env_modifier<M: EnvironmentModifier<E>>(self, modifier: M) -> EnvModifierIVP<E, Self, M>;
    }

    impl<E, I> EnvModifiable<E> for I where E: Environment, I: IntoViewProvider<E> {
        fn env_modifier<M: EnvironmentModifier<E>>(self, modifier: M) -> EnvModifierIVP<E, Self, M> {
            EnvModifierIVP {
                wrapping: self,
                modifier,
                phantom: Default::default(),
            }
        }
    }
}
pub use env_modifier::*;

mod show_hide_modifier {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};

    pub struct ShowHideIVP<E, I, F1, F2, F3, F4>
        where E: Environment,
              I: IntoViewProvider<E>,
              F1: FnMut(MSlock) + 'static,
              F2: FnMut(MSlock) + 'static,
              F3: FnMut(MSlock) + 'static,
              F4: FnMut(MSlock) + 'static,
    {
        wrapping: I,
        pre_show: F1,
        post_show: F2,
        pre_hide: F3,
        post_hide: F4,
        phantom: PhantomData<E>
    }

    struct ShowHideVP<E, P, F1, F2, F3, F4>
        where E: Environment,
              P: ViewProvider<E>,
              F1: FnMut(MSlock) + 'static,
              F2: FnMut(MSlock) + 'static,
              F3: FnMut(MSlock) + 'static,
              F4: FnMut(MSlock) + 'static,
    {
        wrapping: P,
        pre_show: F1,
        post_show: F2,
        pre_hide: F3,
        post_hide: F4,
        phantom: PhantomData<E>
    }

    impl<E, I, F1, F2, F3, F4> IntoViewProvider<E> for ShowHideIVP<E, I, F1, F2, F3, F4>
        where E: Environment,
              I: IntoViewProvider<E>,
              F1: FnMut(MSlock) + 'static,
              F2: FnMut(MSlock) + 'static,
              F3: FnMut(MSlock) + 'static,
              F4: FnMut(MSlock) + 'static,
    {
        type UpContext = I::UpContext;
        type DownContext = I::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            ShowHideVP {
                wrapping: self.wrapping.into_view_provider(env, s),
                pre_show: self.pre_show,
                post_show: self.post_show,
                pre_hide: self.pre_hide,
                post_hide: self.post_hide,
                phantom: Default::default(),
            }
        }
    }

    impl<E, P, F1, F2, F3, F4> ViewProvider<E> for ShowHideVP<E, P, F1, F2, F3, F4>
        where E: Environment,
              P: ViewProvider<E>,
              F1: FnMut(MSlock) + 'static,
              F2: FnMut(MSlock) + 'static,
              F3: FnMut(MSlock) + 'static,
              F4: FnMut(MSlock) + 'static,
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.wrapping.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.wrapping.xsquished_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.wrapping.xstretched_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.wrapping.ysquished_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.wrapping.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.wrapping.up_context(s)
        }

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.wrapping.init_backing(invalidator, subtree, backing_source.map(|(nv, bs)| (nv, bs.wrapping)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            self.wrapping.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.wrapping.layout_down(subtree, frame, layout_context, env, s)
        }

        fn pre_show(&mut self, s: MSlock) {
            (self.pre_show)(s);
            self.wrapping.pre_show(s)
        }

        fn post_show(&mut self, s: MSlock) {
            self.wrapping.post_show(s);
            (self.post_show)(s);
        }

        fn pre_hide(&mut self, s: MSlock) {
            (self.pre_hide)(s);
            self.wrapping.pre_hide(s);
        }

        fn post_hide(&mut self, s: MSlock) {
            self.wrapping.post_hide(s);
            (self.post_hide)(s);
        }

        fn focused(&mut self, s: MSlock) {
            self.wrapping.focused(s);
        }

        fn unfocused(&mut self, s: MSlock) {
            self.wrapping.unfocused(s);
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.wrapping.push_environment(env, s);
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.wrapping.pop_environment(env, s);
        }

        fn handle_event(&mut self, e: Event, s: MSlock) -> EventResult {
            self.wrapping.handle_event(e, s)
        }
    }

    pub trait ShowHideCallback<E>: IntoViewProvider<E> where E: Environment {
        fn pre_show(self, f: impl FnMut(MSlock) + 'static) -> impl IntoViewProvider<E>;
        fn post_show(self, f: impl FnMut(MSlock) + 'static) -> impl IntoViewProvider<E>;
        fn pre_hide(self, f: impl FnMut(MSlock) + 'static) -> impl IntoViewProvider<E>;
        fn post_hide(self, f: impl FnMut(MSlock) + 'static) -> impl IntoViewProvider<E>;
    }

    #[inline]
    fn do_nothing(_s: MSlock) {

    }

    pub(crate) fn pre_show_wrap<E: Environment, I: IntoViewProvider<E>>(ivp: I, f: impl FnMut(MSlock) + 'static)
        -> impl IntoViewProvider<E, DownContext=I::DownContext, UpContext=I::UpContext>
    {
        ShowHideIVP {
            wrapping: ivp,
            pre_show: f,
            post_show: do_nothing,
            pre_hide: do_nothing,
            post_hide: do_nothing,
            phantom: Default::default(),
        }
    }

    pub(crate) fn post_show_wrap<E: Environment, I: IntoViewProvider<E>>(ivp: I, f: impl FnMut(MSlock) + 'static)
        -> impl IntoViewProvider<E, DownContext=I::DownContext, UpContext=I::UpContext>
    {
        ShowHideIVP {
            wrapping: ivp,
            pre_show: do_nothing,
            post_show: f,
            pre_hide: do_nothing,
            post_hide: do_nothing,
            phantom: Default::default(),
        }
    }

    pub(crate) fn pre_hide_wrap<E: Environment, I: IntoViewProvider<E>>(ivp: I, f: impl FnMut(MSlock) + 'static)
        -> impl IntoViewProvider<E, DownContext=I::DownContext, UpContext=I::UpContext>
    {
        ShowHideIVP {
            wrapping: ivp,
            pre_show: do_nothing,
            post_show: do_nothing,
            pre_hide: f,
            post_hide: do_nothing,
            phantom: Default::default(),
        }
    }

    pub(crate) fn post_hide_wrap<E: Environment, I: IntoViewProvider<E>>(ivp: I, f: impl FnMut(MSlock) + 'static)
        -> impl IntoViewProvider<E, DownContext=I::DownContext, UpContext=I::UpContext>
    {
        ShowHideIVP {
            wrapping: ivp,
            pre_show: do_nothing,
            post_show: do_nothing,
            pre_hide: do_nothing,
            post_hide: f,
            phantom: Default::default(),
        }
    }
}
pub use show_hide_modifier::*;