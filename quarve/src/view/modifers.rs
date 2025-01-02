pub use cursor::*;
pub use env_modifier::*;
pub use foreback_modifier::*;
pub use identity_modifier::*;
pub use key_listener::*;
pub use layer_modifier::*;
pub use provider_modifier::*;
pub use show_hide_modifier::*;
pub use when_modifier::*;

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
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};
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

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.source.init_backing(invalidator, subtree, backing_source.map(|(nv, this)| (nv, this.source)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            self.source.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.source.layout_down(subtree, frame, layout_context, env, s)
        }

        fn finalize_frame(&self, frame: Rect, s: MSlock) {
            self.source.finalize_frame(frame, s);
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

        fn focused(&self, rel_depth: u32, s: MSlock) {
            self.source.focused(rel_depth, s)
        }

        fn unfocused(&self, rel_depth: u32, s: MSlock) {
            self.source.unfocused(rel_depth, s)
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.push_environment(env, s)
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.pop_environment(env, s)
        }

        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            self.source.handle_event(e, s)
        }
    }
}

mod provider_modifier {
    use std::marker::PhantomData;

    use crate::core::{Environment, MSlock, Slock};
    use crate::event::{Event, EventResult};
    use crate::state::{FixedSignal, Signal, SignalOrValue};
    use crate::util::geo;
    use crate::util::geo::{Alignment, HorizontalAlignment, Point, Rect, ScreenUnit, Size, UNBOUNDED, VerticalAlignment};
    use crate::util::marker::ThreadMarker;
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    // Note that you should generally not
    // affect the subtree in any way
    pub trait ProviderModifier<E, U, D>: Sized + 'static
        where E: Environment, U: 'static, D: 'static {

        #[allow(unused_variables)]
        fn init(&mut self, invalidator: &WeakInvalidator<E>, source: Option<Self>, s: MSlock) {

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
        fn focused(&self, s: MSlock)  {

        }

        #[allow(unused_variables)]
        fn unfocused(&self, s: MSlock)  {

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
        source: P,
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
                source: self.provider.into_view_provider(env, s),
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
                self.modifier.intrinsic_size(&mut self.source, s)
            } else {
                self.source.intrinsic_size(s)
            }
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            if self.enabled {
                self.modifier.xsquished_size(&mut self.source, s)
            } else {
                self.source.xsquished_size(s)
            }
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            if self.enabled {
                self.modifier.xstretched_size(&mut self.source, s)
            } else {
                self.source.xstretched_size(s)
            }
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            if self.enabled {
                self.modifier.ysquished_size(&mut self.source, s)
            } else {
                self.source.ysquished_size(s)
            }
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            if self.enabled {
                self.modifier.ystretched_size(&mut self.source, s)
            } else {
                self.source.ystretched_size(s)
            }
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.source.up_context(s)
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            if let Some((nv, m)) = backing_source {
                self.modifier.init(&invalidator, Some(m.modifier), s);
                self.source.init_backing(invalidator, subtree, Some((nv, m.source)), env, s)
            }
            else {
                self.modifier.init(&invalidator, None, s);
                self.source.init_backing(invalidator, subtree, None, env, s)
            }

        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.enabled {
                self.modifier.layout_up(&mut self.source, subtree, env, s)
            } else {
                self.source.layout_up(subtree, env, s)
            }
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            if self.enabled {
                self.modifier.layout_down(&mut self.source, subtree, frame, layout_context, env, s)
            } else {
                self.source.layout_down(subtree, frame, layout_context, env, s)
            }
        }

        fn finalize_frame(&self, frame: Rect, s: MSlock) {
            self.source.finalize_frame(frame, s);
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

        fn focused(&self, rel_depth: u32, s: MSlock) {
            self.source.focused(rel_depth, s);
            // currently it gets notifications regardless of enabled status
            // i think this makes most sense?
            self.modifier.focused(s);
        }

        fn unfocused(&self, rel_depth: u32, s: MSlock) {
            self.modifier.unfocused(s);
            self.source.unfocused(rel_depth, s);
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.push_environment(env, s);
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.pop_environment(env, s);
        }

        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            self.source.handle_event(e, s)
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
                source: self.provider.into_conditional_view_provider(e, s),
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
                self.source.enable(subtree, env, s);
            }
        }

