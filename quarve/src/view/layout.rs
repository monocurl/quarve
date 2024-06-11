mod general_layout {
    use std::marker::PhantomData;
    use crate::core::{Environment, MSlock};
    use crate::native;
    use crate::state::slock_cell::{MainSlockCell};
    use crate::util::geo::{AlignedFrame, Rect, Size};
    use crate::view::{EnvHandle, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};

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
            env: &mut EnvHandle<E>,
            s: MSlock
        );

        fn layout_up(
            &mut self,
            subtree: &mut Subtree<E>,
            env: &mut EnvHandle<E>,
            s: MSlock
        ) -> bool;

        fn layout_down(
            &mut self,
            subtree: &Subtree<E>,
            frame: AlignedFrame,
            layout_context: &Self::DownContext,
            env: &mut EnvHandle<E>,
            s: MSlock
        ) -> Rect;
    }

    pub struct LayoutViewProvider<E, L>(L, PhantomData<MainSlockCell<E>>) where E: Environment, L: LayoutProvider<E>;

    unsafe impl<E, L> ViewProvider<E> for LayoutViewProvider<E, L>
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

        fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvHandle<E>, s: MSlock<'_>) -> NativeView {
            if let Some(source) = backing_source {
                self.0.init(invalidator, subtree, Some(source.1.0), env, s);

                source.0
            } else {
                self.0.init(invalidator, subtree, None, env, s);

                NativeView::new(native::view::init_layout_view(s))
            }
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvHandle<E>, s: MSlock<'_>) -> bool {
            self.0.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedFrame, layout_context: &Self::DownContext, env: &mut EnvHandle<E>, s: MSlock<'_>) -> Rect {
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
    use crate::view::{EnvHandle, IntoViewProvider, View, ViewProvider};

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

    pub trait VecLayoutProvider<E>: 'static where E: Environment {
        type Options: Default;
        type DownContext: 'static;
        type UpContext: 'static;
        type SubviewDownContext: 'static;
        type SubviewUpContext: 'static;

        fn from_options(options: Self::Options) -> Self;
        fn options(&mut self) -> &mut Self::Options;

        fn intrinsic_size(&mut self, s: MSlock) -> Size;
        fn xsquished_size(&mut self, s: MSlock) -> Size;
        fn ysquished_size(&mut self, s: MSlock) -> Size;
        fn xstretched_size(&mut self, s: MSlock) -> Size;
        fn ystretched_size(&mut self, s: MSlock) -> Size;

        fn up_context(&mut self, s: MSlock) -> Self::UpContext;

        fn layout_up<'a, P>(
            &mut self,
            subviews: impl Iterator<Item=&'a View<E, P>>,
            env: &mut EnvHandle<E>,
            s: MSlock
        ) -> bool where P: ViewProvider<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext>;

        fn layout_down<'a, P>(
            &mut self,
            subviews: impl Iterator<Item=&'a View<E, P>>,
            frame: AlignedFrame,
            context: &Self::DownContext,
            env: &mut EnvHandle<E>,
            s: MSlock
        ) -> Rect where P: ViewProvider<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext>;
    }

    mod macros {
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
                    VecSignalLayout::new(self, map, <$t as VecLayoutProvider<E>>::from_options(<$t as VecLayoutProvider<E>>::Options::default()))
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
                type E = $env;
                impl_signal_layout_extension!(__declare_trait  $t, $trait_name, $method_name);

                impl<T, S> $trait_name<T, S, E> for S where T: Send + 'static, S: Signal<Vec<T>>
                {
                    impl_signal_layout_extension!(__impl_trait $t, $trait_name, $method_name);
                }
            }
        }

        pub use impl_signal_layout_extension;
    }

    // mod hetero_layout {
    //     use crate::core::Environment;
    //     use crate::view::layout::VecLayoutProvider;
    //
    //     pub struct HeterogenousLayout<E, P>
    //         where E: Environment, P: VecLayoutProvider<E> {
    //
    //         providers: Vec<Box<
    //             dyn ViewProvider<E, DownContext=P::SubviewDownContext, UpContext=P::UpContext>
    //         >>
    //     }
    //
    //     struct HeterogenousViewProvider<E, P>
    //         where E: Environment, P: VecLayoutProvider<E> {
    //         view: Vec<View<E, Box<
    //             dyn ViewProvider<E, DownContext=P::SubviewDownContext, UpContext=P::UpContext>
    //         >>>
    //     }
    //     // up to 12 via macros
    // }
    //

    // mod binding_layout {
    //     use std::marker::PhantomData;
    //     use crate::core::{Environment, MSlock};
    //     use crate::state::{Binding, Buffer, StoreContainer, VecActionBasis, Word};
    //     use crate::util::geo::{AlignedFrame, Rect, Size};
    //     use crate::view::{EnvHandle, IntoUpContext, IntoViewProvider, Invalidator, Subtree, UpContextAdapter, View, ViewProvider};
    //     use crate::view::layout::VecLayoutProvider;
    //
    //     pub struct VecBindingLayout<E, S, B, M, U, P, L>
    //         where E: Environment,
    //               S: StoreContainer,
    //               B: Binding<Vec<S>>,
    //               M: Fn(&S) -> P + 'static,
    //               U: IntoUpContext<L::SubviewUpContext>,
    //               P: ViewProvider<E,
    //                   DownContext=L::SubviewDownContext,
    //                   UpContext=U
    //               >,
    //               L: VecLayoutProvider<E>
    //     {
    //         binding: B,
    //         layout: L,
    //         map: M,
    //         // everything is static so dont care about variacne too much
    //         provider: PhantomData<fn(S) -> P>,
    //         env: PhantomData<E>,
    //         store: PhantomData<S>,
    //         context: PhantomData<U>,
    //     }
    //
    //     pub struct VecBindingViewProvider<E, S, B, M, U, P, L>
    //         where E: Environment,
    //               S: StoreContainer,
    //               B: Binding<Vec<S>>,
    //               M: Fn(&S) -> P + 'static,
    //               U: IntoUpContext<L::SubviewUpContext>,
    //               P: ViewProvider<E,
    //                   DownContext=L::SubviewDownContext,
    //                   UpContext=U
    //               >,
    //               L: VecLayoutProvider<E>
    //     {
    //         binding: B,
    //         layout: L,
    //         map: M,
    //         action_buffer: Buffer<Word<VecActionBasis<()>>>,
    //         subviews: Vec<View<E, UpContextAdapter<E, P, L::SubviewUpContext>>>,
    //         // everything is static so dont care about variacne too much
    //         provider: PhantomData<fn(S) -> P>,
    //         env: PhantomData<E>,
    //         store: PhantomData<S>,
    //         context: PhantomData<U>,
    //     }
    //
    //     impl<E, S, B, M, U, P, L> IntoViewProvider<E> for VecBindingLayout<E, S, B, M, U, P, L>
    //         where E: Environment,
    //               S: StoreContainer,
    //               B: Binding<Vec<S>>,
    //               M: Fn(&S) -> P + 'static,
    //               U: IntoUpContext<L::SubviewUpContext>,
    //               P: ViewProvider<E,
    //                   DownContext=L::SubviewDownContext,
    //                   UpContext=U
    //               >,
    //               L: VecLayoutProvider<E> {
    //         type UpContext = ();
    //         type DownContext = ();
    //
    //         fn into_view_provider(self, _env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
    //             VecBindingViewProvider {
    //                 binding: self.binding,
    //                 layout: self.layout,
    //                 map: self.map,
    //                 action_buffer: Buffer::new(Word::default()),
    //                 subviews: vec![],
    //                 provider: Default::default(),
    //                 env: Default::default(),
    //                 store: Default::default(),
    //                 context: Default::default(),
    //             }
    //         }
    //     }
    //
    //     unsafe impl<E, S, B, M, U, P, L> ViewProvider<E> for VecBindingViewProvider<E, S, B, M, U, P, L>
    //         where E: Environment,
    //               S: StoreContainer,
    //               B: Binding<Vec<S>>,
    //               M: Fn(&S) -> P + 'static,
    //               U: IntoUpContext<L::SubviewUpContext>,
    //               P: ViewProvider<E,
    //                   DownContext=L::SubviewDownContext,
    //                   UpContext=U
    //               >,
    //               L: VecLayoutProvider<E> {
    //         type UpContext = L::UpContext;
    //         type DownContext = L::DownContext;
    //
    //         fn intrinsic_size(&mut self, s: MSlock) -> Size {
    //             self.layout.intrinsic_size(s)
    //         }
    //
    //         fn xsquished_size(&mut self, s: MSlock) -> Size {
    //             self.layout.xsquished_size(s)
    //         }
    //
    //         fn ysquished_size(&mut self, s: MSlock) -> Size {
    //             self.layout.ysquished_size(s)
    //         }
    //
    //         fn xstretched_size(&mut self, s: MSlock) -> Size {
    //             self.layout.xstretched_size(s)
    //         }
    //
    //         fn ystretched_size(&mut self, s: MSlock) -> Size {
    //             self.layout.ystretched_size(s)
    //         }
    //
    //         fn up_context(&mut self, s: MSlock) -> Self::UpContext {
    //             self.layout.up_context(s)
    //         }
    //
    //         fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvHandle<E>, s: MSlock<'_>) -> NativeView where Self: Sized {
    //             // register invalidator for binding
    //             let buffer = self.action_buffer.weak_buffer();
    //             self.binding.action_listener(|_, a, s| {
    //                 let Some(invalidator) = invalidator.upgrade() else {
    //                     return false;
    //                 };
    //                 let Some(buffer) = buffer.upgrade() else {
    //                     return false;
    //                 };
    //
    //                 for a in a.iter() {
    //                     let mapped = match a {
    //                         VecActionBasis::Insert(_, at) => {
    //                             // Vec
    //                         }
    //                         VecActionBasis::Remove(_) => {}
    //                         VecActionBasis::InsertMany(_, _) => {}
    //                         VecActionBasis::RemoveMany(_) => {}
    //                         VecActionBasis::Swap(_, _) => {}
    //                     }
    //                 }
    //
    //                 invalidator.invalidate(s);
    //                 true
    //             }, s);
    //             // if let Some(source) = invalidator.
    //             todo!()
    //         }
    //
    //         fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvHandle<E>, s: MSlock<'_>) -> bool {
    //             // FIXME we should be able to derive this from the action alone
    //             // this doesn't introduce additional complexity though i suppose at least
    //             let source_indices =
    //                 todo!()
    //         }
    //
    //         fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedFrame, layout_context: &Self::DownContext, env: &mut EnvHandle<E>, s: MSlock) -> Rect {
    //             // self.
    //         }
    //     }
    // }
    // pub use binding_layout::*;

    mod signal_layout {
        use std::marker::PhantomData;
        use crate::core::{Environment, MSlock};
        use crate::native;
        use crate::state::Signal;
        use crate::util::geo::{AlignedFrame, Rect, Size};
        use crate::view::{EnvHandle, IntoUpContext, IntoViewProvider, Invalidator, NativeView, Subtree, UpContextAdapter, View, ViewProvider};
        use crate::view::layout::{VecLayoutProvider};
        use crate::view::layout::vec_layout::into_view_provider;

        pub struct VecSignalLayout<E, T, S, M, U, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Vec<T>>,
                  M: FnMut(&T, MSlock) -> P + 'static,
                  U: IntoUpContext<L::SubviewUpContext>,
                  P: IntoViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=U
                  >,
                  L: VecLayoutProvider<E>
        {
            source: S,
            map: M,
            layout: L,
            phantom: PhantomData<(T, E, U, P)>
        }

        impl<E, T, S, M, U, P, L> VecSignalLayout<E, T, S, M, U, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Vec<T>>,
                  M: FnMut(&T, MSlock) -> P + 'static,
                  U: IntoUpContext<L::SubviewUpContext>,
                  P: IntoViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=U
                  >,
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
                  P: ViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=L::SubviewUpContext
                  >,
                  L: VecLayoutProvider<E>
        {
            source: S,
            map: M,
            layout: L,
            subviews: Vec<View<E, P>>,
            phantom: PhantomData<T>
        }

        impl<E, T, S, M, U, P, L> IntoViewProvider<E> for VecSignalLayout<E, T, S, M, U, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Vec<T>>,
                  M: FnMut(&T, MSlock) -> P + 'static,
                  U: IntoUpContext<L::SubviewUpContext>,
                  P: IntoViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=U
                  >,
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

        unsafe impl<E, T, S, M, P, L> ViewProvider<E> for VecSignalViewProvider<E, T, S, M, P, L>
            where E: Environment,
                  T: Send + 'static,
                  S: Signal<Vec<T>>,
                  M: FnMut(&T, &E::Const, MSlock) -> View<E, P> + 'static,
                  P: ViewProvider<E,
                      DownContext=L::SubviewDownContext,
                      UpContext=L::SubviewUpContext,
                  >,
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

            fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvHandle<E>, s: MSlock<'_>) -> NativeView {
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
                    NativeView::new(native::view::init_layout_view(s))
                }
            }

            fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvHandle<E>, s: MSlock<'_>) -> bool {
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

                self.layout.layout_up(self.subviews.iter(), env, s)
            }

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: AlignedFrame, layout_context: &Self::DownContext, env: &mut EnvHandle<E>, s: MSlock<'_>) -> Rect {
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
        use crate::view::layout::VecLayoutProvider;
        use crate::view::{EnvHandle, View, ViewProvider};
        use crate::view::util::SizeContainer;

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
        
        impl<E> VecLayoutProvider<E> for VStack where E: Environment {
            type Options = VStackOptions;
            type DownContext = ();
            type UpContext = ();
            type SubviewDownContext = ();
            type SubviewUpContext = ();

            fn from_options(options: Self::Options) -> Self {
                VStack(SizeContainer::default(), options)
            }

            fn options(&mut self) -> &mut Self::Options {
                &mut self.1
            }

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

            fn layout_up<'a, P>(&mut self, subviews: impl Iterator<Item=&'a View<E, P>>, _env: &mut EnvHandle<E>, s: MSlock) -> bool where P: ViewProvider<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> {
                let mut new = SizeContainer::default();
                let mut first = true;
                for view in subviews {
                    let sizes = view.sizes(s);
                    for i in 0..SizeContainer::num_sizes() {
                        new[i].w = new[i].w.max(sizes[i].w);
                        new[i].h += sizes[i].h;
                        if first {
                            new[i].h += self.1.spacing;
                        }
                    }
                    first = false;
                }

                if new != self.0 {
                    self.0 = new;
                    true
                }
                else {
                    false
                }
            }

            fn layout_down<'a, P>(&mut self, subviews: impl Iterator<Item=&'a View<E, P>>, frame: AlignedFrame, context: &Self::DownContext, env: &mut EnvHandle<E>, s: MSlock) -> Rect where P: ViewProvider<E, DownContext=Self::SubviewDownContext, UpContext=Self::SubviewUpContext> {
                let mut elapsed = 0.0;
                for view in subviews {
                    let intrinsic = view.intrinsic_size(s);
                    view.layout_down(AlignedFrame::new_from_size(intrinsic, Alignment::Center), Point::new(0.0, elapsed), env, s);
                    elapsed += intrinsic.h + self.1.spacing;
                }
                frame.full_rect()
            }
        }

        mod impls {
            use crate::state::Signal;
            use crate::core::Environment;
            use crate::view::IntoViewProvider;
            use crate::core::MSlock;
            use crate::impl_signal_layout_extension;
            use crate::view::layout::{VecSignalLayout, VecLayoutProvider};
            use super::VStack;

            impl_signal_layout_extension!(VStack, SignalVMap, signal_vmap, where E: Environment);
        }
        pub use impls::*;
    }
    pub use vstack::*;

    mod hstack {
        pub struct HStack {}
    }
    pub use hstack::*;

    mod zstack {
        pub struct ZStack {}
    }
    pub use zstack::*;
}
pub use vec_layout::*;