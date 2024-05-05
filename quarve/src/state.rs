mod listener {
    use crate::core::Slock;
    use crate::state::Stateful;

    /* trait aliases */
    pub trait GeneralListener : FnMut(&Slock) -> bool + Send + 'static {}
    pub trait InverseListener : FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) -> bool + Send + 'static {}
    impl<T> GeneralListener for T where T: FnMut(&Slock) -> bool + Send + 'static {}
    impl<T> InverseListener for T where T: FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) -> bool + Send + 'static {}

    pub(super) type BoxInverseListener = Box<
        dyn FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) -> bool + Send
    >;

    pub(super) enum StateListener<S: Stateful> {
        ActionListener(Box<dyn (FnMut(&S::Action, &Slock) -> bool) + Send>),
        SignalListener(Box<dyn (FnMut(&S, &Slock) -> bool) + Send>),
        GeneralListener(Box<dyn FnMut(&Slock) -> bool + Send>),
    }
}
use listener::*;

mod group {
    use std::ops::Mul;
    use crate::state::{StoreContainer};


    pub trait Stateful: StoreContainer + 'static {
        type Action: GroupAction<Self>;
    }

    pub trait GroupAction<T>: Send + Sized + Mul<Output=Self> + 'static where T: Stateful {

        fn identity() -> Self;

        // returns inverse action
        fn apply(self, to: &mut T) -> Self;

        fn description(&self) -> &'static str {
            ""
        }
    }

    pub trait IntoAction<T> where T: Stateful {
        fn into(self, target: &T) -> T::Action;
    }

    impl<T> IntoAction<T> for T::Action where T: Stateful {
        fn into(self, _target: &T) -> T::Action {
            self
        }
    }

    mod filter {
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{Stateful};

        pub trait ActionFilter<S: Stateful>: 'static {
            fn new() -> Self;

            fn add_filter<F>(&mut self, f: F)
                where F: Send + 'static + Fn(S::Action, &Slock) -> S::Action;

            fn filter(&self, a: S::Action, s: &Slock<impl ThreadMarker>) -> S::Action;
        }

        pub struct Filter<S: Stateful>(
            Vec<Box<dyn Send + Fn(S::Action, &Slock) -> S::Action>>
        );

        pub struct Filterless();

        impl<S> ActionFilter<S> for Filterless where S: Stateful {
            fn new() -> Self {
                Filterless()
            }

            fn add_filter<F>(&mut self, _f: F) where F: Send + 'static + Fn(S::Action, &Slock) -> S::Action {

            }

            #[inline]
            fn filter(&self, a: S::Action, _s: &Slock<impl ThreadMarker>) -> S::Action {
                a
            }
        }

        impl<S> ActionFilter<S> for Filter<S> where S: Stateful {
            fn new() -> Self {
                Filter(Vec::new())
            }

            fn add_filter<F>(&mut self, f: F) where F: Send + 'static + Fn(S::Action, &Slock) -> S::Action {
                self.0.push(Box::new(f));
            }

            fn filter(&self, a: S::Action, s: &Slock<impl ThreadMarker>) -> S::Action {
                self.0
                    .iter()
                    .fold(a, |a, action| action(a, s.as_ref()))
            }
        }
    }
    pub use filter::*;

    mod action {
        use std::ops::Mul;
        use crate::state::{GroupAction, Stateful, StoreContainer};

        #[derive(Clone)]
        pub enum SetAction<T>
            where T: Stateful + Copy
        {
            Set(T),
            Identity
        }

        #[derive(Clone)]
        pub enum StringAction {
            // start, length, with
            ReplaceSubrange(usize, usize, String),
            ReplaceSubranges(Vec<(usize, usize, String)>)
        }

        #[derive(Clone)]
        pub enum VectorAction<T> {
            Replace(usize, usize, Option<T>),
            PermuteReplace(Vec<usize>, Vec<(usize, usize, Vec<T>)>),
        }

        impl<T> Mul for SetAction<T>
            where T: Stateful + Copy
        {
            type Output = Self;

            fn mul(self, rhs: Self) -> Self {
                match (self, rhs) {
                    (SetAction::Identity, rhs) => rhs,
                    (lhs, SetAction::Identity) => lhs,
                    (SetAction::Set(val), _) => SetAction::Set(val),
                }
            }
        }

        impl<T> GroupAction<T> for SetAction<T>
            where T: Stateful + Copy + 'static
        {
            fn identity() -> Self {
                SetAction::Identity
            }

            fn apply(self, to: &mut T) -> Self {
                match self {
                    SetAction::Identity => SetAction::Identity,
                    SetAction::Set(targ) => {
                        let ret = *to;
                        *to = targ;

                        SetAction::Set(ret)
                    },
                }
            }
        }

        impl Mul for StringAction {
            type Output = Self;

            fn mul(self, _rhs: Self) -> Self {
                todo!();
                /*
                match (self, rhs) {
                    (
                        StringAction::ReplaceSubrange(s, l, with),
                        StringAction::ReplaceSubrange(sp, lp, withp)
                    ) => {
                        /* if overlapping just a subrange */
                    }
                }
                 */
            }
        }

        impl GroupAction<String> for StringAction {
            fn identity() -> Self {
                Self::ReplaceSubrange(0, 0, "".to_owned())
            }

            fn apply(self, _to: &mut String) -> Self {
                match self {
                    StringAction::ReplaceSubrange(_, _end, _content) => {
                        Self::identity()
                    },
                    StringAction::ReplaceSubranges(_) => {
                        Self::identity()
                    }
                }
            }
        }


        impl<T> Mul for VectorAction<T> {
            type Output = Self;

            fn mul(self, _rhs: Self) -> Self {
                self
            }
        }

        impl<T> GroupAction<Vec<T>> for VectorAction<T>
            where T: 'static, T: StoreContainer
        {
            fn identity() -> Self {
                Self::Replace(0, 0, None)
            }

            fn apply(self, _to: &mut Vec<T>) -> Self {
                self
                /*
                match self {
                    VectorAction::Identity => VectorAction::Identity,
                    VectorAction::Set(targ) => {
                        let mut ret = VectorAction::Set(targ);
                        swap(&mut ret, to);

                        ret
                    },
                }
                 */
            }
        }
    }
    pub use action::*;

    mod stateful {
        use crate::core::{Slock, ThreadMarker};
        use super::action::{SetAction, VectorAction, StringAction};
        use crate::state::{GeneralListener, InverseListener, Stateful, StoreContainer};

        impl StoreContainer for bool {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for bool { type Action = SetAction<bool>; }

        impl StoreContainer for usize {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for usize { type Action = SetAction<Self>; }

        impl StoreContainer for u8 {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for u8 { type Action = SetAction<Self>; }

        impl StoreContainer for u16 {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for u16 { type Action = SetAction<Self>; }

        impl StoreContainer for u32 {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for u32 { type Action = SetAction<Self>; }

        impl StoreContainer for u64 {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for u64 { type Action = SetAction<Self>; }

        impl StoreContainer for i8 {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for i8 { type Action = SetAction<Self>; }

        impl StoreContainer for i16 {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for i16 { type Action = SetAction<Self>; }

        impl StoreContainer for i32 {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for i32 { type Action = SetAction<Self>; }

        impl StoreContainer for i64 {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for i64 { type Action = SetAction<Self>; }

        impl StoreContainer for String {
            fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }

            fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

            }
        }

        impl Stateful for String { type Action = StringAction; }

        impl<T> StoreContainer for Vec<T> where T: StoreContainer {
            fn subtree_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: GeneralListener + Clone {
                for container in self {
                    container.subtree_listener(f.clone(), s);
                }
            }

            fn inverse_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: InverseListener + Clone {
                for container in self {
                    container.inverse_listener(f.clone(), s);
                }
            }
        }

        impl<T> Stateful for Vec<T> where T: StoreContainer { type Action = VectorAction<T>; }
    }
    #[allow(unused_imports)]
    pub use stateful::*;
}
pub use group::*;

mod store {
    use std::ops::Deref;
    use crate::core::{Slock, ThreadMarker};
    use crate::state::{ActionFilter, Filterless, GeneralSignal, IntoAction,  Signal, Stateful};
    use crate::state::listener::{GeneralListener, InverseListener, StateListener};

    /// It is the implementors job to guarantee that subtree_listener
    /// and relatives do not get into call cycles
    pub trait StoreContainer: Send + Sized + 'static {
        fn subtree_listener<F: GeneralListener + Clone>(&self, f: F, s: &Slock<impl ThreadMarker>);

        fn inverse_listener<F: InverseListener + Clone>(&self, f: F, s: &Slock<impl ThreadMarker>);
    }

    pub trait Binding<S: Stateful, F: ActionFilter<S>=Filterless>: Signal<S> {
        fn apply(&self, action: impl IntoAction<S>, s: &Slock);
    }

    mod sealed_base {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::ops::Deref;
        use std::sync::Arc;
        use crate::core::{Slock, ThreadMarker};
        use super::{ActionFilter, Binding, GeneralSignal, IntoAction, Stateful};
        use super::{GeneralListener, InverseListener, StateListener};
        use super::StateRef;

        pub trait RawStoreBase<S, F>: 'static where S: Stateful, F: ActionFilter<S> {
            fn apply(inner: &Arc<RefCell<Self>>, action: impl IntoAction<S>, s: &Slock<impl ThreadMarker>);

            fn listen(&mut self, listener: StateListener<S>, s: &Slock<impl ThreadMarker>);

            fn subtree_listener(&mut self, f: impl GeneralListener + Clone, s: &Slock<impl ThreadMarker>);
            fn inverse_listener(&mut self, f: impl InverseListener + Clone, s: &Slock<impl ThreadMarker>);

            fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(S::Action, &Slock) -> S::Action + 'static;

            fn data(&self) -> &S;
        }

        /* IMO this is a bad side effect of rust's insistence on
           having no duplicate implementations. What could be done
           as impl<R: RawStore...> Binding for R now becomes an awkward
           derivation, with lots of duplicate code
         */
        pub trait RawStoreSharedOwnerBase<S: Stateful, F: ActionFilter<S>> : Sized + Binding<S, F> {
            type Inner: RawStoreBase<S, F>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>>;

            fn _action_listener(&self, f: Box<dyn FnMut(&S::Action, &Slock) -> bool + Send>, s: &Slock<impl ThreadMarker>) {
                self.get_ref().borrow_mut().listen(StateListener::ActionListener(f), s);
            }

            fn _borrow<'a>(&'a self, _s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=S> {
                StateRef {
                    main_ref: self.get_ref().borrow(),
                    lifetime: PhantomData,
                    filter: PhantomData,
                    inner: PhantomData,
                }
            }

            fn _listen<G>(&self, listener: G, s: &Slock<impl ThreadMarker>)
                where G: FnMut(&S, &Slock) -> bool + Send + 'static {
                self.get_ref().borrow_mut().listen(StateListener::SignalListener(Box::new(listener)), s)
            }

            fn _map<T, G>(&self, map: G, s: &Slock<impl ThreadMarker>) -> GeneralSignal<T>
                where T: Send + 'static, G: Send + 'static + Fn(&S) -> T {
                GeneralSignal::from(self, map, |this, listener, slock| {
                    this.get_ref().borrow_mut().listen(StateListener::SignalListener(listener), slock)
                }, s)
            }

            fn _apply(&self, action: impl IntoAction<S>, s: &Slock) {
                Self::Inner::apply(self.get_ref(), action, s);
            }
        }
    }

    mod raw_store {
        use crate::state::{ActionFilter, Stateful};
        use crate::state::store::sealed_base::RawStoreBase;

        pub trait RawStore<S, F>: RawStoreBase<S, F>
            where S: Stateful, F: ActionFilter<S> {}

        impl<S, F, R> RawStore<S, F> for R where S: Stateful, F: ActionFilter<S>, R: RawStoreBase<S, F> {

        }
    }
    pub use raw_store::*;

    mod raw_store_shared_owner {
        use crate::state::{ActionFilter, Stateful};
        use super::sealed_base::{RawStoreSharedOwnerBase};

        pub trait RawStoreSharedOwner<S, F>: RawStoreSharedOwnerBase<S, F>
            where S: Stateful, F: ActionFilter<S> {}
        impl<S, F, R> RawStoreSharedOwner<S, F> for R
            where S: Stateful, F: ActionFilter<S>, R: RawStoreSharedOwnerBase<S, F> {

        }
    }
    pub use raw_store_shared_owner::*;

    mod state_ref {
        use std::cell::Ref;
        use std::marker::PhantomData;
        use std::ops::Deref;
        use crate::state::{ActionFilter, Stateful};
        use crate::state::store::RawStore;

        pub(super) struct StateRef<'a, S, M, I>
            where S: Stateful, M: ActionFilter<S>, I: RawStore<S, M> {
            pub(super) main_ref: Ref<'a, I>,
            pub(super) lifetime: PhantomData<&'a S>,
            pub(super) filter: PhantomData<&'a M>,
            pub(super) inner: PhantomData<&'a I>
        }

        impl<'a, S, M, I> Deref for StateRef<'a, S, M, I>
            where S: Stateful, M: ActionFilter<S>, I: RawStore<S, M> {
            type Target = S;
            fn deref(&self) -> &S {
                self.main_ref.data()
            }
        }
    }
    use state_ref::*;

    mod bindable {
        use std::marker::PhantomData;
        use std::sync::Arc;
        use crate::state::{ActionFilter, Binding, GeneralBinding, Signal, Stateful};
        use crate::state::store::RawStoreSharedOwner;

        pub trait Bindable<S: Stateful, F: ActionFilter<S>> {
            type Binding: Binding<S, F> + Clone;

            fn binding(&self) -> Self::Binding;
            fn signal(&self) -> impl Signal<S> + Clone;
        }

        impl<S: Stateful, F: ActionFilter<S>, I: RawStoreSharedOwner<S, F>> Bindable<S, F> for I {
            type Binding = GeneralBinding<S, F, I::Inner>;

            fn binding(&self) -> Self::Binding {
                GeneralBinding {
                    inner: Arc::clone(&self.get_ref()),
                    phantom_state: PhantomData,
                    phantom_filter: PhantomData
                }
            }

            fn signal(&self) -> impl Signal<S> + Clone {
                self.binding()
            }
        }
    }
    pub use bindable::*;

    mod filterable {
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{Filter, RawStoreSharedOwner, Stateful};
        use super::sealed_base::RawStoreBase;

        pub trait Filterable<S: Stateful> {
            fn action_filter<G>(&mut self, filter: G, s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(S::Action, &Slock) -> S::Action + 'static;
        }

        impl<S: Stateful, I: RawStoreSharedOwner<S, Filter<S>>> Filterable<S> for I {
            fn action_filter<G>(&mut self, filter: G, s: &Slock<impl ThreadMarker>) where G: Send + Fn(S::Action, &Slock) -> S::Action + 'static {
                self.get_ref().borrow_mut().action_filter(filter, s);
            }
        }
    }
    pub use filterable::*;

    mod action_dispatcher {
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{ActionFilter, Stateful};
        use super::RawStoreSharedOwner;

        pub trait ActionDispatcher<S: Stateful, F: ActionFilter<S>> {
            fn action_listener<G>(&self, listener: G, s: &Slock<impl ThreadMarker>)
                where G: Send + FnMut(&S::Action, &Slock) -> bool + 'static;
        }

        impl<S: Stateful, F: ActionFilter<S>, I: RawStoreSharedOwner<S, F>> ActionDispatcher<S, F> for I {
            fn action_listener<G>(&self, listener: G, s: &Slock<impl ThreadMarker>)
                where G: Send + FnMut(&S::Action, &Slock) -> bool + 'static {
                self._action_listener(Box::new(listener), s);
            }
        }
    }
    pub use action_dispatcher::*;

    mod store {
        use std::cell::RefCell;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::{
            state::{
                 ActionFilter, Binding, BoxInverseListener, Filterless, IntoAction, Signal, Stateful, StoreContainer, GeneralSignal,
            },
            core::Slock,
            core::ThreadMarker,
        };
        use crate::state::{GroupAction};
        use crate::state::listener::{GeneralListener, InverseListener, StateListener};
        use crate::state::store::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        pub struct InnerStore<S: Stateful, F: ActionFilter<S>> {
            data: S,
            listeners: Vec<StateListener<S>>,
            inverse_listener: Option<BoxInverseListener>,
            filter: F,
        }

        impl<S, F> RawStoreBase<S, F> for InnerStore<S, F>
            where S: Stateful, F: ActionFilter<S>
        {
            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<S>, s: &Slock<impl ThreadMarker>) {
                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();
                let transaction = alt_action.into(&inner.data);

                let action = inner.filter.filter(transaction, s);
                let name = action.description();

                // tell action listeners
                inner.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::ActionListener(listener) => listener(&action, s.as_ref()),
                        _ => true
                    }
                );

                // apply action
                let inverse = action.apply(&mut inner.data);

                // tell signal and general listeners
                let data = &inner.data;
                inner.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::GeneralListener(action) => {
                            action(s.as_ref())
                        },
                        StateListener::SignalListener(action) => {
                            action(data, s.as_ref())
                        },
                        _ => true
                    }
                );

                // tell inverse listener
                if let Some(ref mut inv_listener) = inner.inverse_listener {
                    let state = Store { inner: arc.clone() };
                    let invert = move |s: &Slock| {
                        state.apply(inverse, s);
                    };
                    if !inv_listener(name, Box::new(invert), s.as_ref()) {
                        inner.inverse_listener = None;
                    }
                }
            }

            fn listen(&mut self, listener: StateListener<S>, _s: &Slock<impl ThreadMarker>) {
                self.listeners.push(listener);
            }

            fn subtree_listener(&mut self, f: impl GeneralListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.subtree_listener(f.clone(), s);
                self.listen(StateListener::GeneralListener(Box::new(f)), s);
            }

            fn inverse_listener(&mut self, f: impl InverseListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.inverse_listener(f.clone(), s);
                self.inverse_listener = Some(Box::new(f));
            }

            fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(S::Action, &Slock) -> S::Action + 'static {
                self.filter.add_filter(filter);
            }

            fn data(&self) -> &S {
                &self.data
            }
        }

        pub struct Store<S: Stateful, F: ActionFilter<S>=Filterless>
        {
            inner: Arc<RefCell<InnerStore<S, F>>>
        }

        impl<S, F> Store<S, F>
            where S: Stateful, F: ActionFilter<S>
        {
            pub fn new(initial: S) -> Self {
                Store {
                    inner: Arc::new(RefCell::new(InnerStore {
                        data: initial,
                        listeners: Vec::new(),
                        inverse_listener: None,
                        filter: F::new(),
                    }))
                }
            }
        }

        impl<S, F> Default for Store<S, F>
            where S: Stateful + Default, F: ActionFilter<S>
        {
            fn default() -> Self {
                Self::new(S::default())
            }
        }

        impl<S, M> StoreContainer for Store<S, M>
            where S: Stateful, M: ActionFilter<S>
        {
            fn subtree_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: GeneralListener + Clone {
                self.inner.borrow_mut().subtree_listener(f, s);
            }

            fn inverse_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: InverseListener + Clone {
                self.inner.borrow_mut().inverse_listener(f, s);
            }
        }

        impl<S, A> Signal<S> for Store<S, A>
            where S: Stateful, A: ActionFilter<S>
        {
            fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=S> {
                self._borrow(s)
            }

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>)
                where F: FnMut(&S, &Slock) -> bool + Send + 'static {
                self._listen(listener, s);
            }

            type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
            fn map<U, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U>
                where U: Send + 'static, F: Send + 'static + Fn(&S) -> U {
                self._map(map, s)
            }
        }

        impl<S, F> Binding<S, F> for Store<S, F>
            where S: Stateful, F: ActionFilter<S>
        {
            fn apply(&self, action: impl IntoAction<S>, s: &Slock) {
                self._apply(action, s);
            }
        }

        impl<S, F> RawStoreSharedOwnerBase<S, F> for Store<S, F>
            where S: Stateful, F: ActionFilter<S>
        {
            type Inner = InnerStore<S, F>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }
        }

        // safety: all accesses to inner are done using the global state lock
        // and Stateful: Send
        unsafe impl<S, F> Send for Store<S, F> where S: Stateful, F: ActionFilter<S> { }
        unsafe impl<S, F> Sync for Store<S, F> where S: Stateful, F: ActionFilter<S> { }
    }
    pub use store::*;

    mod token_store {
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::hash::Hash;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{ActionFilter, Binding, BoxInverseListener, Filter, Filterless, GeneralListener, GeneralSignal, GroupAction, IntoAction, InverseListener, Signal, Stateful, StoreContainer};
        use crate::state::listener::StateListener;
        use crate::state::store::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        pub struct InnerTokenStore<S: Stateful + Copy + Hash + Eq, F: ActionFilter<S>> {
            data: S,
            listeners: Vec<StateListener<S>>,
            equal_listeners: HashMap<S, Vec<Box<dyn FnMut(&S, &Slock) -> bool + Send>>>,
            inverse_listener: Option<BoxInverseListener>,
            filter: F
        }
        impl<S, F> RawStoreBase<S, F> for InnerTokenStore<S, F>
            where S: Stateful + Copy + Hash + Eq,
                  F: ActionFilter<S>
        {
            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<S>, s: &Slock<impl ThreadMarker>) {
                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();
                let transaction = alt_action.into(&inner.data);

                let action = inner.filter.filter(transaction, s);
                let name = action.description();

                // tell action listeners
                inner.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::ActionListener(listener) => {
                            listener(&action, s.as_ref())
                        },
                        _ => true
                    }
                );

                // apply action
                let old = inner.data;
                let inverse = action.apply(&mut inner.data);

                // tell signal and general listeners
                let data = &inner.data;
                inner.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::GeneralListener(action) => {
                            action(s.as_ref())
                        },
                        StateListener::SignalListener(action) => {
                            action(data, s.as_ref())
                        },
                        _ => true
                    }
                );

                // relevant equal listeners (old and new)
                let new = inner.data;
                if old != new {
                    for class in [old, new] {
                        inner.equal_listeners.entry(class)
                            .and_modify(|l|
                                l.retain_mut(|f| f(&new, s.as_ref()))
                            );
                    }
                }

                // tell inverse listener
                if let Some(ref mut inv_listener) = inner.inverse_listener {
                    let state = TokenStore { inner: arc.clone() };
                    let invert = move |s: &Slock| {
                        state.apply(inverse, s);
                    };
                    if !inv_listener(name, Box::new(invert), s.as_ref()) {
                        inner.inverse_listener = None;
                    }
                }
            }

            fn listen(&mut self, listener: StateListener<S>, _s: &Slock<impl ThreadMarker>) {
                self.listeners.push(listener);
            }

            fn subtree_listener(&mut self, f: impl GeneralListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.subtree_listener(f.clone(), s);
                self.listen(StateListener::GeneralListener(Box::new(f)), s);
            }

            fn inverse_listener(&mut self, f: impl InverseListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.inverse_listener(f.clone(), s);
                self.inverse_listener = Some(Box::new(f));
            }

            fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(S::Action, &Slock) -> S::Action + 'static {
                self.filter.add_filter(filter);
            }

            fn data(&self) -> &S {
                &self.data
            }
        }

        pub struct TokenStore<S, F=Filterless>
            where S: Stateful + Copy + Hash + Eq, F: ActionFilter<S> {
            inner: Arc<RefCell<InnerTokenStore<S, F>>>
        }

        impl<S, F> TokenStore<S, F> where S: Stateful + Copy + Hash + Eq, F: ActionFilter<S> {

            pub fn new(initial: S) -> Self {
                TokenStore {
                    inner: Arc::new(RefCell::new(InnerTokenStore {
                        data: initial,
                        listeners: Vec::new(),
                        inverse_listener: None,
                        equal_listeners: HashMap::new(),
                        filter: F::new(),
                    }))
                }
            }

            pub fn action_listener<G>(&self, listener: G, s: &Slock<impl ThreadMarker>)
                where G: Send + FnMut(&S::Action, &Slock) -> bool + 'static
            {
                self._action_listener(Box::new(listener), s);
            }

            pub fn equals(&self, target: S, s: &Slock) -> impl Signal<bool> + Clone {
                GeneralSignal::from(self, move |u| *u == target,
                    |this, listener, _s | {
                        this.inner.borrow_mut().equal_listeners.entry(target)
                            .or_insert(Vec::new())
                            .push(listener);
                    },
                    s
                )
            }
        }

        impl<S, F> Default for TokenStore<S, F>
            where S: Default + Stateful + Copy + Hash + Eq, F: ActionFilter<S> {
            fn default() -> Self {
                Self::new(S::default())
            }
        }

        impl<S> TokenStore<S, Filter<S>>
            where S: Stateful + Hash + Copy + Eq {

            pub fn action_filter<F>(&self, filter: F, s: &Slock<impl ThreadMarker>)
                where F: Send + Sync + Fn(S::Action, &Slock) -> S::Action + 'static {
                self.get_ref().borrow_mut().action_filter(filter, s);
            }
        }

        impl<S, M> StoreContainer for TokenStore<S, M>
            where S: Stateful + Copy + Hash + Eq, M: ActionFilter<S> {
            fn subtree_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: GeneralListener + Clone {
                self.inner.borrow_mut().subtree_listener(f, s);
            }

            fn inverse_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: InverseListener + Clone {
                self.inner.borrow_mut().inverse_listener(f, s);
            }
        }

        impl<S, A> Signal<S> for TokenStore<S, A>
            where S: Stateful + Copy + Hash + Eq, A: ActionFilter<S> {
            fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=S> {
                self._borrow(s)
            }

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>)
                where F: FnMut(&S, &Slock) -> bool + Send + 'static {
                self._listen(listener, s);
            }

            type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
            fn map<U, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U>
                where U: Send + 'static, F: Send + 'static + Fn(&S) -> U {
                self._map(map, s)
            }
        }

        impl<S, F> Binding<S, F> for TokenStore<S, F>
            where S: Stateful + Copy + Hash + Eq, F: ActionFilter<S> {
            fn apply(&self, action: impl IntoAction<S>, s: &Slock) {
                self._apply(action, s);
            }
        }

        impl<S, A> RawStoreSharedOwnerBase<S, A> for TokenStore<S, A>
            where S: Stateful + Copy + Hash + Eq, A: ActionFilter<S> {
            type Inner = InnerTokenStore<S, A>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }
        }

        // safety: all accesses to inner are done using the global state lock
        // and Stateful: Send
        unsafe impl<S, F> Send for TokenStore<S, F> where S: Stateful + Copy + Hash + Eq, F: ActionFilter<S> { }
        unsafe impl<S, F> Sync for TokenStore<S, F> where S: Stateful + Copy + Hash + Eq, F: ActionFilter<S> { }
    }
    pub use token_store::*;

    mod coupled_store {

    }
    pub use coupled_store::*;

    mod general_binding {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::ops::Deref;
        use std::sync::Arc;
        use crate::core::{Slock, ThreadMarker};
        use super::{ActionFilter, Binding, GeneralSignal, IntoAction, Signal, Stateful};
        use super::RawStore;
        use super::sealed_base::RawStoreSharedOwnerBase;

        pub struct GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<S>, I: RawStore<S, F> {
            pub(super) inner: Arc<RefCell<I>>,
            pub(super) phantom_state: PhantomData<S>,
            pub(super) phantom_filter: PhantomData<F>,
        }

        impl<S, F, I> Clone for GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<S>, I: RawStore<S, F> {
            fn clone(&self) -> Self {
                GeneralBinding {
                    inner: Arc::clone(&self.inner),
                    phantom_state: PhantomData,
                    phantom_filter: PhantomData
                }
            }
        }

        impl<S, A, I> Signal<S> for GeneralBinding<S, A, I>
            where S: Stateful, A: ActionFilter<S>, I: RawStore<S, A> {
            fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=S> {
                self._borrow(s)
            }

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>) where F: FnMut(&S, &Slock) -> bool + Send + 'static {
                self._listen(listener, s);
            }

            type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
            fn map<U, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U> where U: Send + 'static, F: Send + 'static + Fn(&S) -> U {
                self._map(map, s)
            }
        }

        impl<S, F, I> Binding<S, F> for GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<S>, I: RawStore<S, F> {
            fn apply(&self, action: impl IntoAction<S>, s: &Slock) {
                self._apply(action, s);
            }
        }

        impl<S, A, I> RawStoreSharedOwnerBase<S, A> for GeneralBinding<S, A, I>
            where S: Stateful, A: ActionFilter<S>, I: RawStore<S, A> {
            type Inner = I;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }
        }

        // Safety: all operations require the slock
        unsafe impl<S, F, I> Send for GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<S>, I: RawStore<S, F> {}
        unsafe impl<S, F, I> Sync for GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<S>, I: RawStore<S, F> {}
    }
    pub use general_binding::*;
}
pub use store::*;

