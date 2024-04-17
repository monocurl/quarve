use std::cell::{Ref, RefCell};
use std::marker::PhantomData;
use std::ops::{Add, Deref, DerefMut, Mul, Sub};
use std::sync::{Arc};

use crate::core::Slock;

trait GeneralListener : FnMut(&Slock) + Send + 'static {}
trait InverseListener : FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) + Send + 'static {}
impl<T: FnMut(&Slock) + Send + 'static> GeneralListener for T {}
impl<T: FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) + Send + 'static> InverseListener for T {}

/// It is the implementors job to guarantee that subtree_listener
/// and relatives do not get into call cycles
pub trait StateContainer: Send + 'static {
    fn subtree_listener<F: GeneralListener + Clone>(&self, f: F, s: &Slock);

    fn inverse_listener<F: InverseListener + Clone>(&self, f: F, s: &Slock);
}

pub trait StatefulDispatcher: Send + Sized {
    fn subtree_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock)
    {

    }

    fn inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock)
    {

    }
}

pub trait Stateful: StatefulDispatcher + 'static {
    type Action: GroupAction<Self>;
}

pub trait GroupAction<T: Stateful>: Send + Sized + Mul<Output=Self> + 'static {

    fn identity() -> Self;

    // returns inverse action
    fn apply(self, to: &mut T) -> Self;

    // for undo grouping
    fn description(&self) -> &'static str {
        ""
    }
}

pub trait ActionFilter<S: Stateful> : 'static {
    fn new() -> Self;
    fn filter(&self, a: S::Action, s: &Slock) -> S::Action;
}

pub struct Filter<S: Stateful>(
    Vec<Box<dyn Send + Sync + Fn(S::Action, &Slock) -> S::Action>>
);

pub struct Filterless();

impl<S: Stateful> ActionFilter<S> for Filter<S> {
    fn new() -> Filter<S> {
        Filter(Vec::new())
    }

    fn filter(&self, a: S::Action, s: &Slock) -> S::Action {
        self.0
            .iter()
            .fold(a, |a, action| action(a, s))
    }
}

impl<S: Stateful> ActionFilter<S> for Filterless {
    fn new() -> Filterless {
        Filterless()
    }

    fn filter(&self, a: S::Action, _s: &Slock) -> S::Action {
        a
    }
}

struct InnerState<S: Stateful, F: ActionFilter<S>> {
    data: S,
    listeners: Vec<StateListener<S>>,
    inverse_listener: Option<Box<dyn FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) + Send>>,
    filter: F,
}

pub struct State<S: Stateful, F: ActionFilter<S>=Filter<S>>
{
    inner: Arc<RefCell<InnerState<S, F>>>
}

// safety: all accesses to inner are done using the global state lock
// and Stateful: Send
unsafe impl<S: Stateful, F: ActionFilter<S>> Send for State<S, F> { }

// safety: all accesses to inner are done using the global state lock
// and Stateful: Send
unsafe impl<S: Stateful, F: ActionFilter<S>> Sync for State<S, F> { }

enum StateListener<S: Stateful> {
    ActionListener(Box<dyn (FnMut(&S::Action, &Slock) -> bool) + Send>),
    SignalListener(Box<dyn (FnMut(&S, &Slock) -> bool) + Send>),
    GeneralListener(Box<dyn FnMut(&Slock) + Send>),
}

#[derive(Clone)]
pub enum ToggleAction {
    Identity,
    Set(bool),
    Toggle
}