        fn disable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            if self.enabled {
                self.source.disable(subtree, env, s);
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
        fn init(&mut self, invalidator: &WeakInvalidator<E>, _source: Option<Self>, s: MSlock) {
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
        fn init(&mut self, invalidator: &WeakInvalidator<E>, _source: Option<Self>, s: MSlock) {
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
                subtree_translation.y += amnt;
                total.h += amnt;
            }

            if self.edges & geo::edge::DOWN != 0 {
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
        pub const fn new() -> Self {
            Frame {
                squished_w: None,
                squished_h: None,
                intrinsic: None,
                stretched_w: None,
                stretched_h: None,
                alignment: Alignment::Center
            }
        }

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
            let w = self.squished_w
                .or(self.intrinsic.map(|i| i.w))
                .unwrap_or_else(|| src.xsquished_size(s).w);
            let h = self.squished_w
                .or(self.intrinsic.map(|i| i.h))
                .unwrap_or_else(|| src.xsquished_size(s).h);
            Size::new(w, h)
        }

        fn xstretched_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            let w = self.stretched_w
                .or(self.intrinsic.map(|i| i.w))
                .unwrap_or_else(|| src.xstretched_size(s).w);
            let h = self.stretched_h
                .or(self.intrinsic.map(|i| i.h))
                .unwrap_or_else(|| src.xstretched_size(s).h);
            Size::new(w, h)
        }

        fn ysquished_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            let w = self.squished_w
                .or(self.intrinsic.map(|i| i.w))
                .unwrap_or_else(|| src.ysquished_size(s).w);
            let h = self.squished_h
                .or(self.intrinsic.map(|i| i.h))
                .unwrap_or_else(|| src.ysquished_size(s).h);
            Size::new(w, h)
        }

        fn ystretched_size(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, s: MSlock) -> Size {
            let w = self.stretched_w
                .or(self.intrinsic.map(|i| i.w))
                .unwrap_or_else(|| src.ystretched_size(s).w);
            let h = self.stretched_h
                .or(self.intrinsic.map(|i| i.h))
                .unwrap_or_else(|| src.ystretched_size(s).h);
            Size::new(w, h)
        }