mod signal {
    use std::ops::{Deref};
    use crate::core::{Slock, ThreadMarker};

    pub trait Signal<T: Send + 'static> : Send + Sync + 'static {
        fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=T>;

        fn listen<F>(&self, listener: F, _s: &Slock<impl ThreadMarker>)
            where F: (FnMut(&T, &Slock) -> bool) + Send + 'static;

        type MappedOutput<S: Send + 'static>: Signal<S>;
        fn map<S, F>(&self, map: F, _s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<S>
            where S: Send + 'static,
                  F: Send + 'static + Fn(&T) -> S;
    }

    trait InnerSignal<T: Send> {
        fn borrow(&self) -> &T;
    }

    mod signal_audience {
        use crate::core::{Slock, ThreadMarker};

        pub(super) struct SignalAudience<T: Send> {
            listeners: Vec<Box<dyn FnMut(&T, &Slock) -> bool + Send>>
        }

        impl<T> SignalAudience<T> where T: Send {
            pub(super) fn new() -> SignalAudience<T> {
                SignalAudience {
                    listeners: Vec::new()
                }
            }

            pub(super) fn listen<F>(&mut self, listener: F, _s: &Slock<impl ThreadMarker>) where F: (FnMut(&T, &Slock) -> bool) + Send + 'static {
                self.listeners.push(Box::new(listener));
            }

