use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Mul};
use std::sync::{Arc};

use crate::core::{Slock, ThreadMarker};

mod sealed {
    pub trait SealedBase<S> {

    }
}

/* trait aliases */
pub trait GeneralListener : FnMut(&Slock) -> bool + Send + 'static {}
pub trait InverseListener : FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) -> bool + Send + 'static {}
impl<T: FnMut(&Slock) -> bool + Send + 'static> GeneralListener for T {}
impl<T: FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) -> bool + Send + 'static> InverseListener for T {}


/// It is the implementors job to guarantee that subtree_listener
/// and relatives do not get into call cycles
pub trait StoreContainer: Send + Sized + 'static {
    fn subtree_listener<F: GeneralListener + Clone>(&self, f: F, s: &Slock<impl ThreadMarker>);

    fn inverse_listener<F: InverseListener + Clone>(&self, f: F, s: &Slock<impl ThreadMarker>);
}

pub trait Stateful: StoreContainer + 'static {
    type Action: GroupAction<Self>;
}

pub trait GroupAction<T: Stateful>: Send + Sized + Mul<Output=Self> + 'static {

    fn identity() -> Self;

    // returns inverse action
    fn apply(self, to: &mut T) -> Self;

    fn description(&self) -> &'static str {
        ""
    }
}

pub trait IntoAction<T: Stateful> {
    fn into(self, target: &T) -> T::Action;
}

pub trait ActionFilter<S: Stateful> : 'static {
    fn new() -> Self;
    
    fn add_filter<F>(&mut self, f: F)
        where F: Send + Sync + 'static + Fn(S::Action, &Slock) -> S::Action;

    fn filter(&self, a: S::Action, s: &Slock<impl ThreadMarker>) -> S::Action;
}

pub trait Signal<T: Send + 'static> : Send + Sync + 'static {
    fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=T>;

    fn listen<F: (Fn(&T, &Slock) -> bool) + Send + 'static>(&self, listener: F, _s: &Slock<impl ThreadMarker>);

    type MappedOutput<S: Send + 'static>: Signal<S>;
    fn map<S, F>(&self, map: F, _s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<S>
        where S: Send + 'static,
              F: Send + 'static + Fn(&T) -> S;
}

pub trait Binding<S: Stateful, F: ActionFilter<S>=Filterless>: Signal<S> {
    fn apply(&self, action: impl IntoAction<S>, s: &Slock);
}

impl<T: Stateful> IntoAction<T> for T::Action {
    fn into(self, _target: &T) -> T::Action {
        self
    }
}

pub struct Filter<S: Stateful>(
    Vec<Box<dyn Send + Sync + Fn(S::Action, &Slock) -> S::Action>>
);

pub struct Filterless();

impl<S: Stateful> ActionFilter<S> for Filterless {
    fn new() -> Self {
        Filterless()
    }

    fn add_filter<F>(&mut self, _f: F) where F: Send + Sync + 'static + Fn(S::Action, &Slock) -> S::Action {
        
    }

    #[inline]
    fn filter(&self, a: S::Action, _s: &Slock<impl ThreadMarker>) -> S::Action {
        a
    }
}

impl<S: Stateful> ActionFilter<S> for Filter<S> {
    fn new() -> Self {
        Filter(Vec::new())
    }

    fn add_filter<F>(&mut self, f: F) where F: Send + Sync + 'static + Fn(S::Action, &Slock) -> S::Action {
        self.0.push(Box::new(f));
    }

    fn filter(&self, a: S::Action, s: &Slock<impl ThreadMarker>) -> S::Action {
        self.0
            .iter()
            .fold(a, |a, action| action(a, s.as_ref()))
    }
}

trait RawStore<S: Stateful, F: ActionFilter<S>>: 'static {
    fn apply(inner: &Arc<RefCell<Self>>, action: impl IntoAction<S>, s: &Slock<impl ThreadMarker>);

    fn listen(&mut self, listener: StateListener<S>, s: &Slock<impl ThreadMarker>);

    fn subtree_listener(&mut self, f: impl GeneralListener + Clone, s: &Slock<impl ThreadMarker>);
    fn inverse_listener(&mut self, f: impl InverseListener + Clone, s: &Slock<impl ThreadMarker>);

    fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
        where G: Send + Sync + Fn(S::Action, &Slock) -> S::Action + 'static;

    fn data(&self) -> &S;
}