        fn layout_down(&mut self, src: &mut impl ViewProvider<E, UpContext=U, DownContext=D>, subtree: &Subtree<E>, frame: Size, layout_context: &D, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let min_x = self.xsquished_size(src, s);
            let max_x= self.xstretched_size(src, s);
            let min_y = self.ysquished_size(src, s);
            let max_y= self.ystretched_size(src, s);

            let chosen = Size::new(
                frame.w.clamp(min_x.w, max_x.w),
                frame.h.clamp(min_y.h, max_y.h)
            );

            // reposition
            let (view, used) = src.layout_down(subtree, chosen, layout_context, env, s);
            let mut translation = Point::new(0.0,0.0);
            translation.x = match self.alignment.horizontal() {
                HorizontalAlignment::Leading => {
                    -used.x
                }
                HorizontalAlignment::Center => {
                    chosen.w / 2.0 - (used.x + used.w) / 2.0
                }
                HorizontalAlignment::Trailing => {
                    chosen.w - (used.x + used.w)
                }
            };

            translation.y = match self.alignment.vertical() {
                VerticalAlignment::Top => {
                    -used.y
                }
                VerticalAlignment::Center => {
                    chosen.h / 2.0 - (used.y + used.h) / 2.0
                }
                VerticalAlignment::Bottom => {
                    chosen.h - (used.y + used.h)
                }
            };

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

mod layer_modifier {
    use std::ffi::c_void;
    use std::marker::PhantomData;

    use crate::core::{Environment, MSlock};
    use crate::native;
    use crate::native::view::layer::set_layer_view_frame;
    use crate::state::{FixedSignal, Signal, SignalOrValue};
    use crate::util::geo::{Rect, ScreenUnit, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, View, ViewProvider, ViewRef, WeakInvalidator};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};
    use crate::view::util::Color;

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
                background_color: SignalOrValue::value(Color::clear()),
                corner_radius: SignalOrValue::value(0.0),
                border_color: SignalOrValue::value(Color::clear()),
                border_width: SignalOrValue::value(0.0),
                opacity: SignalOrValue::value(1.0),
            }
        }
    }

    impl Layer<FixedSignal<Color>, FixedSignal<ScreenUnit>, FixedSignal<Color>, FixedSignal<ScreenUnit>, FixedSignal<f32>>
    {
        pub const fn new() -> Self {
            Layer {
                background_color: SignalOrValue::value(Color::clear()),
                corner_radius: SignalOrValue::value(0.0),
                border_color: SignalOrValue::value(Color::clear()),
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

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.layer.opacity.add_invalidator(&invalidator, s);
            self.layer.border_width.add_invalidator(&invalidator, s);
            self.layer.border_color.add_invalidator(&invalidator, s);
            self.layer.corner_radius.add_invalidator(&invalidator, s);
            self.layer.background_color.add_invalidator(&invalidator, s);

            if let Some((nv, layer)) = backing_source {
                self.view.take_backing(layer.view, env, s);
                subtree.push_subview(&self.view, env, s);

                self.backing = nv.backing();
                nv
            }
            else {
                subtree.push_subview(&self.view, env, s);

                let nv = NativeView::layer_view(s);
                self.backing = nv.backing();
                nv
            }
        }

        fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.enabled {
                native::view::layer::update_layer_view(
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
                native::view::layer::update_layer_view(self.backing, Color::clear(), Color::clear(), 0.0, 0.0, 1.0, s);
            }
            // generally only called if subview propagated to here
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let used = self.view.layout_down_with_context(frame.full_rect(), layout_context, env, s);
            (used, used)
        }

        fn finalize_frame(&self, frame: Rect, s: MSlock) {
            set_layer_view_frame(self.backing, frame, s);
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

mod foreback_modifier {
    use std::marker::PhantomData;

    use crate::core::{Environment, MSlock};
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, View, ViewProvider, ViewRef, WeakInvalidator};
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

        fn init_backing(&mut self, _invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
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

mod when_modifier {
    use std::marker::PhantomData;

    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::state::{ActualDiffSignal, Signal};
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};
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
                source: self.provider.into_conditional_view_provider(env, s),
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
                source: self.provider.into_conditional_view_provider(env, s),
                phantom: PhantomData,
            }
        }
    }

    struct WhenVP<E, S, P> where E: Environment, S: Signal<Target=bool>, P: ConditionalVPModifier<E> {
        enabled: S,
        parent_enabled: bool,
        last_enabled: bool,
        source: P,
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
            self.source.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.source.up_context(s)
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            let inv = invalidator.clone();
            self.enabled.diff_listen(move |_, s| {
                let Some(inv) = inv.upgrade() else {
                    return false;
                };

                inv.invalidate(s);
                true
            }, s);

            self.source.init_backing(invalidator, subtree, backing_source.map(|(nv, bs)| (nv, bs.source)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.last_enabled != self.fully_enabled(s) {
                if self.fully_enabled(s) {
                    self.source.enable(subtree, env, s);
                } else {
                    self.source.disable(subtree, env, s);
                }

                self.last_enabled = self.fully_enabled(s);
            }

            self.source.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.source.layout_down(subtree, frame, layout_context, env, s)
        }

        fn finalize_frame(&self, frame: Rect, s: MSlock) {
            self.source.finalize_frame(frame, s);
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

        fn focused(&self, rel_depth: u32, s: MSlock) {
            self.source.focused(rel_depth, s);
        }

        fn unfocused(&self, rel_depth: u32, s: MSlock) {
            self.source.unfocused(rel_depth, s);
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.push_environment(env, s);
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.pop_environment(env, s);
        }

        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            self.source.handle_event(e, s)
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

mod env_modifier {
    use std::marker::PhantomData;

    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};
    use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

    pub trait EnvironmentModifier<E>: 'static where E: Environment {
        fn init(&mut self, invalidator: WeakInvalidator<E>, s: MSlock);
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
                source: self.wrapping.into_view_provider(env, s),
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
                source: self.wrapping.into_conditional_view_provider(env, s),
                modifier: self.modifier,
                enabled: true,
                enabled_during_last_push: false,
                phantom: PhantomData
            }
        }
    }

    struct EnvModifierVP<E, P, M> where E: Environment, P: ViewProvider<E>, M: EnvironmentModifier<E> {
        source: P,
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
            self.source.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.source.up_context(s)
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.modifier.init(invalidator.clone(), s);

            self.source.init_backing(invalidator, subtree, backing_source.map(|(nv, bs)| (nv, bs.source)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            self.source.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.source.layout_down(subtree, frame, layout_context, env, s)
        }

        fn finalize_frame(&self, frame: Rect, s: MSlock) {
            self.source.finalize_frame(frame, s);
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

        fn focused(&self, rel_depth: u32, s: MSlock) {
            self.source.focused(rel_depth, s)
        }

        fn unfocused(&self, rel_depth: u32, s: MSlock) {
            self.source.unfocused(rel_depth, s)
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            if self.enabled {
                self.modifier.push_environment(env, s);
                self.enabled_during_last_push = true;
            }
            self.source.push_environment(env, s)
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.pop_environment(env, s);
            if self.enabled_during_last_push {
                self.modifier.pop_environment(env, s);
            }
        }

        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            self.source.handle_event(e, s)
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
                self.source.enable(subtree, env, s);
                self.pop_environment(env.0.variable_env_mut(), s);
                self.push_environment(env.0.variable_env_mut(), s);
                subtree.invalidate_subtree(env, s);
            }
        }

        fn disable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
            if self.enabled {
                self.enabled = false;
                self.source.disable(subtree, env, s);
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

mod show_hide_modifier {
    use std::marker::PhantomData;

    use crate::core::{Environment, MSlock};
    use crate::event::{Event, EventResult};
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};

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
        source: P,
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
                source: self.wrapping.into_view_provider(env, s),
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
            self.source.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.source.up_context(s)
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.source.init_backing(invalidator, subtree, backing_source.map(|(nv, bs)| (nv, bs.source)), env, s)
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            self.source.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.source.layout_down(subtree, frame, layout_context, env, s)
        }

        fn finalize_frame(&self, frame: Rect, s: MSlock) {
            self.source.finalize_frame(frame, s);
        }

        fn pre_show(&mut self, s: MSlock) {
            (self.pre_show)(s);
            self.source.pre_show(s)
        }

        fn post_show(&mut self, s: MSlock) {
            self.source.post_show(s);
            (self.post_show)(s);
        }

        fn pre_hide(&mut self, s: MSlock) {
            (self.pre_hide)(s);
            self.source.pre_hide(s);
        }

        fn post_hide(&mut self, s: MSlock) {
            self.source.post_hide(s);
            (self.post_hide)(s);
        }

        fn focused(&self, rel_depth: u32, s: MSlock) {
            self.source.focused(rel_depth, s);
        }

        fn unfocused(&self, rel_depth: u32, s: MSlock) {
            self.source.unfocused(rel_depth, s);
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.push_environment(env, s);
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.pop_environment(env, s);
        }

        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            self.source.handle_event(e, s)
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

mod key_listener {
    use std::marker::PhantomData;
    use std::sync::{Arc, Weak};

    use crate::core::{Environment, MSlock, WindowViewCallback};
    use crate::event::{Event, EventModifiers, EventPayload, EventResult, KeyEvent};
    use crate::state::slock_cell::MainSlockCell;
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, InnerViewBase, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};

    struct KeyListenerIVP<E, I, F>
        where E: Environment,
              I: IntoViewProvider<E>,
              F: Fn(&str, EventModifiers, MSlock) + 'static
    {
        source: I,
        listener: F,
        phantom: PhantomData<E>
    }

    impl<E, I, F> IntoViewProvider<E> for KeyListenerIVP<E, I, F>
        where E: Environment,
              I: IntoViewProvider<E>,
              F: Fn(&str, EventModifiers, MSlock) + 'static
    {
        type UpContext = I::UpContext;
        type DownContext = I::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            KeyListenerVP {
                source: self.source.into_view_provider(env, s),
                listener: self.listener,
                window: None,
                owner: None,
                phantom: Default::default(),
            }
        }
    }

    struct KeyListenerVP<E, V, F>
        where E: Environment,
              V: ViewProvider<E>,
              F: Fn(&str, EventModifiers, MSlock) + 'static
    {
        source: V,
        listener: F,
        window: Option<Weak<MainSlockCell<dyn WindowViewCallback<E>>>>,
        owner: Option<Weak<MainSlockCell<dyn InnerViewBase<E>>>>,
        phantom: PhantomData<E>
    }

    impl<E, V, F> ViewProvider<E> for KeyListenerVP<E, V, F>
        where E: Environment,
              V: ViewProvider<E>,
              F: Fn(&str, EventModifiers, MSlock) + 'static
    {
        type UpContext = V::UpContext;
        type DownContext = V::DownContext;

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
            self.source.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.source.up_context(s)
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            if let Some((nv, bs)) = backing_source {
                self.source.init_backing(invalidator, subtree, Some((nv, bs.source)), env, s)
            }
            else {
                self.source.init_backing(invalidator, subtree, None, env, s)
            }
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.window.is_none() {
                self.window = subtree.window();
                let owner = Arc::downgrade(subtree.owner());
                self.owner = Some(owner.clone());
                if let Some(w) = self.window.as_ref().and_then(|w| w.upgrade()) {
                    w.borrow_main(s)
                        .request_key_listener(owner);
                }
            }
            self.source.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            self.source.layout_down(subtree, frame, layout_context, env, s)
        }

        fn finalize_frame(&self, frame: Rect, s: MSlock) {
            self.source.finalize_frame(frame, s);
        }

        fn pre_show(&mut self, s: MSlock) {
            if let Some(w) = self.window.as_ref().and_then(|w| w.upgrade()) {
                w.borrow_main(s)
                    .request_key_listener(self.owner.clone().unwrap());
            }
            self.source.pre_show(s)
        }

        fn post_show(&mut self, s: MSlock) {
            self.source.post_show(s)
        }

        fn pre_hide(&mut self, s: MSlock) {
            self.source.pre_hide(s)
        }

        fn post_hide(&mut self, s: MSlock) {
            self.source.post_hide(s);
            if let Some(w) = self.window.as_ref().and_then(|w| w.upgrade()) {
                w.borrow_main(s)
                    .unrequest_key_listener(self.owner.clone().unwrap());
            }
        }

        fn focused(&self, rel_depth: u32, s: MSlock) {
            self.source.focused(rel_depth, s)
        }

        fn unfocused(&self, rel_depth: u32, s: MSlock) {
            self.source.focused(rel_depth, s)
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.push_environment(env, s)
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.pop_environment(env, s)
        }

        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            let res = self.source.handle_event(e, s);
            if let EventPayload::Key(KeyEvent::Press(ref ke))  = e.payload {
                (self.listener)(ke.chars(), e.modifiers, s)
            }

            res
        }
    }

    pub trait KeyListener<E> : IntoViewProvider<E> where E: Environment {
        fn key_listener(self, listener: impl Fn(&str, EventModifiers, MSlock) + 'static) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
    }

    impl<E, I> KeyListener<E> for I
        where E: Environment,
              I: IntoViewProvider<E> {
        fn key_listener(self, listener: impl Fn(&str, EventModifiers, MSlock) + 'static) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            KeyListenerIVP {
                source: self,
                listener,
                phantom: Default::default(),
            }
        }
    }
}