            pub(super) fn listen_box(
                &mut self,
                listener: Box<dyn (FnMut(&T, &Slock) -> bool) + Send + 'static>,
                _s: &Slock<impl ThreadMarker>
            ) {
                self.listeners.push(listener);
            }

            pub(super) fn dispatch(&mut self, new_val: &T, s: &Slock<impl ThreadMarker>) {
                self.listeners
                    .retain_mut(|listener| listener(new_val, s.as_ref()))
            }
        }
    }
    use signal_audience::*;

    mod signal_ref {
        use std::cell::Ref;
        use std::marker::PhantomData;
        use std::ops::Deref;
        use super::InnerSignal;

        pub(super) struct SignalRef<'a, T: Send, U: InnerSignal<T>> {
            pub(super) src: Ref<'a, U>,
            pub(super) marker: PhantomData<&'a T>
        }

        impl<'a, T, U> Deref for SignalRef<'a, T, U> where T: Send, U: InnerSignal<T> {
            type Target = T;

            fn deref(&self) -> &T {
                self.src.borrow()
            }
        }
    }
    use signal_ref::*;

    mod fixed_signal {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::ops::Deref;
        use std::sync::Arc;
        use crate::core::{Slock, ThreadMarker};
        use crate::state::Signal;
        use super::SignalRef;
        use super::InnerSignal;

        struct InnerFixedSignal<T: Send>(T);

        impl<T> InnerSignal<T> for InnerFixedSignal<T> where T: Send {
            fn borrow(&self) -> &T {
                &self.0
            }
        }

        pub struct FixedSignal<T: Send + 'static> {
            inner: Arc<RefCell<InnerFixedSignal<T>>>
        }

        impl<T> FixedSignal<T> where T: Send + 'static {
            pub fn new(val: T) -> FixedSignal<T> {
                FixedSignal {
                    inner: Arc::new(RefCell::new(InnerFixedSignal(val)))
                }
            }
        }

        impl<T> Clone for FixedSignal<T> where T: Send + 'static {
            fn clone(&self) -> Self {
                FixedSignal {
                    inner: self.inner.clone()
                }
            }
        }

        impl<T> Signal<T> for FixedSignal<T> where T: Send + 'static {
            fn borrow<'a>(&'a self, _s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=T> {
                SignalRef {
                    src: self.inner.borrow(),
                    marker: PhantomData
                }
            }

            fn listen<F>(&self, _listener: F, _s: &Slock<impl ThreadMarker>) where F: FnMut(&T, &Slock) -> bool + Send {
                /* no op */
            }

            type MappedOutput<S: Send + 'static> = FixedSignal<S>;
            fn map<S, F>(&self, map: F, _s: &Slock<impl ThreadMarker>) -> FixedSignal<S>
                where S: Send + 'static,
                      F: Send + 'static + Fn(&T) -> S
            {
                let inner = self.inner.borrow();
                let data = map(&inner.0);

                FixedSignal {
                    inner: Arc::new(RefCell::new(InnerFixedSignal(data)))
                }
            }
        }

        // Safety: all types require T to be Send
        // and can only be read or written to using the global state lock
        unsafe impl<T> Send for FixedSignal<T> where T: Send + 'static {}
        unsafe impl<T> Sync for FixedSignal<T> where T: Send + 'static {}
    }
    pub use fixed_signal::*;

    mod general_signal {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::core::{Slock, ThreadMarker};
        use crate::state::Signal;
        use super::SignalRef;
        use super::{InnerSignal, SignalAudience};

        struct GeneralInnerSignal<T: Send> {
            val: T,
            audience: SignalAudience<T>
        }

        impl<T> InnerSignal<T> for GeneralInnerSignal<T> where T: Send {
            fn borrow(&self) -> &T {
                &self.val
            }
        }

        struct GeneralSyncCell<T: Send>(RefCell<GeneralInnerSignal<T>>);

        pub struct GeneralSignal<T: Send + 'static> {
            inner: Arc<GeneralSyncCell<T>>
        }

        impl<T> Clone for GeneralSignal<T> where T: Send + 'static {
            fn clone(&self) -> Self {
                GeneralSignal {
                    inner: self.inner.clone()
                }
            }
        }

        impl<T> GeneralSignal<T> where T: Send + 'static {
            /// add listener is a function to help out generally handling
            /// TokenStore. Otherwise, .listen is used
            pub(crate) fn from<S, U, F, G>(source: &S, map: F, add_listener: G, s: &Slock<impl ThreadMarker>)
                -> GeneralSignal<T>
                where S: Signal<U>,
                      U: Send + 'static,
                      F: Send + 'static + Fn(&U) -> T,
                      G: FnOnce(&S, Box<dyn FnMut(&U, &Slock) -> bool + Send>, &Slock)
            {
                let inner;
                {
                    let val = source.borrow(s);
                    inner = GeneralInnerSignal {
                        val: map(&*val),
                        audience: SignalAudience::new(),
                    };
                }

                let arc = Arc::new(GeneralSyncCell(RefCell::new(inner)));
                let weak = Arc::downgrade(&arc);

                add_listener(source, Box::new(move |val, s| {
                    if let Some(arc) = weak.upgrade() {
                        let mut binding = arc.0.borrow_mut();
                        let inner = binding.deref_mut();
                        inner.val = map(val);
                        inner.audience.dispatch(&inner.val, s);
                        true
                    } else {
                        false
                    }
                }), s.as_ref());

                GeneralSignal {
                    inner: arc
                }
            }
        }

        impl<T> Signal<T> for GeneralSignal<T> where T: Send + 'static {
            fn borrow<'a>(&'a self, _s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=T> {
                SignalRef {
                    src: self.inner.0.borrow(),
                    marker: PhantomData,
                }
            }

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>)
                where F: FnMut(&T, &Slock) -> bool + Send + 'static {
                self.inner.0.borrow_mut().audience.listen(listener, s);
            }

            type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
            fn map<S, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> GeneralSignal<S>
                where S: Send + 'static, F: Fn(&T) -> S + Send + 'static {
                GeneralSignal::from(self, map, |this, listener, slock| {
                    this.inner.0.borrow_mut().audience.listen_box(listener, slock);
                }, s)
            }
        }

        unsafe impl<T> Send for GeneralSignal<T> where T: Send + 'static {}
        unsafe impl<T> Sync for GeneralSignal<T> where T: Send + 'static {}

        // safety: the refcell will only be accessed by a single thread at a time
        unsafe impl<T> Sync for GeneralSyncCell<T> where T: Send { }
    }
    pub use general_signal::*;

    mod joined_signal {
        use std::cell::RefCell;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{GeneralSignal, Signal};
        use crate::state::signal::InnerSignal;
        use crate::state::signal::signal_audience::SignalAudience;
        use crate::state::signal::signal_ref::SignalRef;

        struct JoinedInnerSignal<T, U, V>
            where T: Send + 'static,
                  U: Send + 'static,
                  V: Send + 'static
        {
            t: T,
            u: U,
            ours: V,
            audience: SignalAudience<V>
        }

        impl<T, U, V> InnerSignal<V> for JoinedInnerSignal<T, U, V>
            where T: Send + 'static,
                  U: Send + 'static,
                  V: Send + 'static
        {
            fn borrow(&self) -> &V {
                &self.ours
            }
        }

        struct JoinedSyncCell<T, U, V>(RefCell<JoinedInnerSignal<T, U, V>>)
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static;

        pub struct JoinedSignal<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {
            inner: Arc<JoinedSyncCell<T, U, V>>
        }

        impl<T, U, V> Clone for JoinedSignal<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {
            fn clone(&self) -> Self {
                JoinedSignal {
                    inner: self.inner.clone()
                }
            }
        }

        impl<T, U, V> JoinedSignal<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {
            pub fn from<F>(lhs: &impl Signal<T>, rhs: &impl Signal<U>, map: F, s: &Slock<impl ThreadMarker>)
                           -> JoinedSignal<T, U, V>
                where F: Send + Clone + 'static + Fn(&T, &U) -> V
            {
                let l = lhs.borrow(s);
                let r = rhs.borrow(s);

                let inner = JoinedInnerSignal {
                    t: l.clone(),
                    u: r.clone(),
                    ours: map(&*l, &*r),
                    audience: SignalAudience::new(),
                };
                drop(l);
                drop(r);

                let arc = Arc::new(JoinedSyncCell(RefCell::new(inner)));

                let weak = Arc::downgrade(&arc);
                let lhs_map = map.clone();
                lhs.listen(move |lhs, slock| {
                    if let Some(arc) = weak.upgrade() {
                        let mut binding = arc.0.borrow_mut();
                        let inner = binding.deref_mut();
                        inner.t = lhs.clone();
                        inner.ours = lhs_map(&inner.t, &inner.u);
                        inner.audience.dispatch(&inner.ours, slock);
                        true
                    } else {
                        false
                    }
                }, s);

                let weak = Arc::downgrade(&arc);
                rhs.listen(move |rhs, slock| {
                    if let Some(arc) = weak.upgrade() {
                        let mut binding = arc.0.borrow_mut();
                        let inner = binding.deref_mut();
                        inner.u = rhs.clone();
                        inner.ours = map(&inner.t, &inner.u);
                        inner.audience.dispatch(&inner.ours, slock);
                        true
                    } else {
                        false
                    }
                }, s);

                JoinedSignal {
                    inner: arc
                }
            }
        }

        impl<T, U, V> Signal<V> for JoinedSignal<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {
            fn borrow<'a>(&'a self, _s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=V> {
                SignalRef {
                    src: self.inner.0.borrow(),
                    marker: Default::default(),
                }
            }

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>) where F: FnMut(&V, &Slock) -> bool + Send + 'static {
                self.inner.0.borrow_mut().audience.listen(listener, s);
            }

            type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
            fn map<S, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> GeneralSignal<S>
                where S: Send + 'static,
                      F: Send + 'static + Fn(&V) -> S
            {
                GeneralSignal::from(self, map, |this, listener, slock| {
                    this.inner.0.borrow_mut().audience.listen_box(listener, slock);
                }, s)
            }
        }

        unsafe impl<T, U, V> Send for JoinedSignal<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {}
        unsafe impl<T, U, V> Sync for JoinedSignal<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {}

        unsafe impl<T, U, V> Sync for JoinedSyncCell<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {}
    }
    pub use joined_signal::*;
}
pub use signal::*;