/* IMO this is a bad side effect of rust's insistence on
   having no duplicate implementations. What could be done
   as impl<R: RawStore...> Binding for R now becomes an awkward
   derivation, with lots of duplicate code
 */
trait RawStoreSharedOwner<S: Stateful, F: ActionFilter<S>> : Sized + Binding<S, F> {
    type Inner: RawStore<S, F>;
    fn get_ref(&self) -> &Arc<RefCell<Self::Inner>>;

    fn _action_listener(&self, f: Box<dyn FnMut(&S::Action, &Slock) -> bool + Send + Sync>, s: &Slock<impl ThreadMarker>) {
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
        GeneralSignal::from(self, map, s)
    }

    fn _apply(&self, action: impl IntoAction<S>, s: &Slock) {
        Self::Inner::apply(self.get_ref(), action, s);
    }
}


type BoxInverseListener = Box<
    dyn FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) -> bool + Send
>;

struct InnerStore<S: Stateful, F: ActionFilter<S>> {
    data: S,
    listeners: Vec<StateListener<S>>,
    inverse_listener: Option<BoxInverseListener>,
    filter: F,
}

struct InnerTokenStore<S: Stateful + Copy + Hash + Eq, F: ActionFilter<S>> {
    data: S,
    listeners: Vec<StateListener<S>>,
    equal_listeners: HashMap<S, Vec<Box<dyn FnMut(&S, &Slock) -> bool + Send>>>,
    inverse_listener: Option<BoxInverseListener>,
    filter: F
}

// struct InnerCoupledStore<F: Stateful, B: Stateful>
//     where F::Action : Clone,
//           B::Action : Clone
// {
//
// }

pub struct Store<S: Stateful, F: ActionFilter<S>=Filterless>
{
    inner: Arc<RefCell<InnerStore<S, F>>>
}

pub struct TokenStore<S, F=Filterless>
    where S: Stateful + Copy + Hash + Eq, F: ActionFilter<S> {
    inner: Arc<RefCell<InnerTokenStore<S, F>>>
}

// // pub struct CoupledStore<F: Stateful, B: Stateful, FF: ActionFilter<F>=Filterless, BF:ActionFilter<B>=Filterless>
// // {
// //     front: Arc<RefCell<InnerCoupledStore<F, FF>>>,
// //     back: Arc<RefCell<InnerCoupledStore<B, BF>>>
// }

// safety: all accesses to inner are done using the global state lock
// and Stateful: Send
unsafe impl<S: Stateful, F: ActionFilter<S>> Send for Store<S, F> { }
unsafe impl<S: Stateful, F: ActionFilter<S>> Sync for Store<S, F> { }
unsafe impl<S: Stateful + Copy + Hash + Eq, F: ActionFilter<S>> Send for TokenStore<S, F> { }
unsafe impl<S: Stateful + Copy + Hash + Eq, F: ActionFilter<S>> Sync for TokenStore<S, F> { }

pub struct StateRef<'a, S, M, I>
    where S: Stateful, M: ActionFilter<S>, I: RawStore<S, M> {
    main_ref: Ref<'a, I>,
    lifetime: PhantomData<&'a S>,
    filter: PhantomData<&'a M>,
    inner: PhantomData<&'a I>
}

struct GeneralBinding<S, F, I>
    where S: Stateful, F: ActionFilter<S>, I: RawStore<S, F> {
    inner: Arc<RefCell<I>>,
    phantom_state: PhantomData<S>,
    phantom_filter: PhantomData<F>,
}

// Safety: all operations require the slock
unsafe impl<S, F, I> Send for GeneralBinding<S, F, I>
    where S: Stateful, F: ActionFilter<S>, I: RawStore<S, F> {}
unsafe impl<S, F, I> Sync for GeneralBinding<S, F, I>
    where S: Stateful, F: ActionFilter<S>, I: RawStore<S, F> {}

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

    fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>) where F: Fn(&S, &Slock) -> bool + Send + 'static {
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


impl<S, I> GeneralBinding<S, Filter<S>, I>
    where S: Stateful, I: RawStore<S, Filter<S>> {
    fn action_filter<F>(&self, filter: F, s: &Slock<impl ThreadMarker>)
        where F: Send + Sync + Fn(S::Action, &Slock) -> S::Action + 'static {
        self.get_ref().borrow_mut().action_filter(filter, s);
    }
}