mod cursor {
    use std::marker::PhantomData;

    use crate::core::{Environment, MSlock};
    use crate::native;
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, View, ViewProvider, ViewRef, WeakInvalidator};

    #[derive(Copy, Clone)]
    pub enum Cursor {
        Arrow = 0,
        Pointer = 1,
        IBeam = 2,
        HorizontalResize = 3,
        VerticalResize = 4,
    }

    pub struct CursorIVP<E, I> where E: Environment, I: IntoViewProvider<E> {
        source: I,
        cursor: Cursor,
        phantom: PhantomData<E>
    }

    impl<E, I> IntoViewProvider<E> for CursorIVP<E, I> where E: Environment, I: IntoViewProvider<E> {
        type UpContext = I::UpContext;
        type DownContext = I::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            CursorVP {
                source: self.source.into_view_provider(env, s).into_view(s),
                cursor: self.cursor,
            }
        }
    }

    pub struct CursorVP<E, P> where E: Environment, P: ViewProvider<E> {
        source: View<E, P>,
        cursor: Cursor
    }

    impl<E, P> ViewProvider<E> for CursorVP<E, P> where E: Environment, P: ViewProvider<E> {
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
            self.source.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.source.up_context(s)
        }

        fn init_backing(&mut self, _invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {

            if let Some((nv, src)) = backing_source {
                self.source.take_backing(src.source, env, s);
                subtree.push_subview(&self.source, env, s);

                native::view::cursor::update_cursor_view(nv.backing(), self.cursor);
                nv
            }
            else {
                subtree.push_subview(&self.source, env, s);
                unsafe {
                    NativeView::new(native::view::cursor::init_cursor_view(self.cursor, s), s)
                }
            }
        }

        fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) -> bool {
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let used = self.source.layout_down_with_context(frame.full_rect(), layout_context, env, s);
            (used, used)
        }
    }

    pub trait CursorModifiable<E>: IntoViewProvider<E> where E: Environment {
        /// TODO this is currently platform dependent
        /// as on qt it will be cursor for entire subtree
        /// but for cocoa it's only this view
        fn cursor(self, cursor: Cursor) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
    }

    impl<E, I> CursorModifiable<E> for I where E: Environment, I: IntoViewProvider<E> {
        fn cursor(self, cursor: Cursor) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            CursorIVP {
                source: self,
                cursor,
                phantom: Default::default(),
            }
        }
    }
}