#[cfg(test)]
mod test {
    use crate::core::{slock};
    use crate::state::{SetAction, Store, Signal, TokenStore, Binding, Bindable};
    use crate::state::SetAction::Set;

    #[test]
    fn test_numeric() {
        let c = slock();

        let s: Store<i32> = Store::new(2);
        let derived_sig;
        let derived_derived;
        {
            derived_sig = c.map(&s.signal(), |x| x * x);
            let b = c.read(&derived_sig);
            assert_eq!(*b, 4);
        }
        {
            derived_derived = derived_sig.map(|x| x - 4, &c);
        }

        c.apply(Set(6), &s);
        {
            let b = c.read(&derived_sig);
            assert_eq!(*b, 36);
            let b = c.read(&derived_derived);
            assert_eq!(*b, 32);
        }

        c.apply(SetAction::Identity *
                    Set(1),
                &s
        );
        {
            let b = c.read(&derived_sig);
            assert_eq!(*b, 1);
            let b = c.read(&derived_derived);
            assert_eq!(*b, -3);
        }

        let sig1;
        {
            let sig = c.fixed(-1);

            sig1 = Signal::map(&sig, |x| 2 * x, &c);
        }

        let b = sig1.borrow(&c);
        let c = *b;
        assert_eq!(c, -2);
    }