impl<S, A, I> RawStoreSharedOwner<S, A> for GeneralBinding<S, A, I>
    where S: Stateful, A: ActionFilter<S>, I: RawStore<S, A> {
    type Inner = I;

    fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
        &self.inner
    }
}

impl<S, M> StoreContainer for Store<S, M>
    where S: Stateful, M: ActionFilter<S> {
    fn subtree_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
        where F: GeneralListener + Clone {
        self.inner.borrow_mut().subtree_listener(f, s);
    }

    fn inverse_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
        where F: InverseListener + Clone {
        self.inner.borrow_mut().inverse_listener(f, s);
    }
}

impl<S> Store<S, Filter<S>>
    where S: Stateful {
    fn action_filter<F>(&self, filter: F, s: &Slock<impl ThreadMarker>)
        where F: Send + Sync + Fn(S::Action, &Slock) -> S::Action + 'static {
        self.get_ref().borrow_mut().action_filter(filter, s);
    }
}

impl<S, A> Signal<S> for Store<S, A>
    where S: Stateful, A: ActionFilter<S>
{
    fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=S> {
        self._borrow(s)
    }

    fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>) where F: Fn(&S, &Slock) -> bool + Send + 'static {
        self._listen(listener, s);
    }

    type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
    fn map<U, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U> where U: Send + 'static, F: Send + 'static + Fn(&S) -> U {
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


impl<S, F> RawStoreSharedOwner<S, F> for Store<S, F>
    where S: Stateful, F: ActionFilter<S> {
    type Inner = InnerStore<S, F>;

    fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
        &self.inner
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

    fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>) where F: Fn(&S, &Slock) -> bool + Send + 'static {
        self._listen(listener, s);
    }

    type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
    fn map<U, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U> where U: Send + 'static, F: Send + 'static + Fn(&S) -> U {
        self._map(map, s)
    }
}

impl<S, F> Binding<S, F> for TokenStore<S, F>
    where S: Stateful + Copy + Hash + Eq, F: ActionFilter<S> {
    fn apply(&self, action: impl IntoAction<S>, s: &Slock) {
        self._apply(action, s);
    }
}

impl<S, A> RawStoreSharedOwner<S, A> for TokenStore<S, A>
    where S: Stateful + Copy + Hash + Eq, A: ActionFilter<S> {
    type Inner = InnerTokenStore<S, A>;

    fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
        &self.inner
    }
}

impl<S> TokenStore<S, Filter<S>>
    where S: Stateful + Hash + Copy + Eq {
    fn action_filter<F>(&self, filter: F, s: &Slock<impl ThreadMarker>)
        where F: Send + Sync + Fn(S::Action, &Slock) -> S::Action + 'static {
        self.get_ref().borrow_mut().action_filter(filter, s);
    }
}

enum StateListener<S: Stateful> {
    ActionListener(Box<dyn (FnMut(&S::Action, &Slock) -> bool) + Send>),
    SignalListener(Box<dyn (FnMut(&S, &Slock) -> bool) + Send>),
    GeneralListener(Box<dyn FnMut(&Slock) -> bool + Send>),
}

impl<'a, S, M, I> Deref for StateRef<'a, S, M, I>
    where S: Stateful, M: ActionFilter<S>, I: RawStore<S, M> {
    type Target = S;
    fn deref(&self) -> &S {
        self.main_ref.data()
    }
}

/* RawState Implementations */
mod _raw_store_impl {
    use std::cell::RefCell;
    use std::hash::Hash;
    use std::ops::DerefMut;
    use std::sync::Arc;
    use crate::{
        core::{Slock, ThreadMarker},
        state::{ActionFilter, GroupAction, GeneralListener, InnerStore, IntoAction, InverseListener, RawStore, Stateful, StateListener, Store, Binding, InnerTokenStore, TokenStore}
    };

    impl<S: Stateful, F:ActionFilter<S>> RawStore<S, F> for InnerStore<S, F> {
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

        fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>) where G: Send + Sync + Fn(S::Action, &Slock) -> S::Action + 'static {
            self.filter.add_filter(filter);
        }

        fn data(&self) -> &S {
            &self.data
        }
    }

    impl<S, F> RawStore<S, F> for InnerTokenStore<S, F>
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

        fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>) where G: Send + Sync + Fn(S::Action, &Slock) -> S::Action + 'static {
            self.filter.add_filter(filter);
        }

        fn data(&self) -> &S {
            &self.data
        }
    }
}
pub use _raw_store_impl::*;