#[derive(Clone)]
pub enum NumericAction<T>
    where T: Stateful + Add<Output=T> + Sub<Output=T> + PartialOrd + Copy
{
    Set(T),
    Incr(T),
    Decr(T),
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

/* Group Action Implementations */
mod _group_action_impl {
    use std::ops::{Add, Mul, Sub};
    use super::{GroupAction, NumericAction, StateContainer, Stateful, StringAction, ToggleAction, VectorAction};

    impl Mul for ToggleAction {
        type Output = Self;

        fn mul(self, rhs: Self) -> Self {
            match (self, rhs) {
                (ToggleAction::Identity, rhs) => rhs,
                (lhs, ToggleAction::Identity) => lhs,
                (ToggleAction::Set(val), _) => ToggleAction::Set(val),
                (ToggleAction::Toggle, ToggleAction::Set(val)) => ToggleAction::Set(!val),
                (ToggleAction::Toggle, ToggleAction::Toggle) => ToggleAction::Identity
            }
        }
    }

    impl GroupAction<bool> for ToggleAction {
        fn identity() -> Self {
            ToggleAction::Identity
        }

        fn apply(self, to: &mut bool) -> Self {
            match self {
                ToggleAction::Identity => ToggleAction::Identity,
                ToggleAction::Toggle => {
                    *to = !*to;
                    ToggleAction::Toggle
                },
                ToggleAction::Set(new) => {
                    let old = *to;
                    *to = new;
                    ToggleAction::Set(old)
                }
            }
        }
    }

    impl<T> Mul for NumericAction<T>
        where T: Stateful + Add<Output=T> + Sub<Output=T> + PartialOrd + Copy
    {
        type Output = Self;

        fn mul(self, rhs: Self) -> Self {
            let mul_incrs = |incr, decr| {
                if incr >= decr {
                    NumericAction::Incr(incr - decr)
                }
                else {
                    NumericAction::Decr(decr - incr)
                }
            };

            match (self, rhs) {
                (NumericAction::Identity, rhs) => rhs,
                (lhs, NumericAction::Identity) => lhs,
                (NumericAction::Set(val), _) => NumericAction::Set(val),
                (NumericAction::Incr(delta), NumericAction::Decr(ndelta)) => {
                    mul_incrs(delta, ndelta)
                }
                (NumericAction::Incr(delta), NumericAction::Set(pin)) => {
                    NumericAction::Set(pin + delta)
                }
                (NumericAction::Decr(ndelta), NumericAction::Incr(delta)) => {
                    mul_incrs(delta, ndelta)
                }
                (NumericAction::Decr(ndelta), NumericAction::Set(pin)) => {
                    NumericAction::Set(pin - ndelta)
                }
                (NumericAction::Incr(d1), NumericAction::Incr(d2)) => {
                    NumericAction::Incr(d1 + d2)
                }
                (NumericAction::Decr(d1), NumericAction::Decr(d2)) => {
                    NumericAction::Decr(d1 + d2)
                }
            }
        }
    }

    impl<T> GroupAction<T> for NumericAction<T>
        where T: Stateful + Add<Output=T> + Sub<Output=T> + PartialOrd + Copy + 'static
    {
        fn identity() -> Self {
            NumericAction::Identity
        }

        fn apply(self, to: &mut T) -> Self {
            match self {
                NumericAction::Identity => NumericAction::Identity,
                NumericAction::Set(targ) => {
                    let ret = *to;
                    *to = targ;

                    NumericAction::Set(ret)
                },
                NumericAction::Incr(delta) => {
                    *to = *to + delta;

                    NumericAction::Decr(delta)
                }
                NumericAction::Decr(delta) => {
                    *to = *to - delta;

                    NumericAction::Incr(delta)
                }
            }
        }
    }


    impl Mul for StringAction {
        type Output = Self;

        fn mul(self, rhs: Self) -> Self {
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

        fn apply(self, to: &mut String) -> Self {
            match self {
                StringAction::ReplaceSubrange(start, end, content) => {
                    Self::identity()
                },
                StringAction::ReplaceSubranges(replacements) => {
                    Self::identity()
                }
            }
        }
    }


    impl<T> Mul for VectorAction<T> {
        type Output = Self;

        fn mul(self, rhs: Self) -> Self {
            self
        }
    }

    impl<T: StateContainer> GroupAction<Vec<T>> for VectorAction<T>
    where T: 'static
    {
        fn identity() -> Self {
            Self::Replace(0, 0, None)
        }

        fn apply(self, to: &mut Vec<T>) -> Self {
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

/* Stateful Implementations */
mod _stateful_implementations {
    use crate::core::Slock;
    use super::{StatefulDispatcher, Stateful, ToggleAction, NumericAction, VectorAction, StringAction, StateContainer};

    impl StatefulDispatcher for bool {}

    impl Stateful for bool { type Action = ToggleAction; }

    impl StatefulDispatcher for usize {}

    impl Stateful for usize { type Action = NumericAction<Self>; }

    impl StatefulDispatcher for u8 {}

    impl Stateful for u8 { type Action = NumericAction<Self>; }

    impl StatefulDispatcher for u16 {}

    impl Stateful for u16 { type Action = NumericAction<Self>; }

    impl StatefulDispatcher for u32 {}

    impl Stateful for u32 { type Action = NumericAction<Self>; }

    impl StatefulDispatcher for u64 {}

    impl Stateful for u64 { type Action = NumericAction<Self>; }

    impl StatefulDispatcher for i8 {}

    impl Stateful for i8 { type Action = NumericAction<Self>; }

    impl StatefulDispatcher for i16 {}

    impl Stateful for i16 { type Action = NumericAction<Self>; }

    impl StatefulDispatcher for i32 {}

    impl Stateful for i32 { type Action = NumericAction<Self>; }

    impl StatefulDispatcher for i64 {}

    impl Stateful for i64 { type Action = NumericAction<Self>; }

    impl StatefulDispatcher for String {}

    impl Stateful for String { type Action = StringAction; }

    impl<T: StateContainer> StatefulDispatcher for Vec<T> {
        fn subtree_listener<F>(&self, f: F, s: &Slock) where F: FnMut(&Slock) + Send + Clone + 'static {
            for container in self {
                container.subtree_listener(f.clone(), s);
            }
        }

        fn inverse_listener<F>(&self, f: F, s: &Slock)
            where F: FnMut(&'static str, Box<dyn FnOnce(&Slock) + Send>, &Slock) + Send + Clone + 'static {
            for container in self {
                container.inverse_listener(f.clone(), s);
            }
        }
    }

    impl<T: StateContainer> Stateful for Vec<T> { type Action = VectorAction<T>; }
}

pub struct StateRef<'a, S: Stateful, M: ActionFilter<S>> {
    main_ref: Ref<'a, InnerState<S, M>>,
    lifetime: PhantomData<&'a S>,
    filter: PhantomData<M>
}

impl<'a, S: Stateful, M: ActionFilter<S>> Deref for StateRef<'a, S, M> {
    type Target = S;
    fn deref(&self) -> &S {
        &(*self.main_ref).data
    }
}

impl<S: Stateful, F: ActionFilter<S>> State<S, F> {
    pub fn new(initial: S) -> State<S, F> {
        State {
            inner: Arc::new(RefCell::new(InnerState {
                data: initial,
                listeners: Vec::new(),
                inverse_listener: None,
                filter: F::new(),
            }))
        }
    }

    pub fn apply(&self, transaction: S::Action, s: &Slock)  {
        let mut binding = self.inner.borrow_mut();
        let inner = binding.deref_mut();

        let action = inner.filter.filter(transaction, s);
        let name = action.description();

        // tell action listeners
        inner.listeners.retain_mut(
            |listener| match listener {
                StateListener::ActionListener(listener) => listener(&action, s),
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
                    action(s);
                    true
                },
                StateListener::SignalListener(action) => action(data, s),
                _ => true
            }
        );

        // tell inverse listener
        if let Some(ref mut inv_listener) = inner.inverse_listener {
            let state = State { inner: self.inner.clone() };
            let invert = move |s: &Slock| {
                state.apply(inverse, s);
            };
            inv_listener(name, Box::new(invert), s)
        }
    }
}

impl<S: Stateful, F: ActionFilter<S>> State<S, F>
{
    pub fn action_listener(&self, f: Box<dyn FnMut(&S::Action, &Slock) -> bool + Send + Sync>, _s: &Slock) {
        let mut inner = self.inner.borrow_mut();
        inner.listeners.push(
            StateListener::ActionListener(f)
        );
    }
}

impl<S: Stateful, M: ActionFilter<S>> StateContainer for State<S, M>
{
    fn subtree_listener<F: GeneralListener + Clone>(&self, f: F, s: &Slock)
    {
        // we have the global state lock
        // reentry will be caught by the ref_cell
        let mut inner = self.inner.borrow_mut();
        inner.listeners.push(
            StateListener::GeneralListener(Box::new(f.clone()))
        );
        inner.data.subtree_listener(f, s);
    }

    fn inverse_listener<F: InverseListener + Clone>(&self, f: F, s: &Slock)
    {
        let mut inner = self.inner.borrow_mut();
        inner.data.inverse_listener(f.clone(), s);
        inner.inverse_listener = Some(Box::new(f));
    }
}

impl<S: Stateful, M: ActionFilter<S>> Clone for State<S, M> {
    fn clone(&self) -> Self {
        State {
            inner: Arc::clone(&self.inner)
        }
    }
}

impl<S: Stateful, M: ActionFilter<S>> Signal<S> for State<S, M> {
    fn borrow<'a>(&'a self, _s: &'a Slock) -> StateRef<'a, S, M> {
        StateRef {
            main_ref: self.inner.borrow(),
            lifetime: PhantomData,
            filter: PhantomData
        }
    }

    fn listen<F: Fn(&S, &Slock) -> bool + Send + 'static>(&self, listener: F, _s: &Slock) {
        let mut inner = self.inner.borrow_mut();
        inner.listeners.push(StateListener::SignalListener(Box::new(listener)));
    }

    type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
    fn map<U: Send + 'static, F: Fn(&S) -> U + Send + 'static>(&self, map: F, s: &Slock) -> GeneralSignal<U> {
        GeneralSignal::from(self, map, s)
    }
}

/* only for filtered state */
impl<S: Stateful> State<S> {
    fn action_filter<F>(&self, filter: F, _s: &Slock)
        where F: Send + Sync + Fn(S::Action, &Slock) -> S::Action + 'static
    {
        self.inner.borrow_mut()
            .filter.0.insert(0, Box::new(filter));
    }
}

struct SignalAudience<T: Send> {
    listeners: Vec<Box<dyn FnMut(&T, &Slock) -> bool + Send>>
}

impl<T: Send> SignalAudience<T> {
    fn new() -> SignalAudience<T> {
        SignalAudience {
            listeners: Vec::new()
        }
    }

    fn listen<F: (Fn(&T, &Slock) -> bool) + Send + 'static>(&mut self, listener: F, _s: &Slock) {
        self.listeners.push(Box::new(listener));
    }

    fn dispatch(&mut self, new_val: &T, s: &Slock) {
        self.listeners
            .retain_mut(|listener| listener(new_val, s))
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

pub trait Signal<T: Send + 'static> : Clone + Send + Sync + 'static {

    fn borrow<'a>(&'a self, s: &'a Slock) -> impl Deref<Target=T>;

    fn listen<F: (Fn(&T, &Slock) -> bool) + Send + 'static>(&self, listener: F, _s: &Slock);

    type MappedOutput<S: Send + 'static>: Signal<S>;
    fn map<S, F>(&self, map: F, _s: &Slock) -> Self::MappedOutput<S>
        where S: Send + 'static,
              F: Send + 'static + Fn(&T) -> S;
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
    fn borrow<'a>(&'a self, _s: &'a Slock) -> SignalRef<'a, T, InnerFixedSignal<T>> {
        SignalRef {
            src: self.inner.borrow(),
            marker: PhantomData
        }
    }

    fn listen<F: Fn(&T, &Slock) -> bool + Send>(&self, _listener: F, _s: &Slock) {
        /* no op */
    }

    type MappedOutput<S: Send + 'static> = FixedSignal<S>;
    fn map<S, F>(&self, map: F, _s: &Slock) -> FixedSignal<S>
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
    fn from<U, F>(source: &impl Signal<U>, map: F, s: &Slock) -> GeneralSignal<T>
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
    fn borrow<'a>(&'a self, _s: &'a Slock) -> impl Deref<Target=T> {
        SignalRef {
            src: self.inner.0.borrow(),
            marker: PhantomData,
        }
    }

    fn listen<F: Fn(&T, &Slock) -> bool + Send + 'static>(&self, listener: F, s: &Slock) {
        self.inner.0.borrow_mut().audience.listen(listener, s);
    }

    type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
    fn map<S: Send + 'static, F: Fn(&T) -> S + Send + 'static>(&self, map: F, s: &Slock) -> GeneralSignal<S> {
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
    pub fn from<F>(lhs: &impl Signal<T>, rhs: &impl Signal<U>, map: F, s: &Slock)
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
            if let Some(mut arc) = weak.upgrade() {
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
            if let Some(mut arc) = weak.upgrade() {
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
    fn borrow<'a>(&'a self, _s: &'a Slock) -> impl Deref<Target=V> {
        SignalRef {
            src: self.inner.0.borrow(),
            marker: Default::default(),
        }
    }

    fn listen<F: Fn(&V, &Slock) -> bool + Send + 'static>(&self, listener: F, s: &Slock) {
        self.inner.0.borrow_mut().audience.listen(listener, s);
    }

    type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
    fn map<S, F>(&self, map: F, s: &Slock) -> GeneralSignal<S>
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
    use crate::state::{NumericAction, State, ToggleAction};
    use crate::state::Signal;

    use std::ops::Deref;

    #[test]
    fn test_numeric() {
        let c = slock();

        let s: State<i32> = State::new(2);
        let derived_sig;
        let derived_derived;
        {
            derived_sig = c.map(&s, |x| x * x);
            let b = c.read(&derived_sig);
            assert_eq!(*b, 4);
        }
        {
            derived_derived = derived_sig.map(|x| x - 4, &c);
        }

        c.apply(NumericAction::Incr(4), &s);
        {
            let b = c.read(&derived_sig);
            assert_eq!(*b, 36);
            let b = c.read(&derived_derived);
            assert_eq!(*b, 32);
        }

        c.apply(NumericAction::Identity *
                    NumericAction::Decr(2) *
                    NumericAction::Set(3) *
                    NumericAction::Incr(4),
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
    fn test_bool() {
        let s = slock();

        let bool_state: State<bool> = State::new(false);
        bool_state.apply(ToggleAction::Toggle, &s);
        assert_eq!(*bool_state.borrow(&s), true);
        bool_state.apply(ToggleAction::Set(false) * ToggleAction::Identity, &s);
        assert_eq!(*bool_state.borrow(&s), false);
        bool_state.apply(ToggleAction::Toggle * ToggleAction::Set(true) * ToggleAction::Toggle, &s);
        assert_eq!(*bool_state.borrow(&s), false);
        bool_state.apply(ToggleAction::Toggle * ToggleAction::Toggle * ToggleAction::Toggle, &s);
        assert_eq!(*bool_state.borrow(&s), true);
    }

    #[test]
    fn test_join() {
        let s = slock();

        let x: State<i32> = State::new(3);
        let y: State<bool> = State::new(false);

        let join = s.join(&x, &y);
        assert_eq!(*join.borrow(&s), (3, false));

        s.apply(NumericAction::Set(4), &x);
        assert_eq!(*join.borrow(&s), (4, false));

        s.apply(ToggleAction::Toggle, &y);
        assert_eq!(*join.borrow(&s), (4, true));

        s.apply(NumericAction::Decr(5), &x);
        s.apply(ToggleAction::Toggle, &y);
        assert_eq!(*join.borrow(&s), (-1, false));
    }

    #[test]
    fn test_join_map() {
        let s = slock();

        let x: State<i32> = State::new(3);
        let y: State<bool> = State::new(false);

        let join = s.join_map(&x, &y, |x, y|
            if *y {
                x + 4
            }
            else {
                x * x
            }
        );
        assert_eq!(*join.borrow(&s), 9);

        s.apply(NumericAction::Set(4), &x);
        assert_eq!(*join.borrow(&s), 16);

        s.apply(ToggleAction::Toggle, &y);
        assert_eq!(*join.borrow(&s), 8);

        s.apply(NumericAction::Decr(5), &x);
        s.apply(ToggleAction::Toggle, &y);
        assert_eq!(*join.borrow(&s), 1);

        drop(x);
        s.apply(ToggleAction::Toggle, &y);
        assert_eq!(*join.borrow(&s), 3);
    }
}