    #[test]
    fn test_join() {
        let s = slock();

        let x: Store<i32> = Store::new(3);
        let y: Store<bool> = Store::new(false);

        let join = s.join(&x.signal(), &y.signal());
        assert_eq!(*join.borrow(&s), (3, false));

        s.apply(Set(4), &x);
        assert_eq!(*join.borrow(&s), (4, false));

        s.apply(Set(true), &y);
        assert_eq!(*join.borrow(&s), (4, true));

        s.apply(Set(-1), &x);
        s.apply(Set(false), &y);
        assert_eq!(*join.borrow(&s), (-1, false));
    }

    #[test]
    fn test_join_map() {
        let s = slock();

        let x: Store<i32> = Store::new(3);
        let y: Store<bool> = Store::new(false);

        let join = s.join_map(&x.signal(), &y.signal(), |x, y|
            if *y {
                x + 4
            }
            else {
                x * x
            }
        );
        assert_eq!(*join.borrow(&s), 9);

        s.apply(Set(4), &x);
        assert_eq!(*join.borrow(&s), 16);

        s.apply(Set(true), &y);
        assert_eq!(*join.borrow(&s), 8);

        s.apply(Set(-1), &x);
        s.apply(Set(false), &y);
        assert_eq!(*join.borrow(&s), 1);

        drop(x);
        s.apply(Set(true), &y);
        assert_eq!(*join.borrow(&s), 3);
    }