impl<S: Stateful + Default, F: ActionFilter<S>> Default for Store<S, F> {
    fn default() -> Self {
        Self::new(S::default())
    }
}

impl<S: Stateful, F: ActionFilter<S>> Store<S, F> {
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

    fn binding(&self) -> impl Binding<S, F> + Clone {
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

impl<S: Stateful + Copy + Hash + Eq + Default, F: ActionFilter<S>> Default for TokenStore<S, F> {
    fn default() -> Self {
        Self::new(S::default())
    }
}

impl<S: Stateful + Copy + Hash + Eq, F: ActionFilter<S>> TokenStore<S, F> {
    pub fn new(initial: S) -> Self {
        TokenStore {
            inner: Arc::new(RefCell::new(InnerTokenStore {
                data: initial,
                listeners: Vec::new(),
                equal_listeners: HashMap::new(),
                inverse_listener: None,
                filter: F::new(),
            }))
        }
    }

    fn binding(&self) -> impl Binding<S, F> + Clone {
        GeneralBinding {
            inner: Arc::clone(&self.get_ref()),
            phantom_state: PhantomData,
            phantom_filter: PhantomData
        }
    }

    fn signal(&self) -> impl Signal<S> + Clone {
        self.binding()
    }

    pub fn equals(&self, target: S, _s: &Slock) -> impl Signal<bool> + Clone {
        // TODO this feels like awkward placement lots of repetitive code here in general
        let mut my_inner = self.inner.borrow_mut();
        let inner = GeneralInnerSignal {
            val: my_inner.data == target,
            audience: SignalAudience::new(),
        };

        let arc = Arc::new(GeneralSyncCell(RefCell::new(inner)));
        let weak = Arc::downgrade(&arc);

        my_inner.equal_listeners.entry(target)
            .or_insert(Vec::new())
            .push(Box::new(
                move |val, s| {
                    if let Some(arc) = weak.upgrade() {
                        let mut binding = arc.0.borrow_mut();
                        let inner = binding.deref_mut();
                        inner.val = *val == target;
                        inner.audience.dispatch(&inner.val, s);
                        true
                    }
                    else {
                        false
                    }
                }
            ));

        GeneralSignal {
            inner: arc
        }
    }
}
/* Group Action Implementations */
mod _group_action_impl {
    use std::ops::{Mul};
    use super::{GroupAction, StoreContainer, Stateful};

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

    impl<T: StoreContainer> GroupAction<Vec<T>> for VectorAction<T>
    where T: 'static
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
pub use _group_action_impl::*;

/* Stateful Implementations */
mod _stateful_impl {
    use crate::core::{Slock, ThreadMarker};
    use super::{Stateful, SetAction, VectorAction, StringAction, StoreContainer, GeneralListener, InverseListener};

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

    impl<T: StoreContainer> StoreContainer for Vec<T> {
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

    impl<T: StoreContainer> Stateful for Vec<T> { type Action = VectorAction<T>; }
}
pub use _stateful_impl::*;

struct SignalAudience<T: Send> {
    listeners: Vec<Box<dyn FnMut(&T, &Slock) -> bool + Send>>
}

impl<T: Send> SignalAudience<T> {
    fn new() -> SignalAudience<T> {
        SignalAudience {
            listeners: Vec::new()
        }
    }

    fn listen<F: (Fn(&T, &Slock) -> bool) + Send + 'static>(&mut self, listener: F, _s: &Slock<impl ThreadMarker>) {
        self.listeners.push(Box::new(listener));
    }

    fn dispatch(&mut self, new_val: &T, s: &Slock<impl ThreadMarker>) {
        self.listeners
            .retain_mut(|listener| listener(new_val, s.as_ref()))
    }
}

trait InnerSignal<T: Send> {
    fn borrow(&self) -> &T;
}

struct SignalRef<'a, T: Send, U: InnerSignal<T>> {
    src: Ref<'a, U>,
    marker: PhantomData<&'a T>
}

impl<'a, T: Send, U: InnerSignal<T>> Deref for SignalRef<'a, T, U> {
    type Target = T;

    fn deref(&self) -> &T {
        self.src.borrow()
    }
}


struct InnerFixedSignal<T: Send>(T);

impl<T: Send> InnerSignal<T> for InnerFixedSignal<T> {
    fn borrow(&self) -> &T {
        &self.0
    }
}

pub struct FixedSignal<T: Send + 'static> {
    inner: Arc<RefCell<InnerFixedSignal<T>>>
}

impl<T: Send + 'static> FixedSignal<T> {
    pub fn new(val: T) -> FixedSignal<T> {
        FixedSignal {
            inner: Arc::new(RefCell::new(InnerFixedSignal(val)))
        }
    }
}

