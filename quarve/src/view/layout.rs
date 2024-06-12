mod general_layout {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::state::slock_cell::{MainSlockCell};
    use crate::util::geo::{AlignedFrame, Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};

    pub trait LayoutProvider<E>: Sized + 'static where E: Environment {
        type DownContext: 'static;
        type UpContext: 'static;

        fn into_layout_view_provider(self) -> LayoutViewProvider<E, Self> {
            LayoutViewProvider(self, PhantomData)
        }

        fn intrinsic_size(&mut self, s: MSlock) -> Size;

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.intrinsic_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.intrinsic_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.intrinsic_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.intrinsic_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext;

        fn init(
            &mut self,
            invalidator: Invalidator<E>,
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
            frame: AlignedFrame,
            layout_context: &Self::DownContext,
            env: &mut EnvRef<E>,
            s: MSlock
        ) -> Rect;
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

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock<'_>) -> NativeView {
            if let Some(source) = backing_source {
                self.0.init(invalidator, subtree, Some(source.1.0), env, s);

                source.0
            } else {
                self.0.init(invalidator, subtree, None, env, s);

                NativeView::layout_view(s)
            }
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock<'_>) -> bool {
            self.0.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedFrame, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock<'_>) -> Rect {
            self.0.layout_down(subtree, frame, layout_context, env, s)
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
pub use general_layout::*;

mod vec_layout {
    use crate::core::{Environment, MSlock};
    use crate::util::geo::{AlignedFrame, Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, ViewProvider, ViewRef};
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

    pub trait FromOptions {
        type Options: Default;

        fn from_options(options: Self::Options) -> Self;
        fn options(&mut self) -> &mut Self::Options;
    }

    pub trait VecLayoutProvider<E>: FromOptions + 'static where E: Environment {
        type DownContext: 'static;
        type UpContext: 'static;
        type SubviewDownContext: 'static;
        type SubviewUpContext: 'static;

        fn intrinsic_size(&mut self, s: MSlock) -> Size;
        fn xsquished_size(&mut self, s: MSlock) -> Size;
        fn ysquished_size(&mut self, s: MSlock) -> Size;
        fn xstretched_size(&mut self, s: MSlock) -> Size;
        fn ystretched_size(&mut self, s: MSlock) -> Size;

        fn up_context(&mut self, s: MSlock) -> Self::UpContext;

        fn layout_up<'a, P>(
            &mut self,
            subviews: impl Iterator<Item=&'a P>,
            env: &mut EnvRef<E>,
            s: MSlock
        ) -> bool where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a;

        fn layout_down<'a, P>(
            &mut self,
            subviews: impl Iterator<Item=&'a P>,
            frame: AlignedFrame,
            context: &Self::DownContext,
            env: &mut EnvRef<E>,
            s: MSlock
        ) -> Rect where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a;
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
                                quarve::view::layout::new_hetero_ivp(
                                    <$t as quarve::view::layout::FromOptions>::from_options(
                                        <$t as quarve::view::layout::FromOptions>::Options::default()
                                    )
                                )
                            };
                            ($d first: expr $d(; $d child: expr )* $d(;)?) => {
                                $macro_name! {
                                    $d($d child;)*
                                }
                                .prepend($d first)
                            }
                        }
                    }
                }
            };
        }
        pub use impl_hetero_layout;

        #[macro_export]
        macro_rules! impl_signal_layout_extension {
            (__declare_trait $t: ty, $trait_name: ident, $method_name: ident) => {
                pub trait $trait_name<T, S, E> where T: Send + 'static, S: Signal<Vec<T>>, E: Environment {
                    fn $method_name<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static)
                        -> impl IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                        where P: IntoViewProvider<E,
                                        DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                        UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>;
                }
            };
            (__impl_trait $t: ty, $trait_name: ident, $method_name: ident) => {
                fn $method_name<P>(self, map: impl FnMut(&T, MSlock) -> P + 'static)
                    -> impl IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::DownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::UpContext>
                    where P: IntoViewProvider<E,
                                    DownContext=<$t as VecLayoutProvider<E>>::SubviewDownContext,
                                    UpContext=<$t as VecLayoutProvider<E>>::SubviewUpContext>
                {
                    VecSignalLayout::new(self, map, <$t as FromOptions>::from_options(<$t as FromOptions>::Options::default()))
                }
            };

            ($t: ty, $trait_name: ident, $method_name: ident, where E: $env: path) => {
                impl_signal_layout_extension!(__declare_trait  $t, $trait_name, $method_name);

                impl<E, T, S> $trait_name<T, S, E> for S where T: Send + 'static, S: Signal<Vec<T>>, E: $env
                {
                    impl_signal_layout_extension!(__impl_trait $t, $trait_name, $method_name);
                }
            };
            ($t: ty, $trait_name: ident, $method_name: ident, where E = $env: ty) => {
                mod {
                    type E = $env;
                    impl_signal_layout_extension!(__declare_trait  $t, $trait_name, $method_name);

                    impl<T, S> $trait_name<T, S, E> for S where T: Send + 'static, S: Signal<Vec<T>>
                    {
                        impl_signal_layout_extension!(__impl_trait $t, $trait_name, $method_name);
                    }
                }
            }
        }

        pub use impl_signal_layout_extension;
    }

    // FIXME could make more organized
    mod hetero_layout {
        use std::marker::PhantomData;
        use crate::core::{Environment, MSlock};
        use crate::util::geo::{AlignedFrame, Point, Rect, Size};
        use crate::view::{DummyProvider, EnvRef, IntoUpContext, IntoViewProvider, Invalidator, NativeView, Subtree, UpContextAdapter, View, ViewProvider, ViewRef};
        use crate::view::layout::{VecLayoutProvider};
        use crate::view::util::SizeContainer;

        pub trait HeteroIVPNode<E, U, D> where E: Environment, U: 'static, D: 'static {
            fn into_layout(self, env: &E::Const, s: MSlock) -> impl HeteroVPNode<E, U, D>;
        }

        trait HeteroVPNodeBase<E, U, D>: 'static where E: Environment, U: 'static, D: 'static {
            fn next(&self) -> &dyn HeteroVPNodeBase<E, U, D>;
            fn view(&self) -> Option<&dyn ViewRef<E, UpContext=U, DownContext=D>>;
        }

        trait HeteroVPNode<E, U, D>: HeteroVPNodeBase<E, U, D> where E: Environment, U: 'static, D: 'static
        {
            fn push_subviews(&self, tree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock);
            fn take_backing(&mut self, from: Self, env: &mut EnvRef<E>, s: MSlock);
        }

        struct NullNode;
        impl<E: Environment, U: 'static, D: 'static> HeteroIVPNode<E, U, D> for NullNode {
            fn into_layout(self, env: &E::Const, s: MSlock) -> impl HeteroVPNode<E, U, D> {
                 NullNode
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
                  P::UpContext: IntoUpContext<U>,
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
                  P::UpContext: IntoUpContext<U>,
                  N: HeteroIVPNode<E, U, D>,
                  U: 'static, D: 'static
        {
            fn into_layout(self, env: &E::Const, s: MSlock) -> impl HeteroVPNode<E, U, D> {
                HeteroVPActualNode {
                    next: self.next.into_layout(env, s),
                    view:
                    UpContextAdapter::new(
                        self.provider.into_view_provider(env, s)
                    ).into_view(s),
                    phantom: PhantomData
                }
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

        struct HeteroVPIterator<'a, E, L>(&'a dyn HeteroVPNodeBase<E, L::SubviewUpContext, L::SubviewDownContext>)
            where E: Environment,
                  L: VecLayoutProvider<E>;

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

        pub fn new_hetero_ivp<E: Environment, L: VecLayoutProvider<E>>(layout: L) -> HeteroIVP<E, impl HeteroIVPNode<E, L::SubviewUpContext, L::SubviewDownContext>, L> {
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

            pub fn prepend<P>(self, provider: P) -> HeteroIVP<E, impl HeteroIVPNode<E, L::SubviewUpContext, L::SubviewDownContext>, L>
                where P: IntoViewProvider<E, UpContext=L::SubviewUpContext, DownContext=L::SubviewDownContext>
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
                    root: self.root.into_layout(env, s),
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

            fn init_backing(&mut self, _invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
                self.root.push_subviews(subtree, env, s);

                if let Some(source) = backing_source {
                    self.root.take_backing(source.1.root, env, s);

                    source.0
                }
                else {
                    NativeView::layout_view(s)
                }
            }

            fn layout_up(&mut self, _subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock<'_>) -> bool {
                let iterator: HeteroVPIterator<E, L> = HeteroVPIterator(&self.root);

                self.layout.layout_up(iterator, env, s)
            }

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: AlignedFrame, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock<'_>) -> Rect {
                let iterator: HeteroVPIterator<E, L> = HeteroVPIterator(&self.root);

                self.layout.layout_down(iterator, frame, layout_context, env, s)
            }
        }
    }
    pub use hetero_layout::*;

    mod signal_layout {
        use std::marker::PhantomData;
        use crate::core::{Environment, MSlock};
        use crate::state::Signal;
        use crate::util::geo::{AlignedFrame, Rect, Size};
        use crate::view::{EnvRef, IntoUpContext, IntoViewProvider, Invalidator, NativeView, Subtree, UpContextAdapter, View, ViewProvider};
        use crate::view::layout::{VecLayoutProvider};
        use crate::view::layout::vec_layout::into_view_provider;

        pub struct VecSignalLayout<E, T, S, M, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Vec<T>>,
                  M: FnMut(&T, MSlock) -> P + 'static,
                  P: IntoViewProvider<E, DownContext=L::SubviewDownContext>,
                  P::UpContext: IntoUpContext<L::SubviewUpContext>,
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
                  S: Signal<Vec<T>>,
                  M: FnMut(&T, MSlock) -> P + 'static,
                  P: IntoViewProvider<E, DownContext=L::SubviewDownContext>,
                  P::UpContext: IntoUpContext<L::SubviewUpContext>,
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
                  S: Signal<Vec<T>>,
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
                  S: Signal<Vec<T>>,
                  M: FnMut(&T, MSlock) -> P + 'static,
                  P: IntoViewProvider<E, DownContext=L::SubviewDownContext>,
                  P::UpContext: IntoUpContext<L::SubviewUpContext>,
                  L: VecLayoutProvider<E>
        {
            type UpContext = L::UpContext;
            type DownContext = L::DownContext;

            fn into_view_provider(mut self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
                VecSignalViewProvider {
                    source: self.source,
                    map: move |a: &'_ T, env: &'_ E::Const, s: MSlock<'_>| {
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
                  S: Signal<Vec<T>>,
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

            fn init_backing(&mut self, invalidator: Invalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock<'_>) -> NativeView {
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

            fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock<'_>) -> bool {
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

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: AlignedFrame, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock<'_>) -> Rect {
                self.layout.layout_down(
                    self.subviews.iter(),
                    frame,
                    layout_context,
                    env,
                    s
                )
            }
        }
    }
    pub use signal_layout::*;

    mod vstack {
        use crate::core::{Environment, MSlock};
        use crate::util::geo::{AlignedFrame, Alignment, Point, Rect, ScreenUnit, Size};
        use crate::view::layout::{FromOptions, VecLayoutProvider};
        use crate::view::{EnvRef, TrivialContextViewRef, ViewRef};
        use crate::view::util::SizeContainer;

        #[derive(Default)]
        pub struct VStack(SizeContainer, VStackOptions);

        pub struct VStackOptions {
            spacing: ScreenUnit
        }

        impl Default for VStackOptions {
            fn default() -> Self {
                VStackOptions {
                    spacing: 10.0
                }
            }
        }

        impl VStackOptions {
            pub fn spacing(mut self, spacing: ScreenUnit) -> Self {
                self.spacing = spacing;
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
                println!("Layout Up Called on vstack");
                let new = subviews
                    .map(|v| v.sizes(s))
                    .reduce(|mut new, curr| {
                        for i in 0..SizeContainer::num_sizes() {
                            new[i].w = new[i].w.max(curr[i].w);
                            new[i].h += curr[i].h + self.1.spacing;
                        }
                        new
                    })
                    .unwrap_or_default();

                if new != self.0 {
                    self.0 = new;
                    true
                }
                else {
                    false
                }
            }

            fn layout_down<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P>, frame: AlignedFrame, _context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> Rect
                where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a {
                let mut elapsed = 0.0;
                for view in subviews {
                    let intrinsic = view.intrinsic_size(s);
                    let used = view.layout_down(AlignedFrame::new_from_size(intrinsic, Alignment::Center), Point::new(0.0, elapsed), env, s);
                    elapsed += used.h + self.1.spacing;
                }
                frame.full_rect()
            }
        }
    }
    pub use vstack::*;

    mod hstack {
        use crate::core::{Environment, MSlock};
        use crate::util::geo::{AlignedFrame, Alignment, Point, Rect, ScreenUnit, Size};
        use crate::view::layout::{FromOptions, VecLayoutProvider};
        use crate::view::{EnvRef, TrivialContextViewRef, ViewRef};
        use crate::view::util::SizeContainer;

        pub struct HStack(SizeContainer, HStackOptions);

        pub struct HStackOptions {
            spacing: ScreenUnit
        }

        impl Default for HStackOptions {
            fn default() -> Self {
                HStackOptions {
                    spacing: 10.0
                }
            }
        }

        impl HStackOptions {
            pub fn spacing(mut self, spacing: ScreenUnit) -> Self {
                self.spacing = spacing;
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

            fn layout_up<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P>, _env: &mut EnvRef<E>, s: MSlock) -> bool
                where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a{
                let new = subviews
                    .map(|v| v.sizes(s))
                    .reduce(|mut new, curr| {
                        for i in 0..SizeContainer::num_sizes() {
                            new[i].h = new[i].h.max(curr[i].h);
                            new[i].w += curr[i].w;
                        }
                        new
                    })
                    .unwrap_or_default();

                if new != self.0 {
                    self.0 = new;
                    true
                }
                else {
                    false
                }
            }

            fn layout_down<'a, P>(&mut self, subviews: impl Iterator<Item=&'a P>, frame: AlignedFrame, _context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> Rect
                where P: ViewRef<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> + ?Sized + 'a {
                let mut elapsed = 0.0;
                for view in subviews {
                    let intrinsic = view.intrinsic_size(s);
                    view.layout_down(AlignedFrame::new_from_size(intrinsic, Alignment::Center), Point::new(elapsed, 0.0), env, s);
                    elapsed += intrinsic.w + self.1.spacing;
                }
                frame.full_rect()
            }
        }
    }
    pub use hstack::*;

    mod zstack {
        pub struct ZStack {}
    }
    pub use zstack::*;

    mod impls {
        use crate::state::Signal;
        use crate::core::Environment;
        use crate::view::IntoViewProvider;
        use crate::core::MSlock;
        use crate::{impl_hetero_layout, impl_signal_layout_extension};
        use crate::view::layout::{FromOptions, VecSignalLayout, VecLayoutProvider};
        use super::{VStack};

        impl_signal_layout_extension!(VStack, SignalVMap, signal_vmap, where E: Environment);

        impl_hetero_layout!(VStack, vstack);
        pub use vstack;

        impl_hetero_layout!(HStack, hstack);
        pub use hstack;
    }
    pub use impls::*;
}
pub use vec_layout::*;