    #[test]
    fn test_token_store() {
        let s = slock();
        let token: TokenStore<i32> = TokenStore::new(1);
        // let token = Store::new(1);

        let mut listeners = Vec::new();
        // a bit hacky since this testing scenario is rather awkward
        let counts: [Store<usize>; 10] = Default::default();
        for i in 0usize..10usize {
            let equals = token.equals(i as i32, &s);
            let c = counts[i].binding();
            equals.listen(move |_, s| {
                let curr = *c.borrow(s);

                c.apply(Set(curr + 1), s);
                true
            }, &s);
            listeners.push(equals);
        }
        assert_eq!(*counts[1].binding().borrow(&s), 0);
        token.apply(Set(1), &s);
        assert_eq!(*counts[1].binding().borrow(&s), 0);
        token.apply(Set(2), &s);
        assert_eq!(*counts[1].binding().borrow(&s), 1);
        assert_eq!(*counts[2].binding().borrow(&s), 1);
        token.apply(Set(4), &s);
        assert_eq!(*counts[1].binding().borrow(&s), 1);
        assert_eq!(*counts[2].binding().borrow(&s), 2);
        assert_eq!(*counts[4].binding().borrow(&s), 1);
        token.apply(Set(1), &s);
        assert_eq!(*counts[1].binding().borrow(&s), 2);
        assert_eq!(*counts[2].binding().borrow(&s), 2);
        assert_eq!(*counts[4].binding().borrow(&s), 2);
    }
}