impl<T: Send + 'static> Clone for FixedSignal<T> {
    fn clone(&self) -> Self {
        FixedSignal {
            inner: self.inner.clone()
        }
    }
}

impl<T: Send + 'static> Signal<T> for FixedSignal<T> {
    fn borrow<'a>(&'a self, _s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=T> {
        SignalRef {
            src: self.inner.borrow(),
            marker: PhantomData
        }
    }

    fn listen<F: Fn(&T, &Slock) -> bool + Send>(&self, _listener: F, _s: &Slock<impl ThreadMarker>) {
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

struct GeneralInnerSignal<T: Send> {
    val: T,
    audience: SignalAudience<T>
}

impl<T: Send> InnerSignal<T> for GeneralInnerSignal<T> {
    fn borrow(&self) -> &T {
        &self.val
    }
}

struct GeneralSyncCell<T: Send>(RefCell<GeneralInnerSignal<T>>);
pub struct GeneralSignal<T: Send + 'static> {
    inner: Arc<GeneralSyncCell<T>>
}

impl<T: Send + 'static> Clone for GeneralSignal<T> {
    fn clone(&self) -> Self {
        GeneralSignal {
            inner: self.inner.clone()
        }
    }
}

impl<T: Send + 'static> GeneralSignal<T> {
    fn from<U, F>(source: &impl Signal<U>, map: F, s: &Slock<impl ThreadMarker>) -> GeneralSignal<T>
        where U: Send + 'static,
              F: Send + 'static + Fn(&U) -> T
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

        source.listen(move |val, s| {
            if let Some(arc) = weak.upgrade() {
                let mut binding = arc.0.borrow_mut();
                let inner = binding.deref_mut();
                inner.val = map(val);
                inner.audience.dispatch(&inner.val, s);
                true
            }
            else {
                false
            }
        }, s);

        GeneralSignal {
            inner: arc
        }
    }
}

impl<T: Send + 'static> Signal<T> for GeneralSignal<T> {
    fn borrow<'a>(&'a self, _s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=T> {
        SignalRef {
            src: self.inner.0.borrow(),
            marker: PhantomData,
        }
    }

    fn listen<F: Fn(&T, &Slock) -> bool + Send + 'static>(&self, listener: F, s: &Slock<impl ThreadMarker>) {
        self.inner.0.borrow_mut().audience.listen(listener, s);
    }

    type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
    fn map<S: Send + 'static, F: Fn(&T) -> S + Send + 'static>(&self, map: F, s: &Slock<impl ThreadMarker>) -> GeneralSignal<S> {
        GeneralSignal::from(self, map, s)
    }
}

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
            }
            else {
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
            }
            else {
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

    fn listen<F: Fn(&V, &Slock) -> bool + Send + 'static>(&self, listener: F, s: &Slock<impl ThreadMarker>) {
        self.inner.0.borrow_mut().audience.listen(listener, s);
    }

    type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
    fn map<S, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> GeneralSignal<S>
        where S: Send + 'static,
              F: Send + 'static + Fn(&V) -> S
    {
        GeneralSignal::from(self, map, s)
    }
}

// Safety: all types require T to be Send
// and can only be read or written to using the global state lock
unsafe impl<T: Send + 'static> Send for FixedSignal<T> {}
unsafe impl<T: Send + 'static> Sync for FixedSignal<T> {}

unsafe impl<T: Send + 'static> Send for GeneralSignal<T> {}
unsafe impl<T: Send + 'static> Sync for GeneralSignal<T> {}

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

// safety: the refcell will only be accessed by a single thread at a time
unsafe impl<T: Send> Sync for GeneralSyncCell<T> { }
unsafe impl<T, U, V> Sync for JoinedSyncCell<T, U, V>
    where T: Send + Clone + 'static,
          U: Send + Clone + 'static,
          V: Send + 'static
{}

#[cfg(test)]
mod test {
    use crate::core::{slock};
    use crate::state::{SetAction, Store, Signal, TokenStore, Binding};
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
        let mut counts: [Store<usize>; 10] = Default::default();
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