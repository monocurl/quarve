// TODO
// TokenStore should be removed and replaced with a dispatcher
// dependency injection
// CRDT stores at some point
// path listeners and path appliers

pub mod slock_cell {
    use std::cell::{Ref, RefCell, RefMut};
    use std::mem;
    use std::ops::{Deref, DerefMut};
    use crate::core::{MSlock, Slock};
    use crate::native;
    use crate::util::marker::ThreadMarker;
    use crate::util::rust_util::{EnsureSend, EnsureSync};

    pub struct SlockCell<T>(RefCell<T>) where T: Send + ?Sized;
    struct DropHalter;

    // FIXME at some point have a constructor without slockcell that is runtime checked
    pub struct MainSlockCell<T>(DropHalter, RefCell<T>) where T: ?Sized;

    impl<T> SlockCell<T> where T: Send {
        pub fn new(val: T) -> Self {
            SlockCell(RefCell::new(val))
        }

        // slock isnt necessary
        // since we have ownership at this point
        pub fn into_inner(self) -> T {
            self.0.into_inner()
        }
    }

    impl<T> MainSlockCell<T> {
        pub fn into_inner_main(self, _s: MSlock) -> T {
            // technically unnecessary, but micro optimization i guess
            mem::forget(self.0);
            self.1.into_inner()
        }

        pub fn new_main(val: T, _s: MSlock) -> Self {
            MainSlockCell(DropHalter, RefCell::new(val))
        }
    }

    impl<T> SlockCell<T> where T: Send + ?Sized {
        pub fn is_borrowed(&self, _s: Slock<impl ThreadMarker>) -> bool {
            // note that is borrowed must call borrowed mut
            // and vice versa
            self.0.try_borrow_mut().is_err()
        }

        pub fn is_borrowed_mut(&self, _s: Slock<impl ThreadMarker>) -> bool {
            self.0.try_borrow().is_err()
        }

        pub fn borrow<'a>(&'a self, _s: Slock<'a, impl ThreadMarker>) -> Ref<'a, T> {
            self.0.borrow()
        }

        pub fn borrow_mut<'a>(&'a self, _s: Slock<'a, impl ThreadMarker>) -> RefMut<'a, T>  {
            self.0.borrow_mut()
        }

        pub unsafe fn as_ptr(&self) -> *const T {
            self.0.as_ptr()
        }

        pub unsafe fn as_mut_ptr(&self) -> *mut T {
            self.0.as_ptr()
        }
    }

    impl<T> MainSlockCell<T> where T: ?Sized {
        pub fn borrow_main<'a>(&'a self, _s: MSlock<'a>) -> impl Deref<Target=T> + 'a {
            self.1.borrow()
        }

        pub fn borrow_mut_main<'a>(&'a self, _s: MSlock<'a>) -> impl DerefMut<Target=T> + 'a {
            self.1.borrow_mut()
        }

        // caller must ensure that the part they're
        // borrowing is send
        pub unsafe fn borrow_non_main_non_send<'a>(&'a self, _s: Slock<'a>) -> impl Deref<Target=T> + 'a {
            self.1.borrow()
        }

        pub unsafe fn borrow_mut_non_main_non_send<'a>(&'a self, _s: Slock<'a>) -> impl DerefMut<Target=T> + 'a {
            self.1.borrow_mut()
        }

        pub fn as_ptr(&self) -> *const T {
            self.1.as_ptr()
        }

        pub fn as_mut_ptr(&self) -> *mut T {
            self.1.as_ptr()
        }
    }

    impl Drop for DropHalter {
        fn drop(&mut self) {
            assert!(native::global::is_main(),
                    "Cannot drop a MainSlockCell (either directly or via drop glue) \
                     outside of the main thread!
                     note: we are working on ways to prevent this at compile time,
                     though all proposed solutions require intense manual dropping.
            ");
        }
    }

    // Safety: all borrows require the state lock and T: Send
    // OR required the Mslock (hence aren't being sent anywhere)
    // OR are unsafe
    unsafe impl<T> Sync for SlockCell<T> where T: Send + ?Sized {}
    unsafe impl<T> Send for SlockCell<T> where T: Send + ?Sized {}

    impl<T> EnsureSend for SlockCell<T> where T: Send + ?Sized {}
    impl<T> EnsureSync for SlockCell<T> where T: Send + ?Sized {}

    unsafe impl<T> Sync for MainSlockCell<T> where T: ?Sized {}
    unsafe impl<T> Send for MainSlockCell<T> where T: ?Sized {}

    impl<T> EnsureSend for MainSlockCell<T> where T: ?Sized {}
    impl<T> EnsureSync for MainSlockCell<T> where T: ?Sized {}
}

mod listener {
    use crate::core::Slock;
    use crate::state::Stateful;

    #[allow(private_bounds)]
    pub trait DirectlyInvertible: Send {
        // This function must only be called once per instance
        // (We cannot take ownership since the caller is often unsized)
        fn invert(&mut self, s: Slock);

        /// It must be guaranteed by the caller
        /// the other type is exactly the same as our type
        /// and with the same id
        unsafe fn right_multiply(&mut self, by: Box<dyn DirectlyInvertible>, s: Slock);

        // gets a pointer to the action instance
        // (void pointer)
        unsafe fn action_pointer(&self, s: Slock) -> *const ();

        // forgets the reference action without dropping it
        unsafe fn forget_action(&mut self, s: Slock);
        fn id(&self) -> usize;
    }


    /* trait aliases */
    pub trait GeneralListener : FnMut(Slock) -> bool + Send + 'static {}
    pub trait InverseListener : FnMut(Box<dyn DirectlyInvertible>, Slock) -> bool + Send + 'static {}
    impl<T> GeneralListener for T where T: FnMut(Slock) -> bool + Send + 'static {}
    impl<T> InverseListener for T where T: FnMut(Box<dyn DirectlyInvertible>, Slock) -> bool + Send + 'static {}

    pub(super) type BoxInverseListener = Box<
        dyn FnMut(Box<dyn DirectlyInvertible>, Slock) -> bool + Send
    >;

    pub(super) enum StateListener<S: Stateful> {
        ActionListener(Box<dyn (FnMut(&S, &S::Action, Slock) -> bool) + Send>),
        SignalListener(Box<dyn (FnMut(&S, Slock) -> bool) + Send>),
        GeneralListener(Box<dyn FnMut(Slock) -> bool + Send>),
    }
}
pub use listener::*;

mod group {
    use std::ops::Mul;
    use crate::state::{GeneralListener, InverseListener};
    use crate::core::{Slock};
    use crate::util::marker::{BoolMarker, ThreadMarker};

    pub trait Stateful: Send + Sized + 'static {
        type Action: GroupAction<Self>;
        type HasInnerStores: BoolMarker;

        // This method should return an action listener
        // to be applied on the surrounding container
        // (if it wants)
        #[allow(unused_variables)]
        fn subtree_general_listener<F>(&self, f: F, s: Slock<impl ThreadMarker>)
            -> Option<impl Send + FnMut(&Self, &Self::Action, Slock) -> bool + 'static>
            where F: GeneralListener + Clone {
            None::<fn(&Self, &Self::Action, Slock) -> bool>
        }

        // Returns an action listener to be applied on the parent container
        // (if necessary)
        #[allow(unused_variables)]
        fn subtree_inverse_listener<F>(&self, f: F, s: Slock<impl ThreadMarker>)
            -> Option<impl Send + FnMut(&Self, &Self::Action, Slock) -> bool + 'static>
            where F: InverseListener + Clone {
            None::<fn(&Self, &Self::Action, Slock) -> bool>
        }
    }

    pub trait GroupBasis<T>: Send + Sized + 'static {
        // returns inverse action
        fn apply(self, to: &mut T) -> Self;

        fn forward_description(&self) -> impl Into<String>;
        fn backward_description(&self) -> impl Into<String>;

    }

    pub trait GroupAction<T>: GroupBasis<T> + Mul<Output=Self> {

        fn identity() -> Self;

        fn left_multiply(&mut self, other: Self) {
            let curr = std::mem::replace(self, Self::identity());

            *self = other * curr;
        }

        fn right_multiply(&mut self, other: Self) {
            let curr = std::mem::replace(self, Self::identity());

            *self = curr * other;
        }
    }

    /// it's more natural to make this just an impl<T: Stateful>
    /// but this runs into (what I believe are provably wrong) errors
    /// of possibly conflicting implementations
    pub trait IntoAction<A, T> where A: GroupAction<T> {
        fn into_action(self, target: &T) -> A;
    }

    impl<A, T> IntoAction<A, T> for A where A: GroupAction<T>, T: Stateful {
        fn into_action(self, _target: &T) -> A {
            self
        }
    }

    mod word {
        use std::iter::Rev;
        use std::ops::Mul;
        use crate::state::{GroupAction, GroupBasis, IntoAction};

        #[derive(Debug)]
        pub struct Word<T> where T: 'static {
            items: Vec<T>,
        }

        impl<T> Default for Word<T> where T: 'static {
            fn default() -> Self {
                Word::new(vec![])
            }
        }

        impl<T> Clone for Word<T> where T: Clone + 'static {
            fn clone(&self) -> Self {
                Word {
                    items: self.items.clone(),
                }
            }
        }
        impl<T> Word<T> where T: 'static {
            pub fn new(word: Vec<T>) -> Self {
                Word {
                    items: word,
                }
            }

            pub fn iter(&self) -> impl Iterator<Item=&T> {
                self.items.iter()
                    .rev()
            }

            pub fn len(&self) -> usize {
                self.items.len()
            }
        }

        impl<T> IntoIterator for Word<T> where T: 'static {
            type Item = T;
            type IntoIter = Rev<<Vec<T> as IntoIterator>::IntoIter>;

            fn into_iter(self) -> Self::IntoIter {
                self.items.into_iter()
                    .rev()
            }
        }

        impl<T> Mul for Word<T> where T: 'static {
            type Output = Self;

            fn mul(mut self, mut rhs: Self) -> Self::Output {
                self.items.append(&mut rhs.items);

                self
            }
        }

        impl<T, B> GroupBasis<T> for Word<B> where B: GroupBasis<T> {
            fn apply(self, to: &mut T) -> Self {
                let bases = self.items;

                // find inverse
                let build = bases.into_iter()
                    .rev()
                    .map(|b| b.apply(to))
                    .collect::<Vec<_>>();

                Word::new(build)
            }

            fn forward_description(&self) -> impl Into<String> {
                if let Some(first) = self.items.first() {
                    first.forward_description().into()
                }
                else {
                    "".into()
                }
            }

            fn backward_description(&self) -> impl Into<String> {
                if let Some(last) = self.items.last() {
                    last.backward_description().into()
                }
                else {
                    "".into()
                }
            }
        }

        impl<T, B> GroupAction<T> for Word<B> where B: GroupBasis<T> {

            fn identity() -> Self {
                Word::new(Vec::new())
            }
        }

        impl<T, B> IntoAction<Word<B>, T> for B where B: GroupBasis<T> {
            fn into_action(self, _target: &T) -> Word<B> {
                Word::new(vec![self])
            }
        }
    }
    pub use word::*;

    mod filter {
        use std::marker::PhantomData;
        use crate::core::{Slock};
        use crate::state::{Stateful};
        use crate::util::marker::ThreadMarker;

        pub trait StateFilter: Send + 'static {
            type Target: Stateful;

            fn new() -> Self;

            fn add_filter<F>(&mut self, f: F)
                where F: Send + 'static + Fn(&Self::Target, <Self::Target as Stateful>::Action, Slock) -> <Self::Target as Stateful>::Action;

            fn filter(&self, val: &Self::Target, a: <Self::Target as Stateful>::Action, s: Slock<impl ThreadMarker>) -> <Self::Target as Stateful>::Action;
        }

        pub struct Filter<S: Stateful>(
            Vec<Box<dyn Send + Fn(&S, S::Action, Slock) -> S::Action>>
        );

        // generic parameter is needed for some weird things...
        pub struct Filterless<S>(PhantomData<S>);

        impl<S> StateFilter for Filterless<S> where S: Stateful {
            type Target = S;

            fn new() -> Self {
                Filterless(PhantomData)
            }

            fn add_filter<F>(&mut self, _f: F) where F: Send + 'static + Fn(&S, S::Action, Slock) -> S::Action {

            }

            #[inline]
            fn filter(&self, _val: &S, a: S::Action, _s: Slock<impl ThreadMarker>) -> S::Action {
                a
            }
        }

        impl<S> StateFilter for Filter<S> where S: Stateful {
            type Target = S;

            fn new() -> Self {
                Filter(Vec::new())
            }

            fn add_filter<F>(&mut self, f: F) where F: Send + 'static + Fn(&S, S::Action, Slock) -> S::Action {
                self.0.push(Box::new(f));
            }

            fn filter(&self, val: &S, a: S::Action, s: Slock<impl ThreadMarker>) -> S::Action {
                self.0
                    .iter()
                    .rfold(a, |a, action| action(val, a, s.to_general_slock()))
            }
        }
    }
    pub use filter::*;

    mod action {
        mod set_action {
            use std::ops::Mul;
            use crate::state::{GroupAction, GroupBasis, Stateful};
            use crate::util::marker::FalseMarker;

            #[derive(Clone)]
            pub enum SetAction<T>
            {
                Set(T),
                Identity
            }

            impl<T> Mul for SetAction<T>
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

            impl<T> GroupBasis<T> for SetAction<T>
                where T: Send + 'static
            {
                fn apply(self, to: &mut T) -> Self {
                    match self {
                        SetAction::Identity => SetAction::Identity,
                        SetAction::Set(targ) => {
                            let ret = std::mem::replace(to, targ);

                            SetAction::Set(ret)
                        },
                    }
                }

                fn forward_description(&self) -> impl Into<String> {
                    "Change"
                }

                fn backward_description(&self) -> impl Into<String> {
                    "Change"
                }
            }

            impl<T> GroupAction<T> for SetAction<T>
                where T: Send + 'static
            {
                fn identity() -> Self {
                    SetAction::Identity
                }
            }


            macro_rules! impl_set_stateful {
                ($($t:ty), *) => {
                    $(
                        impl Stateful for $t {
                            type Action = SetAction<$t>;
                            type HasInnerStores = FalseMarker;
                        }
                    )*
                };
            }

            impl_set_stateful!(
                i8, u8,
                i16, u16,
                i32, u32,
                i64, u64,
                i128, u128,
                isize, usize,
                f32, f64,
                bool, String,
                Option<i8>, Option<u8>,
                Option<i16>, Option<u16>,
                Option<i32>, Option<u32>,
                Option<i64>, Option<u64>,
                Option<i128>, Option<u128>,
                Option<isize>, Option<usize>,
                Option<f32>, Option<f64>,
                Option<bool>, Option<String>
            );
        }
        pub use set_action::*;

        mod string_action {
            use std::ops::Range;
            use crate::state::{GroupBasis,  Stateful, Word};
            use crate::util::marker::FalseMarker;

            #[derive(Clone, Debug, PartialEq, Eq)]
            pub struct EditingString(pub String);

            #[derive(Clone)]
            pub enum StringActionBasis {
                // start, length, with
                ReplaceSubrange(Range<usize>, String),
            }

            impl GroupBasis<EditingString> for StringActionBasis {
                fn apply(self, to: &mut EditingString) -> Self {
                    match self {
                        StringActionBasis::ReplaceSubrange(range, content) => {
                            let replaced = to.0[range.clone()].to_owned();
                            let next_range = range.start .. range.start + content.len();
                            to.0.replace_range(range, &content);

                            StringActionBasis::ReplaceSubrange(next_range, replaced)
                        }
                    }
                }

                fn forward_description(&self) -> impl Into<String> {
                    "Change"
                }

                fn backward_description(&self) -> impl Into<String> {
                    "Change"
                }
            }

            impl Stateful for EditingString {
                type Action = Word<StringActionBasis>;
                type HasInnerStores = FalseMarker;
            }
        }
        pub use string_action::*;

        mod vec_action {
            use std::ops::Range;
            use crate::core::{Slock};
            use crate::state::{GeneralListener, GroupBasis, InverseListener, Stateful, StoreContainer, Word};
            use crate::util::marker::{ThreadMarker, TrueMarker};

            #[derive(Clone)]
            pub enum VecActionBasis<T> {
                /* indices */
                Insert(T, usize),
                Remove(usize),
                InsertMany(Vec<T>, usize),
                RemoveMany(Range<usize>),
                // u, v
                Swap(usize, usize)
            }

            impl<T> GroupBasis<Vec<T>> for VecActionBasis<T> where T: Send + 'static
            {
                fn apply(self, to: &mut Vec<T>) -> Self {
                    match self {
                        VecActionBasis::Insert(elem, at) => {
                            to.insert(at, elem);
                            VecActionBasis::Remove(at)
                        },
                        VecActionBasis::InsertMany(items, at) => {
                            let reverse = VecActionBasis::RemoveMany(at.. at + items.len());
                            to.splice(at..at, items.into_iter());
                            reverse
                        }
                        VecActionBasis::Remove(at) => {
                            let removed = to.remove(at);
                            VecActionBasis::Insert(removed, at)
                        }
                        VecActionBasis::RemoveMany(start) => {
                            let at = start.start;
                            let items: Vec<T> = to.splice(start, std::iter::empty())
                                .collect();

                            VecActionBasis::InsertMany(items, at)
                        }
                        VecActionBasis::Swap(a, b) => {
                            to.swap(a, b);
                            VecActionBasis::Swap(a, b)
                        }
                    }
                }

                fn forward_description(&self) -> impl Into<String> {
                    "Change"
                }

                fn backward_description(&self) -> impl Into<String> {
                    "Change"
                }
            }

            /* the amount of stores can be variable so that we must add the listeners dynamically */
            /* in certain cases (for inverse listener), some listeners can be held on a bit longer than they ideally should be */
            /* but this is somewhat hard to avoid */
            impl<T> Stateful for Vec<T> where T: StoreContainer {
                type Action = Word<VecActionBasis<T>>;
                type HasInnerStores = TrueMarker;

                fn subtree_general_listener<F>(&self, mut f: F, s: Slock<impl ThreadMarker>)
                    -> Option<impl Send + FnMut(&Self, &Self::Action, Slock) -> bool + 'static>
                    where F: GeneralListener + Clone {

                    for store in self {
                        store.subtree_general_listener(f.clone(), s);
                    }

                    Some(move |_v: &Vec<T>, w: &Word<VecActionBasis<T>>, s: Slock| {
                        for a in w.iter() {
                            match a {
                                VecActionBasis::Insert(store, _) => {
                                    /* make sure it is updated of the listener */
                                    store.subtree_general_listener(f.clone(), s);
                                }
                                _ => {
                                    /* nothing necessary here either (only care about updates) */
                                }
                            }
                        }

                        /* only keep listening if the original still cares */
                        /* this does mean that extra calls are sent out at times */
                        f(s)
                    })
                }

                fn subtree_inverse_listener<F>(&self, f: F, s: Slock<impl ThreadMarker>)
                    -> Option<impl Send + FnMut(&Self, &Self::Action, Slock) -> bool + 'static>
                    where F: InverseListener + Clone {
                    for store in self {
                        store.subtree_inverse_listener(f.clone(), s);
                    }

                    Some(move |_v: &Vec<T>, w: &Word<VecActionBasis<T>>, s: Slock| {
                        for a in w.iter() {
                            match a {
                                VecActionBasis::Insert(store, _) => {
                                    /* make sure it is updated of the inverse listener */
                                    store.subtree_inverse_listener(f.clone(), s);
                                }
                                _ => {
                                    /* nothing necessary here either (only care about updates) */
                                }
                            }
                        }

                        // no way around this, must subscribe forever (??)
                        // realistically not a huge issue though anyways
                        true
                    })
                }
            }
        }
        pub use vec_action::*;

        mod vector_action {
            use std::array;
            use std::ops::Mul;
            use crate::state::{GroupAction, GroupBasis, IntoAction, Stateful};
            use crate::util::marker::FalseMarker;
            use crate::util::Vector;

            pub struct VectorAction<T, const N: usize>
                where T: Stateful<HasInnerStores=FalseMarker>
            {
                actions: [T::Action; N]
            }

            impl<T, const N: usize> VectorAction<T, N>
                where T: Stateful<HasInnerStores=FalseMarker>
            {
                fn from_array(arr: [T::Action; N]) -> Self {
                    VectorAction {
                        actions: arr
                    }
                }
            }

            impl<T, const N: usize> GroupBasis<Vector<T, N>> for VectorAction<T, N>
                where T: Stateful<HasInnerStores=FalseMarker>
            {
                fn apply(self, to: &mut Vector<T, N>) -> Self {
                    let mut ret_actions: [T::Action; N] = array::from_fn(|_| T::Action::identity());

                    for (i, (action, target)) in
                    std::iter::zip(self.actions, &mut to.0).enumerate()
                    {
                        ret_actions[i] = action.apply(target);
                    }

                    VectorAction {
                        actions: ret_actions
                    }
                }

                fn forward_description(&self) -> impl Into<String> {
                    "Change"
                }

                fn backward_description(&self) -> impl Into<String> {
                    "Change"
                }
            }

            impl<T, const N: usize> Mul for VectorAction<T, N>
                where T: Stateful<HasInnerStores=FalseMarker>
            {
                type Output = Self;

                fn mul(self, rhs: Self) -> Self::Output {
                    let mut ret_actions: [T::Action; N] = array::from_fn(|_| T::Action::identity());

                    for (i, (lhs, rhs)) in
                        std::iter::zip(self.actions, rhs.actions).enumerate()
                    {
                        ret_actions[i] = lhs * rhs
                    }

                    VectorAction {
                        actions: ret_actions
                    }
                }
            }

            impl<T, const N: usize> GroupAction<Vector<T, N>> for VectorAction<T, N>
                where T: Stateful<HasInnerStores=FalseMarker>
            {
                fn identity() -> Self {
                    VectorAction {
                        actions: array::from_fn(|_| T::Action::identity())
                    }
                }
            }
            impl<U, T, const N: usize> IntoAction<VectorAction<T, N>, Vector<T, N>> for [U; N]
                where U: IntoAction<T::Action, T>, T: Stateful<HasInnerStores=FalseMarker>
            {
                fn into_action(self, target: &Vector<T, N>) -> VectorAction<T, N> {
                    let mut i = 0;
                    VectorAction::from_array(self.map(|action| {
                        let ret = action.into_action(&target.0[i]);
                        i += 1;
                        ret
                    }))
                }
            }

            impl<T, const N: usize> Stateful for Vector<T, N>
                where T: Stateful<HasInnerStores=FalseMarker>
            {
                type Action = VectorAction<T, N>;
                // it has inner stateful
                // but not inner STORES
                // thus we can say false
                type HasInnerStores = FalseMarker;

                // no need for subtree listeners (general/inverse)
                // Since T::HasInnerStores == false
            }
        }

        // pseudo action that converts into set action
        mod numeric_action {
            use std::ops::{Add, Mul, Sub};
            use crate::state::{IntoAction, SetAction, Stateful};

            /// Not an actual action; merely meant to convert to
            /// SetAction
            pub enum NumericAction<T>
                where T: Stateful + Copy + Add<Output=T> + Mul<Output=T> + Sub<Output=T>
            {
                Set(T),
                Incr(T),
                Decr(T),
                Mul(T),
                Identity
            }

            impl<T> IntoAction<SetAction<T>, T> for NumericAction<T>
                where T: Stateful<Action=SetAction<T>> + Copy + Add<Output=T> + Mul<Output=T> + Sub<Output=T> {
                fn into_action(self, target: &T) -> T::Action {
                    match self {
                        NumericAction::Identity => SetAction::Identity,
                        NumericAction::Set(val) => SetAction::Set(val),
                        NumericAction::Incr(val) => SetAction::Set(*target + val),
                        NumericAction::Decr(val) => SetAction::Set(*target - val),
                        NumericAction::Mul(val) => SetAction::Set(*target * val),
                    }
                }
            }
        }
        pub use numeric_action::*;
    }
    pub use action::*;
}
pub use group::*;

pub mod capacitor {
    use std::collections::VecDeque;
    use std::ops::{Add, Sub};
    use std::time::Duration;
    use crate::state::Stateful;
    use crate::util::numeric::{Lerp, Norm};

    pub trait Capacitor: Send + 'static {
        type Target: Stateful;

        fn target_set(&mut self, target: &Self::Target, span_time: Option<Duration>);

        /// Precondition: Must only be called after set_target has been called one or more times
        /// second parameter is whether or not to continue
        fn sample(&mut self, span_time: Duration) -> (Self::Target, bool);
    }

    // A degenerate capacitor used for ClockSignal
    pub struct IncreasingCapacitor;

    impl Capacitor for IncreasingCapacitor {
        type Target = f64;

        fn target_set(&mut self, _target: &Self::Target, _span_time: Option<Duration>) {
            // no op
        }

        fn sample(&mut self, span_time: Duration) -> (Self::Target, bool) {
            (span_time.as_secs_f64(), true)
        }
    }

    struct ConstantTimeInner<T>
        where T: Stateful + Lerp
    {
        start_time: f64,
        from: T,
        target: T
    }

    pub struct ConstantTimeCapacitor<T>
        where T: Stateful + Lerp + Copy
    {
        time: f64,
        inner: Option<ConstantTimeInner<T>>

    }
    impl<T> ConstantTimeCapacitor<T>
        where T: Stateful + Lerp + Copy
    {
        pub fn new(time: f64) -> Self {
            assert!(time > 1e-3, "Time too small");

            ConstantTimeCapacitor {
                time,
                inner: None
            }
        }
    }

    impl<T> Capacitor for ConstantTimeCapacitor<T>
        where T: Stateful + Lerp + Copy
    {
        type Target = T;

        fn target_set(&mut self, target: &Self::Target, span_time: Option<Duration>) {
            if let Some(ref mut inner) = self.inner {
                inner.from = if let Some(curr) = span_time {
                    let alpha = (curr.as_secs_f64() - inner.start_time).min(1.0);
                    T::lerp(inner.from, alpha, inner.target)
                } else {
                    // not currently active
                    inner.target
                };
                inner.target = *target;
                inner.start_time = span_time.map(|t| t.as_secs_f64()).unwrap_or(0.0);
            }
            else {
                self.inner = Some(ConstantTimeInner {
                    // mark it as already finished
                    start_time: -self.time,
                    from: *target,
                    target: *target,
                })
            }
        }

        fn sample(&mut self, span_time: Duration) -> (Self::Target, bool) {
            let inner = self.inner.as_ref().unwrap();

            let alpha = (span_time.as_secs_f64() - inner.start_time) / self.time;
            if alpha > 1.0 {
                (inner.target, false)
            }
            else {
                (T::lerp(inner.from, alpha, inner.target), true)
            }
        }
    }

    struct ConstantSpeedInner<T>
        where T: Stateful + Lerp + Norm + Sub<Output=T> + Copy
    {
        start_time: f64,
        end_time: f64,
        from: T,
        target: T
    }

    pub struct ConstantSpeedCapacitor<T>
        where T: Stateful + Lerp + Norm + Sub<Output=T> + Copy
    {
        speed: f64,
        inner: Option<ConstantSpeedInner<T>>
    }

    impl<T> ConstantSpeedCapacitor<T>
        where T: Stateful + Lerp + Norm + Sub<Output=T> + Copy
    {
        pub fn new(speed: f64) -> Self {
            assert!(speed > 0.0, "speed must be positive");

            ConstantSpeedCapacitor {
                speed,
                inner: None
            }
        }
    }

    impl<T> Capacitor for ConstantSpeedCapacitor<T>
        where T: Stateful + Lerp + Norm + Sub<Output=T> + Copy
    {
        type Target = T;

        fn target_set(&mut self, target: &Self::Target, span_time: Option<Duration>) {
            if let Some(ref mut inner) = self.inner {
                inner.from = if let Some(curr) = span_time {
                    let total = inner.end_time - inner.start_time;
                    let alpha = (curr.as_secs_f64() - inner.start_time) / total;
                    T::lerp(inner.from, alpha.min(1.0), inner.target)
                } else {
                    // not currently active
                    inner.target
                };

                inner.target = *target;
                // if start of span, set duration to be 0
                inner.start_time = span_time.map(|t| t.as_secs_f64()).unwrap_or(0.0);

                inner.end_time = {
                    let norm = (*target - inner.from).norm();
                    let time = norm / self.speed;

                    inner.start_time + time
                };
            }
            else {
                self.inner = Some(ConstantSpeedInner {
                    start_time: -2.0,
                    // some time in the past so it instantly finishes
                    end_time: -1.0,
                    from: *target,
                    target: *target,
                })
            }
        }

        fn sample(&mut self, span_time: Duration) -> (Self::Target, bool) {
            let inner = self.inner.as_ref().unwrap();

            let alpha = (span_time.as_secs_f64() - inner.start_time) / (inner.end_time - inner.start_time);

            if alpha > 1.0 {
                (inner.target, false)
            }
            else {
                (T::lerp(inner.from, alpha, inner.target), true)
            }
        }
    }

    pub struct SmoothCapacitor<T, F>
        where T: Stateful + Lerp + Copy, F: Fn(f64) -> f64 + Send + 'static
    {
        ease_function: F,
        trans_time: f64,
        points: VecDeque<(f64, T)>,
    }

    impl<T, F> SmoothCapacitor<T, F>
        where T: Stateful + Lerp + Copy + Add<Output=T> + Sub<Output=T>,
              F: Fn(f64) -> f64 + Send + 'static
    {
        pub fn new(func: F, time: f64) -> Self {
            assert!(time > 1e-3);

            SmoothCapacitor {
                ease_function: func,
                trans_time: time,
                points: VecDeque::new()
            }
        }

    }
    // otherwise it's ambiguous implementation
    // so just need to fix the F to something
    impl<T> SmoothCapacitor<T, fn(f64) -> f64>
        where T: Stateful + Lerp + Copy + Add<Output=T> + Sub<Output=T>,
    {
        pub fn ease_in_out(time: f64) -> SmoothCapacitor<T, impl Fn(f64) -> f64> {
            SmoothCapacitor::new(|t| 3.0 * t * t - 2.0 * t * t * t, time)
        }

        pub fn ease_in(time: f64) -> SmoothCapacitor<T, impl Fn(f64) -> f64> {
            SmoothCapacitor::new(|t| t * t, time)
        }

        pub fn ease_out(time: f64) -> SmoothCapacitor<T, impl Fn(f64) -> f64> {
            SmoothCapacitor::new(|t| 1.0 - (t - 1.0) * (t - 1.0), time)
        }
    }

    impl<T, F> Capacitor for SmoothCapacitor<T, F>
        where T: Stateful + Lerp + Copy + Add<Output=T> + Sub<Output=T>,
              F: Fn(f64) -> f64 + Send + 'static
    {
        type Target = T;

        fn target_set(&mut self, target: &Self::Target, span_time: Option<Duration>) {
            if !self.points.is_empty() {
                self.points.push_back(
                    (span_time.map(|t| t.as_secs_f64()).unwrap_or(0.0), *target)
                );
            }
            else {
                self.points.push_back((-self.trans_time, *target));
            }
        }

        fn sample(&mut self, span_time: Duration) -> (Self::Target, bool) {
            let time = span_time.as_secs_f64();
            while self.points.len() > 2 && self.points[1].0 + self.trans_time <= time {
                self.points.pop_front();
            }

            let mut val = self.points[0].1;
            for i in 0 .. self.points.len() - 1 {
                let diff = self.points[i + 1].1 - self.points[i].1;
                let alpha = (self.ease_function)((time - self.points[i + 1].0) / self.trans_time).min(1.0);

                val = T::lerp(val, alpha, val + diff)
            }

            let cont = self.points.back().unwrap().0 + self.trans_time >= time;
            if !cont {
                // clear all of previous span
                while self.points.len() > 1 {
                    self.points.pop_front();
                }
            }
            (val, cont)
        }
    }
}

mod store {
    use std::cell::Cell;
    use crate::core::{Slock};
    use crate::state::{StateFilter, IntoAction, Signal, Stateful};
    use crate::state::listener::{GeneralListener, InverseListener};
    use crate::util::marker::ThreadMarker;

    thread_local! {
        static EXECUTING_INVERSE: Cell<bool> = Cell::new(false);
    }

    /// It is the implementors job to guarantee that subtree_listener
    /// and relatives do not get into call cycles
    pub trait StoreContainer: Send + Sized + 'static {
        // Only ONE general listener
        // can ever be present for a subtree
        fn subtree_general_listener<F: GeneralListener + Clone>(&self, f: F, s: Slock<impl ThreadMarker>);

        // Only ONE active general listener
        // can ever be present for a subtree
        fn subtree_inverse_listener<F: InverseListener + Clone>(&self, f: F, s: Slock<impl ThreadMarker>);
    }

    // FIXME at some point, this and RawStore[SharedOwner] should
    // have F as associated type rather than input parameter
    pub trait Filterable<S: Stateful> {
        fn action_filter<G>(&self, filter: G, s: Slock<impl ThreadMarker>)
            where G: Send + Fn(&S, S::Action, Slock) -> S::Action + 'static;
    }

    // Like with signal, I believe it makes more sense for
    pub trait Binding<F>: Signal<Target=F::Target> + Sized + Send + 'static where F: StateFilter {
        fn is_applying(&self, s: Slock<impl ThreadMarker>) -> bool;
        fn apply(&self, action: impl IntoAction<<<F as StateFilter>::Target as Stateful>::Action, <F as StateFilter>::Target>, s: Slock<impl ThreadMarker>);
        fn apply_coupled(&self, action: impl IntoAction<<<F as StateFilter>::Target as Stateful>::Action, <F as StateFilter>::Target>, s: Slock<impl ThreadMarker>) {
            if !self.is_applying(s) {
                self.apply(action, s);
            }
        }

        fn action_listener<G>(&self, listener: G, s: Slock<impl ThreadMarker>)
            where G: Send + FnMut(&F::Target, &<<F as StateFilter>::Target as Stateful>::Action, Slock) -> bool + 'static;

        type WeakBinding: WeakBinding<F>;
        fn downgrade(&self) -> Self::WeakBinding;
    }

    pub trait WeakBinding<F>: Clone where F: StateFilter {
        type Binding: Binding<F>;
        fn upgrade(&self) -> Option<Self::Binding>;
    }

    pub trait Bindable<F> where F: StateFilter {
        type Binding: Binding<F> + Clone;
        type WeakBinding: WeakBinding<F>;

        fn binding(&self) -> Self::Binding;
        fn weak_binding(&self) -> Self::WeakBinding;
    }

    mod raw_store {
        use std::sync::Arc;
        use crate::core::Slock;
        use crate::state::slock_cell::SlockCell;
        use crate::state::{IntoAction, StateFilter, Stateful};
        use crate::state::store::store_dispatcher::StoreDispatcher;
        use crate::util::marker::ThreadMarker;

        pub(super) trait RawStore<F: StateFilter>: Sized + Send + 'static {
            type InverseListenerHolder: super::inverse_listener_holder::InverseListenerHolder;

            fn dispatcher(&self) -> &StoreDispatcher<F::Target, F, Self::InverseListenerHolder>;

            fn dispatcher_mut(&mut self) -> &mut StoreDispatcher<F::Target, F, Self::InverseListenerHolder>;

            // may introduce some additional behavior that the dispatcher does not handle
            fn apply(inner: &Arc<SlockCell<Self>>, action: impl IntoAction<<F::Target as Stateful>::Action, F::Target>, is_inverted_action: bool, s: Slock<impl ThreadMarker>);

            // Must be careful with these two methods
            // since generally not called with the state lock
            fn strong_count_decrement(_this: &Arc<SlockCell<Self>>) {

            }

            fn strong_count_increment(_this: &Arc<SlockCell<Self>>) {

            }
        }
    }

    pub(super) mod raw_store_shared_owner {
        use std::marker::PhantomData;
        use std::sync::Arc;
        use crate::core::{Slock};
        use crate::state::{StateFilter, Filter, Filterable, Stateful, Signal, Binding, IntoAction};
        use crate::state::listener::StateListener;
        use crate::state::slock_cell::SlockCell;
        use crate::state::store::general_binding::GeneralWeakBinding;
        use crate::state::store::raw_store::RawStore;
        use crate::util::marker::ThreadMarker;

        pub(super) trait RawStoreSharedOwner<F: StateFilter> : Signal<Target=F::Target> + Sync {
            type Inner: RawStore<F>;

            fn from_ref(arc: Arc<SlockCell<Self::Inner>>) -> Self;

            fn inner_ref(&self) -> &Arc<SlockCell<Self::Inner>>;

            // guaranteed to only be used for creating the binding
            // This does not need to call strong_count_increment
            // caller is expected to do so
            fn arc_clone(&self) -> Self;
        }

        impl<I, S: Stateful> Filterable<S> for I where I: RawStoreSharedOwner<Filter<S>> {

            fn action_filter<G>(&self, filter: G, s: Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, Slock) -> S::Action + 'static {
                self.inner_ref().borrow_mut(s).dispatcher_mut().action_filter(filter, s);
            }
        }

        // Unfortunately can't do this for signal as well
        // Since FixedSignal 'might' implement RawStoreSharedOwnerBase
        // It's therefore done as macros
        impl<F, R> Binding<F> for R where F: StateFilter, R: RawStoreSharedOwner<F> {
            fn is_applying(&self, s: Slock<impl ThreadMarker>) -> bool {
                self.inner_ref().is_borrowed_mut(s)
            }

            fn apply(&self, action: impl IntoAction<<<F as StateFilter>::Target as Stateful>::Action, <F as StateFilter>::Target>, s: Slock<impl ThreadMarker>) {
                R::Inner::apply(self.inner_ref(), action, false, s)
            }

            fn action_listener<G>(&self, listener: G, s: Slock<impl ThreadMarker>) where G: Send + FnMut(&F::Target, &<<F as StateFilter>::Target as Stateful>::Action, Slock) -> bool + 'static {
                self.inner_ref().borrow_mut(s).dispatcher_mut().add_listener(StateListener::ActionListener(Box::new(listener)));
            }
            type WeakBinding = GeneralWeakBinding<F, R>;

            fn downgrade(&self) -> Self::WeakBinding {
                GeneralWeakBinding {
                    weak: Arc::downgrade(self.inner_ref()),
                    phantom: PhantomData
                }
            }
        }
    }

    /* MARK: utilities */
    mod action_inverter {
        use std::marker::PhantomData;
        use std::sync::Weak;
        use crate::core::Slock;
        use crate::state::listener::DirectlyInvertible;
        use crate::state::slock_cell::SlockCell;
        use crate::state::{StateFilter, Stateful};
        use crate::state::store::raw_store::RawStore;

        pub(super) struct ActionInverter<F: StateFilter, I> where I: RawStore<F> {
            action: Option<<F::Target as Stateful>::Action>,
            state: Weak<SlockCell<I>>,
            phantom: PhantomData<F>
        }

        impl<F, I> ActionInverter<F, I> where F: StateFilter, I: RawStore<F> {
            pub(super) fn new(action: <F::Target as Stateful>::Action, weak: Weak<SlockCell<I>>) -> Self {
                ActionInverter {
                    action: Some(action),
                    state: weak,
                    phantom: PhantomData,
                }
            }
        }

        impl<F, I> DirectlyInvertible for ActionInverter<F, I> where F: StateFilter, I: RawStore<F> {
            fn invert(&mut self, s: Slock) {
                let Some(state) = self.state.upgrade() else {
                    return;
                };

                I::apply(&state, self.action.take().unwrap(), true, s);
            }

            unsafe fn right_multiply(&mut self, mut by: Box<dyn DirectlyInvertible>, s: Slock) {
                /* we are free to assume by is of type Self, allowing us to do this conversion */
                let ptr = by.action_pointer(s) as *const <F::Target as Stateful>::Action;
                self.action = Some(self.action.take().unwrap() * std::ptr::read(ptr));
                /* we have implicitly moved the other's action, now we must tell it to forget to
                   avoid double free
                 */
                by.forget_action(s);
            }

            unsafe fn action_pointer(&self, _s: Slock) -> *const () {
                self.action.as_ref().unwrap() as *const <F::Target as Stateful>::Action as *const ()
            }

            unsafe fn forget_action(&mut self, _s: Slock) {
                std::mem::forget(self.action.take());
            }

            fn id(&self) -> usize {
                self.state.as_ptr() as usize
            }
        }
    }

    mod state_ref {
        use std::cell::Ref;
        use std::marker::PhantomData;
        use std::ops::Deref;
        use crate::state::StateFilter;
        use crate::state::store::raw_store::RawStore;

        pub(super) struct StateRef<'a, F, I> where F: StateFilter, I: RawStore<F> {
            pub(super) main_ref: Ref<'a, I>,
            pub(super) phantom: PhantomData<F>
        }

        impl<'a, F, I> Deref for StateRef<'a, F, I>
            where F: StateFilter, I: RawStore<F> {
            type Target = F::Target;
            fn deref(&self) -> &F::Target {
                self.main_ref.dispatcher().data()
            }
        }
    }

    mod inverse_listener_holder {
        use crate::core::Slock;
        use crate::state::{DirectlyInvertible};
        use crate::state::listener::BoxInverseListener;

        pub(super) trait InverseListenerHolder {
            fn new() -> Self;
            fn set_listener(&mut self, listener: BoxInverseListener);

            fn invoke_listener(&mut self, action: impl FnOnce() -> Box<dyn DirectlyInvertible>, s: Slock);
        }

        pub(super) struct NullInverseListenerHolder;

        impl InverseListenerHolder for NullInverseListenerHolder {
            fn new() -> Self {
                NullInverseListenerHolder
            }

            fn set_listener(&mut self, _listener: BoxInverseListener) {

            }

            fn invoke_listener(&mut self, _action: impl FnOnce() -> Box<dyn DirectlyInvertible>, _s: Slock) {

            }
        }

        pub(super) struct InverseListenerHolderImpl(Option<BoxInverseListener>);

        impl InverseListenerHolder for InverseListenerHolderImpl {
            fn new() -> Self {
                InverseListenerHolderImpl(None)
            }

            fn set_listener(&mut self, listener: BoxInverseListener) {
                self.0 = Some(listener);
            }

            fn invoke_listener(&mut self, action: impl FnOnce() -> Box<dyn DirectlyInvertible>, s: Slock) {
                if let Some(ref mut func) = self.0 {
                    if !func(action(), s) {
                        self.0 = None;
                    }
                }
            }
        }
    }

    mod store_dispatcher {
        use crate::core::Slock;
        use crate::state::{StateFilter, DirectlyInvertible, GeneralListener, GroupBasis, IntoAction, InverseListener, Stateful};
        use crate::state::listener::{ StateListener};
        use crate::state::store::inverse_listener_holder::InverseListenerHolder;
        use crate::util::marker::ThreadMarker;
        use crate::util::test_util::QuarveAllocTag;

        pub(crate) struct StoreDispatcher<S, F, I>
            where S: Stateful, F: StateFilter, I: InverseListenerHolder
        {
            _quarve_tag: QuarveAllocTag,
            data: S,
            listeners: Vec<StateListener<S>>,
            inverse_listener: I,
            filter: F,
        }

        impl<S, F, I> StoreDispatcher<S, F, I>
            where S: Stateful, F: StateFilter<Target=S>, I: InverseListenerHolder {

            #[inline]
            pub(super) fn new(data: S) -> Self {
                StoreDispatcher {
                    _quarve_tag: QuarveAllocTag::new(),
                    data,
                    listeners: Vec::new(),
                    inverse_listener: I::new(),
                    filter: F::new(),
                }
            }

            #[inline]
            pub fn data(&self) -> &S {
                &self.data
            }

            #[inline]
            pub fn apply_post_filter(
                &mut self,
                into_action: impl IntoAction<S::Action, S>,
                make_inverter: impl FnOnce(S::Action) -> Box<dyn DirectlyInvertible>,
                post_filter: impl FnOnce(&S, S::Action) -> S::Action,
                skip_filters: bool,
                s: Slock<impl ThreadMarker>
            ) {
                let transaction = into_action.into_action(&self.data);

                let filtered_action = if skip_filters {
                    transaction
                }
                else {
                    post_filter(&self.data, self.filter.filter(&self.data, transaction, s))
                };

                // tell action listeners
                self.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::ActionListener(listener) => listener(&self.data, &filtered_action, s.to_general_slock()),
                        _ => true
                    }
                );

                // apply action
                let inverse = filtered_action.apply(&mut self.data);

                // tell signal and general listeners
                let data = &self.data;
                self.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::GeneralListener(action) => {
                            action(s.to_general_slock())
                        },
                        StateListener::SignalListener(action) => {
                            action(data, s.to_general_slock())
                        },
                        _ => true
                    }
                );

                // tell inverse listener
                self.inverse_listener.invoke_listener(move || make_inverter(inverse), s.to_general_slock());
            }

            pub fn apply(
                &mut self,
                into_action: impl IntoAction<S::Action, S>,
                make_inverter: impl FnOnce(S::Action) -> Box<dyn DirectlyInvertible>,
                skip_filters: bool,
                s: Slock<impl ThreadMarker>
            ) {
                self.apply_post_filter(into_action, make_inverter, |_, f| f, skip_filters, s);
            }

            pub fn add_listener(&mut self, listener: StateListener<S>) {
                debug_assert!(! matches!(listener, StateListener::GeneralListener(_)),
                              "Should be set via set_general_listener"
                );
                self.listeners.push(listener);
            }

            pub fn action_filter<G>(&mut self, filter: G, _s: Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, Slock) -> S::Action + 'static {
                self.filter.add_filter(filter);
            }

            pub fn set_general_listener(&mut self, f: impl GeneralListener + Clone, s: Slock) {
                self.listeners.retain(|x| !matches!(x, StateListener::GeneralListener(_)));
                self.listeners.push(StateListener::GeneralListener(Box::new(f.clone())));

                if let Some(action) = self.data.subtree_general_listener(f, s) {
                    self.listeners.push(StateListener::ActionListener(Box::new(action)));
                }
            }

            pub fn set_inverse_listener(&mut self, f: impl InverseListener + Clone, s: Slock) {
                self.inverse_listener.set_listener(Box::new(f.clone()));

                if let Some(action) = self.data.subtree_inverse_listener(f, s) {
                    self.listeners.push(StateListener::ActionListener(Box::new(action)));
                }
            }
        }
    }

    mod macros {
        macro_rules! impl_store_container_inner {
            () => {
                fn subtree_general_listener<Q>(&self, f: Q, s: Slock<impl ThreadMarker>)
                    where Q: GeneralListener + Clone {
                    self.inner.borrow_mut(s).dispatcher_mut().set_general_listener(f, s.to_general_slock());
                }

                fn subtree_inverse_listener<Q>(&self, f: Q, s: Slock<impl ThreadMarker>)
                    where Q: InverseListener + Clone {
                    self.inner.borrow_mut(s).dispatcher_mut().set_inverse_listener(f, s.to_general_slock());
                }
            }
        }

        macro_rules! impl_bindable_inner {
            ($s:ty, $f:ty) => {
                type Binding = GeneralBinding<$f, Self>;
                type WeakBinding = GeneralWeakBinding<$f, Self>;

                fn binding(&self) -> Self::Binding {
                    <Self as RawStoreSharedOwner<$f>>::Inner::strong_count_increment(self.inner_ref());

                    GeneralBinding {
                        inner: self.arc_clone(),
                        phantom: PhantomData,
                    }
                }

                fn weak_binding(&self) -> Self::WeakBinding {
                    let ret: GeneralWeakBinding<$f, Self> = GeneralWeakBinding {
                        weak: Arc::downgrade(self.inner_ref()),
                        phantom: PhantomData
                    };
                    ret
                }

            }
        }

        macro_rules! impl_adhoc_inner {
            ($s:ty, $f:ty) => {
                pub fn signal(&self) -> impl Signal<Target=$s> + Clone {
                    <Self as RawStoreSharedOwner<$f>>::Inner::strong_count_increment(self.inner_ref());

                    GeneralBinding {
                        inner: self.arc_clone(),
                        phantom: PhantomData,
                    }
                }
            }
        }

        macro_rules! impl_signal_inner {
            ($s:ty) => {
                type Target=$s;

                fn borrow<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=$s> + 'a {
                    StateRef {
                        main_ref: self.inner_ref().borrow(s),
                        phantom: PhantomData
                    }
                }

                fn listen<Q>(&self, listener: Q, s: Slock<impl ThreadMarker>)
                    where Q: FnMut(&$s, Slock) -> bool + Send + 'static {
                    self.inner_ref().borrow_mut(s).dispatcher_mut().add_listener(StateListener::SignalListener(Box::new(listener)));
                }

                // non trait method so it's fine to just return impl Signal
                type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
                fn map<U, Q>(&self, map: Q, s: Slock<impl ThreadMarker>) -> Self::MappedOutput<U>
                    where U: Send + 'static, Q: Send + 'static + Fn(&$s) -> U {
                    GeneralSignal::from(self, self, map, |this, listener, s| {
                        this.inner_ref().borrow_mut(s).dispatcher_mut().add_listener(StateListener::SignalListener(listener))
                    }, s)
                }
            };
        }

        pub(super) use {impl_store_container_inner, impl_signal_inner, impl_bindable_inner, impl_adhoc_inner};
    }

    /* MARK: Stores */
    mod store {
        use std::marker::PhantomData;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::state::{Bindable, StateListener};
        use crate::state::store::state_ref::StateRef;
        use crate::{
            state::{StateFilter, Filterless, IntoAction, Signal, Stateful, StoreContainer, GeneralSignal},
            core::Slock,
        };
        use crate::state::{Filter};
        use crate::state::listener::{GeneralListener, InverseListener};
        use crate::state::slock_cell::SlockCell;
        use crate::state::store::action_inverter::ActionInverter;
        use crate::state::store::EXECUTING_INVERSE;
        use crate::state::store::general_binding::{GeneralBinding, GeneralWeakBinding};
        use crate::state::store::inverse_listener_holder::InverseListenerHolderImpl;
        use crate::state::store::macros::{impl_adhoc_inner, impl_bindable_inner, impl_signal_inner, impl_store_container_inner};
        use crate::state::store::raw_store::RawStore;
        use crate::state::store::raw_store_shared_owner::RawStoreSharedOwner;
        use crate::state::store::store_dispatcher::StoreDispatcher;
        use crate::util::marker::ThreadMarker;

        pub(super) struct InnerStore<S: Stateful, F: StateFilter<Target=S>> {
            dispatcher: StoreDispatcher<S, F, InverseListenerHolderImpl>
        }

        impl<S, F> RawStore<F> for InnerStore<S, F>
            where S: Stateful, F: StateFilter<Target=S>
        {
            type InverseListenerHolder = InverseListenerHolderImpl;

            fn dispatcher(&self) -> &StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &self.dispatcher
            }

            fn dispatcher_mut(&mut self) -> &mut StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &mut self.dispatcher
            }

            fn apply(arc: &Arc<SlockCell<Self>>, alt_action: impl IntoAction<S::Action, S>, is_inverse: bool, s: Slock<impl ThreadMarker>) {
                if EXECUTING_INVERSE.get() {
                    // do not execute while inverse is acting
                    return;
                }
                else if is_inverse {
                    EXECUTING_INVERSE.set(true);
                }

                let mut borrow = arc.borrow_mut(s);
                let inner = borrow.deref_mut();

                inner.dispatcher.apply(alt_action, |action| {
                    Box::new(ActionInverter::new(action, Arc::downgrade(&arc)))
                }, is_inverse, s);

                if is_inverse {
                    EXECUTING_INVERSE.set(false);
                }
            }
        }

        pub struct Store<S: Stateful, F: StateFilter<Target=S>=Filterless<S>>
        {
            pub(super) inner: Arc<SlockCell<InnerStore<S, F>>>
        }

        impl<S> Store<S, Filterless<S>>
            where S: Stateful
        {
            pub fn new(initial: S) -> Self {
                Store {
                    inner: Arc::new(SlockCell::new(InnerStore {
                        dispatcher: StoreDispatcher::new(initial)
                    }))
                }
            }
        }

        impl<S> Store<S, Filter<S>>
            where S: Stateful
        {
            pub fn new_with_filter(initial: S) -> Self {
                Store {
                    inner: Arc::new(SlockCell::new(InnerStore {
                        dispatcher: StoreDispatcher::new(initial)
                    }))
                }
            }
        }

        impl<S> Default for Store<S, Filterless<S>>
            where S: Stateful + Default
        {
            fn default() -> Self {
                Self::new(S::default())
            }
        }

        impl<S, M> StoreContainer for Store<S, M>
            where S: Stateful, M: StateFilter<Target=S>
        {
            impl_store_container_inner!();
        }

        impl<S, M> Bindable<M> for Store<S, M>
            where S: Stateful, M: StateFilter<Target=S>
        {
            impl_bindable_inner!(S, M);
        }

        impl<S, M> Store<S, M>
            where S: Stateful, M: StateFilter<Target=S>
        {
            impl_adhoc_inner!(S, M);
        }

        impl<S, M> Signal for Store<S, M>
            where S: Stateful, M: StateFilter<Target=S>
        {
            impl_signal_inner!(S);
        }

        impl<S, F> RawStoreSharedOwner<F> for Store<S, F>
            where S: Stateful, F: StateFilter<Target=S>
        {
            type Inner = InnerStore<S, F>;

            fn from_ref(arc: Arc<SlockCell<Self::Inner>>) -> Self {
                Store {
                    inner: arc,
                }
            }

            fn inner_ref(&self) -> &Arc<SlockCell<Self::Inner>> {
                &self.inner
            }

            fn arc_clone(&self) -> Self {
                Store {
                    inner: Arc::clone(&self.inner)
                }
            }
        }
    }
    pub use store::*;

    mod token_store {
        use std::marker::PhantomData;
        use std::collections::HashMap;
        use std::hash::Hash;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::state::{Bindable, StateListener};
        use crate::state::store::state_ref::StateRef;
        use crate::core::{Slock};
        use crate::state::{StateFilter, Filter, Filterless, GeneralListener, GeneralSignal, IntoAction, Signal, Stateful, StoreContainer};
        use crate::state::store::action_inverter::ActionInverter;
        use crate::state::store::inverse_listener_holder::InverseListenerHolderImpl;
        use crate::state::store::macros::{impl_adhoc_inner, impl_bindable_inner, impl_signal_inner, impl_store_container_inner};
        use crate::state::store::store_dispatcher::StoreDispatcher;
        use crate::state::InverseListener;
        use crate::state::slock_cell::SlockCell;
        use crate::state::store::EXECUTING_INVERSE;
        use crate::state::store::general_binding::{GeneralBinding, GeneralWeakBinding};
        use crate::state::store::raw_store::RawStore;
        use crate::state::store::raw_store_shared_owner::RawStoreSharedOwner;
        use crate::util::marker::ThreadMarker;

        pub(super) struct InnerTokenStore<S: Stateful + Copy + Hash + Eq, F: StateFilter<Target=S>> {
            dispatcher: StoreDispatcher<S, F, InverseListenerHolderImpl>,
            equal_listeners: HashMap<S, Vec<Box<dyn FnMut(&S, Slock) -> bool + Send>>>,
        }
        impl<S, F> RawStore<F> for InnerTokenStore<S, F>
            where S: Stateful + Copy + Hash + Eq,
                  F: StateFilter<Target=S>
        {
            type InverseListenerHolder = InverseListenerHolderImpl;

            fn dispatcher(&self) -> &StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &self.dispatcher
            }

            fn dispatcher_mut(&mut self) -> &mut StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &mut self.dispatcher
            }

            fn apply(arc: &Arc<SlockCell<Self>>, alt_action: impl IntoAction<S::Action, S>, is_inverse: bool, s: Slock<impl ThreadMarker>) {
                if EXECUTING_INVERSE.get() {
                    // do not execute while inverse is acting
                    return;
                }
                else if is_inverse {
                    EXECUTING_INVERSE.set(true);
                }


                let mut borrow = arc.borrow_mut(s);
                let inner = borrow.deref_mut();

                let old = *inner.dispatcher.data();

                inner.dispatcher.apply(alt_action, |action| {
                    Box::new(ActionInverter::new(action, Arc::downgrade(&arc)))
                }, is_inverse, s);

                // relevant equal listeners (old and new)
                let new = *inner.dispatcher.data();

                if old != new {
                    for class in [old, new] {
                        inner.equal_listeners.entry(class)
                            .and_modify(|l|
                                l.retain_mut(|f| f(&new, s.to_general_slock()))
                            );
                    }
                }

                if is_inverse {
                    EXECUTING_INVERSE.set(false);
                }
            }
        }

        pub struct TokenStore<S, F=Filterless<S>>
            where S: Stateful + Copy + Hash + Eq, F: StateFilter<Target=S> {
            inner: Arc<SlockCell<InnerTokenStore<S, F>>>
        }

        impl<S, F> TokenStore<S, F> where S: Stateful + Copy + Hash + Eq, F: StateFilter<Target=S> {
            pub fn equals(&self, target: S, s: Slock<impl ThreadMarker>) -> impl Signal<Target=bool> + Clone {
                GeneralSignal::from(self, &self.signal(), move |u| *u == target,
                    |this, listener, _s | {
                        this.inner.borrow_mut(s).equal_listeners.entry(target)
                            .or_insert(Vec::new())
                            .push(listener);
                    },
                    s
                )
            }
        }

        impl<S> TokenStore<S, Filterless<S>>
            where S: Stateful + Copy + Hash + Eq {
            pub fn new(initial: S) -> Self {
                TokenStore {
                    inner: Arc::new(SlockCell::new(InnerTokenStore {
                        dispatcher: StoreDispatcher::new(initial),
                        equal_listeners: HashMap::new(),
                    }))
                }
            }
        }

        impl<S> TokenStore<S, Filter<S>>
            where S: Stateful + Copy + Hash + Eq {
            pub fn new_with_filter(initial: S) -> Self {
                TokenStore {
                    inner: Arc::new(SlockCell::new(InnerTokenStore {
                        dispatcher: StoreDispatcher::new(initial),
                        equal_listeners: HashMap::new(),
                    }))
                }
            }
        }

        impl<S> Default for TokenStore<S, Filterless<S>>
            where S: Default + Stateful + Copy + Hash + Eq {
            fn default() -> Self {
                Self::new(S::default())
            }
        }

        impl<S> Default for TokenStore<S, Filter<S>>
            where S: Default + Stateful + Copy + Hash + Eq {
            fn default() -> Self {
                Self::new_with_filter(S::default())
            }
        }

        impl<S, M> StoreContainer for TokenStore<S, M>
            where S: Stateful + Copy + Hash + Eq, M: StateFilter<Target=S> {
            impl_store_container_inner!();
        }

        impl<S, M> Bindable<M> for TokenStore<S, M>
            where S: Stateful + Copy + Hash + Eq, M: StateFilter<Target=S> {
            impl_bindable_inner!(S, M);
        }

        impl<S, M> TokenStore<S, M>
            where S: Stateful + Copy + Hash + Eq, M: StateFilter<Target=S> {
            impl_adhoc_inner!(S, M);
        }

        impl<S, M> Signal for TokenStore<S, M>
            where S: Stateful + Copy + Hash + Eq, M: StateFilter<Target=S> {
            impl_signal_inner!(S);
        }

        impl<S, A> RawStoreSharedOwner<A> for TokenStore<S, A>
            where S: Stateful + Copy + Hash + Eq, A: StateFilter<Target=S> {
            type Inner = InnerTokenStore<S, A>;

            fn from_ref(arc: Arc<SlockCell<Self::Inner>>) -> Self {
                TokenStore {
                    inner: arc,
                }
            }

            fn inner_ref(&self) -> &Arc<SlockCell<Self::Inner>> {
                &self.inner
            }

            fn arc_clone(&self) -> Self {
                TokenStore {
                    inner: Arc::clone(&self.inner)
                }
            }
        }
    }
    pub use token_store::*;

    mod derived_store {
        use std::marker::PhantomData;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::state::store::state_ref::StateRef;
        use crate::{
            state::{
                StateFilter, Filterless, IntoAction, Signal, Stateful, StoreContainer, GeneralSignal,
            },
            core::Slock,
        };
        use crate::state::{Bindable, StateListener};
        use crate::state::{Filter};
        use crate::state::listener::{GeneralListener, InverseListener};
        use crate::state::slock_cell::SlockCell;
        use crate::state::store::general_binding::{GeneralBinding, GeneralWeakBinding};
        use crate::state::store::inverse_listener_holder::NullInverseListenerHolder;
        use crate::state::store::macros::{impl_adhoc_inner, impl_bindable_inner, impl_signal_inner, impl_store_container_inner};
        use crate::state::store::raw_store::RawStore;
        use crate::state::store::raw_store_shared_owner::RawStoreSharedOwner;
        use crate::state::store::store_dispatcher::StoreDispatcher;
        use crate::util::marker::ThreadMarker;

        pub(super) struct InnerDerivedStore<S: Stateful, F: StateFilter<Target=S>> {
            dispatcher: StoreDispatcher<S, F, NullInverseListenerHolder>
        }

        impl<S, F> RawStore<F> for InnerDerivedStore<S, F>
            where S: Stateful, F: StateFilter<Target=S>
        {
            type InverseListenerHolder = NullInverseListenerHolder;

            fn dispatcher(&self) -> &StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &self.dispatcher
            }

            fn dispatcher_mut(&mut self) -> &mut StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &mut self.dispatcher
            }

            fn apply(arc: &Arc<SlockCell<Self>>, alt_action: impl IntoAction<S::Action, S>, is_inverse: bool, s: Slock<impl ThreadMarker>) {
                // note that derived store does not care whether or not the action is inverse or not
                // (except in the case of filters)

                let mut borrow = arc.borrow_mut(s);
                let inner = borrow.deref_mut();

                inner.dispatcher.apply(alt_action, |_| unreachable!(), is_inverse, s);
            }
        }

        pub struct DerivedStore<S: Stateful, F: StateFilter<Target=S>=Filterless<S>>
        {
            inner: Arc<SlockCell<InnerDerivedStore<S, F>>>
        }

        impl<S> DerivedStore<S, Filterless<S>>
            where S: Stateful
        {
            pub fn new(initial: S) -> Self {
                DerivedStore {
                    inner: Arc::new(SlockCell::new(InnerDerivedStore {
                        dispatcher: StoreDispatcher::new(initial)
                    }))
                }
            }
        }

        impl<S> DerivedStore<S, Filter<S>>
            where S: Stateful
        {
            pub fn new_with_filter(initial: S) -> Self {
                DerivedStore {
                    inner: Arc::new(SlockCell::new(InnerDerivedStore {
                        dispatcher: StoreDispatcher::new(initial),
                    }))
                }
            }
        }

        impl<S> Default for DerivedStore<S, Filterless<S>>
            where S: Stateful + Default
        {
            fn default() -> Self {
                Self::new(S::default())
            }
        }

        impl<S> Default for DerivedStore<S, Filter<S>>
            where S: Stateful + Default
        {
            fn default() -> Self {
                Self::new_with_filter(S::default())
            }
        }

        impl<S, M> StoreContainer for DerivedStore<S, M>
            where S: Stateful, M: StateFilter<Target=S>
        {
            impl_store_container_inner!();
        }

        impl<S, M> Bindable<M> for DerivedStore<S, M>
            where S: Stateful, M: StateFilter<Target=S>
        {
            impl_bindable_inner!(S, M);
        }

        impl<S, M> DerivedStore<S, M>
            where S: Stateful, M: StateFilter<Target=S>
        {
            impl_adhoc_inner!(S, M);
        }

        impl<S, M> Signal for DerivedStore<S, M>
            where S: Stateful, M: StateFilter<Target=S>
        {
            impl_signal_inner!(S);
        }

        impl<S, F> RawStoreSharedOwner<F> for DerivedStore<S, F>
            where S: Stateful, F: StateFilter<Target=S>
        {
            type Inner = InnerDerivedStore<S, F>;

            fn from_ref(arc: Arc<SlockCell<Self::Inner>>) -> Self {
                DerivedStore {
                    inner: arc
                }
            }

            fn inner_ref(&self) -> &Arc<SlockCell<Self::Inner>> {
                &self.inner
            }

            fn arc_clone(&self) -> Self {
                DerivedStore {
                    inner: Arc::clone(&self.inner)
                }
            }
        }
    }
    pub use derived_store::*;

    mod general_binding {
        use std::marker::PhantomData;
        use std::ops::Deref;
        use std::sync::{Arc, Weak};
        use crate::core::{Slock};
        use crate::state::{StateFilter, StateListener, WeakBinding};
        use crate::state::store::state_ref::StateRef;
        use crate::state::{Signal};
        use crate::state::signal::GeneralSignal;
        use crate::state::slock_cell::SlockCell;
        use crate::state::store::raw_store::RawStore;
        use crate::state::store::raw_store_shared_owner::RawStoreSharedOwner;
        use crate::util::marker::ThreadMarker;

        // will find better solution in the future
        // This really shouldn't even have to be public in first place
        // it's just because of rust typing issues
        #[allow(private_bounds)]
        pub struct GeneralWeakBinding<F, I> where F: StateFilter, I: RawStoreSharedOwner<F> {
            pub(super) weak: Weak<SlockCell<I::Inner>>,
            pub(super) phantom: PhantomData<(Weak<SlockCell<F>>, I)>
        }

        impl<F, I> Clone for GeneralWeakBinding<F, I> where F: StateFilter, I: RawStoreSharedOwner<F> {
            fn clone(&self) -> Self {
                GeneralWeakBinding {
                    weak: self.weak.clone(),
                    phantom: Default::default(),
                }
            }
        }

        #[allow(private_bounds)]
        pub struct GeneralBinding<F, I> where F: StateFilter, I: RawStoreSharedOwner<F> {
            pub(super) inner: I,
            pub(super) phantom: PhantomData<Arc<SlockCell<F>>>,
        }

        impl<F, I> WeakBinding<F> for GeneralWeakBinding<F, I> where F: StateFilter, I: RawStoreSharedOwner<F> {
            type Binding = GeneralBinding<F, I>;

            fn upgrade(&self) -> Option<Self::Binding> {
                let Some(inner) = self.weak.upgrade() else {
                    return None;
                };

                I::Inner::strong_count_increment(&inner);

                Some(GeneralBinding {
                    inner: I::from_ref(inner),
                    phantom: Default::default(),
                })
            }
        }

        impl<F, I> Clone for GeneralBinding<F, I> where F: StateFilter, I: RawStoreSharedOwner<F> {
            fn clone(&self) -> Self {
                I::Inner::strong_count_increment(self.inner.inner_ref());

                GeneralBinding {
                    inner: self.inner.arc_clone(),
                    phantom: PhantomData
                }
            }
        }

        impl<F, I> Signal for GeneralBinding<F, I> where F: StateFilter, I: RawStoreSharedOwner<F> {
            type Target=F::Target;

            fn borrow<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=F::Target> + 'a {
                StateRef {
                    main_ref: self.inner_ref().borrow(s),
                    phantom: PhantomData
                }
            }

            fn listen<Q>(&self, listener: Q, s: Slock<impl ThreadMarker>)
                where Q: FnMut(&F::Target, Slock) -> bool + Send + 'static {
                self.inner_ref().borrow_mut(s).dispatcher_mut().add_listener(StateListener::SignalListener(Box::new(listener)));
            }

            type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
            fn map<U, Q>(&self, map: Q, s: Slock<impl ThreadMarker>) -> Self::MappedOutput<U>
                where U: Send + 'static, Q: Send + 'static + Fn(&F::Target) -> U {
                GeneralSignal::from(self, self, map, |this, listener, s| {
                    this.inner_ref().borrow_mut(s).dispatcher_mut().add_listener(StateListener::SignalListener(listener))
                }, s)
            }
        }

        impl<F, I> RawStoreSharedOwner<F> for GeneralBinding<F, I> where F: StateFilter, I: RawStoreSharedOwner<F> {
            type Inner = I::Inner;

            fn from_ref(arc: Arc<SlockCell<Self::Inner>>) -> Self {
                GeneralBinding {
                    inner: I::from_ref(arc),
                    phantom: Default::default(),
                }
            }

            fn inner_ref(&self) -> &Arc<SlockCell<Self::Inner>> {
                self.inner.inner_ref()
            }

            fn arc_clone(&self) -> Self {
                GeneralBinding {
                    inner: self.inner.arc_clone(),
                    phantom: PhantomData
                }
            }
        }

        #[allow(private_bounds)]
        impl<F, I> GeneralBinding<F, I> where F: StateFilter, I: RawStoreSharedOwner<F> {
            pub fn store(&self) -> &I {
                &self.inner
            }
        }

        impl<F, I> Drop for GeneralBinding<F, I> where F: StateFilter, I: RawStoreSharedOwner<F> {
            fn drop(&mut self) {
                I::Inner::strong_count_decrement(self.inner.inner_ref());
            }
        }
    }
    pub use general_binding::*;

    // only to make rust happy
    pub mod unreachable_binding {
        use std::marker::PhantomData;
        use std::ops::Deref;
        use std::sync::Arc;
        use crate::core::Slock;
        use crate::state::{GeneralSignal, IntoAction, Signal, StateFilter, Stateful};
        use crate::state::slock_cell::{MainSlockCell, SlockCell};
        use crate::state::store::inverse_listener_holder::InverseListenerHolderImpl;
        use crate::state::store::raw_store::RawStore;
        use crate::state::store::raw_store_shared_owner::RawStoreSharedOwner;
        use crate::state::store::store_dispatcher::StoreDispatcher;
        use crate::util::marker::ThreadMarker;

        pub struct UnreachableBindingInner<F>(pub PhantomData<Arc<MainSlockCell<F>>>) where F: StateFilter;

        impl<F> Deref for UnreachableBindingInner<F> where F: StateFilter {
            type Target = F::Target;

            fn deref(&self) -> &Self::Target {
                unreachable!()
            }
        }

        impl<F> RawStore<F> for UnreachableBindingInner<F> where F: StateFilter {
            type InverseListenerHolder = InverseListenerHolderImpl;

            fn dispatcher(&self) -> &StoreDispatcher<F::Target, F, Self::InverseListenerHolder> {
                unreachable!()
            }

            fn dispatcher_mut(&mut self) -> &mut StoreDispatcher<F::Target, F, Self::InverseListenerHolder> {
                unreachable!()
            }

            fn apply(_inner: &Arc<SlockCell<Self>>, _action: impl IntoAction<<F::Target as Stateful>::Action, F::Target>, _is_inverse: bool, _s: Slock<impl ThreadMarker>) {
                unreachable!()
            }
        }

        pub struct UnreachableBinding<F>(pub PhantomData<Arc<MainSlockCell<F>>>) where F: StateFilter;
        impl<F> Signal for UnreachableBinding<F> where F: StateFilter {
            type Target = F::Target;

            fn borrow<'a>(&'a self, _s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=Self::Target> + 'a {
                UnreachableBindingInner::<F>(PhantomData)
            }

            fn listen<G>(&self, _listener: G, _s: Slock<impl ThreadMarker>) where G: FnMut(&Self::Target, Slock) -> bool + Send + 'static {
                unreachable!()
            }

            type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
            fn map<S, G>(&self, _map: G, _s: Slock<impl ThreadMarker>) -> Self::MappedOutput<S> where S: Send + 'static, G: Send + 'static + Fn(&Self::Target) -> S {
                unreachable!()
            }
        }

        impl<F> RawStoreSharedOwner<F> for UnreachableBinding<F> where F: StateFilter {
            type Inner = UnreachableBindingInner<F>;

            fn from_ref(_arc: Arc<SlockCell<Self::Inner>>) -> Self {
                unreachable!()
            }

            fn inner_ref(&self) -> &Arc<SlockCell<Self::Inner>> {
                unreachable!()
            }

            fn arc_clone(&self) -> Self {
                unreachable!()
            }
        }

        impl<F> Clone for UnreachableBinding<F> where F: StateFilter {
            fn clone(&self) -> Self {
                UnreachableBinding(PhantomData)
            }
        }
    }
}
pub use store::*;

mod buffer {
    use std::ops::{Deref, DerefMut};
    use std::sync::{Arc, Weak};
    use crate::core::Slock;
    use crate::state::slock_cell::SlockCell;
    use crate::util::marker::ThreadMarker;
    use crate::util::test_util::QuarveAllocTag;

    pub struct Buffer<T>(Arc<(SlockCell<T>, QuarveAllocTag)>) where T: Send;

    #[derive(Clone)]
    pub struct WeakBuffer<T>(Weak<(SlockCell<T>, QuarveAllocTag)>) where T: Send;

    impl<T> Buffer<T>
        where T: Send
    {
        pub fn new(initial: T) -> Buffer<T> {
            Buffer(Arc::new((SlockCell::new(initial), QuarveAllocTag::new())))
        }

        pub fn borrow<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=T> + 'a {
            self.0.0.borrow(s)
        }

        pub fn borrow_mut<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl DerefMut<Target=T> + 'a  {
            self.0.0.borrow_mut(s)
        }

        pub fn replace<'a>(&'a self, with: T, s: Slock<'a, impl ThreadMarker>) -> T {
            std::mem::replace(self.borrow_mut(s).deref_mut(), with)
        }

        pub fn downgrade(&self) -> WeakBuffer<T> {
            WeakBuffer(Arc::downgrade(&self.0))
        }
    }

    impl<T> Buffer<T> where T: Send + Default {
        pub fn take(&self, s: Slock<impl ThreadMarker>) -> T {
            std::mem::take(self.borrow_mut(s).deref_mut())
        }
    }

    impl<T> WeakBuffer<T> where T: Send {
        pub fn upgrade(&self) -> Option<Buffer<T>> {
            self.0.upgrade().map(|arc| Buffer(arc))
        }
    }
}
pub use buffer::*;

mod signal {
    use std::ops::{Deref};
    use crate::core::{Slock};
    use crate::util::marker::ThreadMarker;

    pub trait Signal: Sized + Send + Sync + 'static {
        type Target: Send + 'static;

        /// Be careful about calling this method within an
        /// action_listener or related fields. While not bad by itself
        /// This can easily cause retain cycles
        /// Instead, similar logic can usually be done by using JoinedSignals,
        /// DerivedStores, or Buffers
        fn borrow<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=Self::Target> + 'a;

        fn listen<F>(&self, listener: F, s: Slock<impl ThreadMarker>)
            where F: (FnMut(&Self::Target, Slock) -> bool) + Send + 'static;

        type MappedOutput<S: Send + 'static>: Signal<Target=S> + Clone;
        fn map<S, F>(&self, map: F, s: Slock<impl ThreadMarker>) -> Self::MappedOutput<S>
            where S: Send + 'static,
                  F: Send + 'static + Fn(&Self::Target) -> S;
    }

    pub trait ActualDiffSignal : Signal where Self::Target: Send + 'static + Clone + PartialEq {
        fn diff_listen<F>(&self, mut listener: F, s: Slock<impl ThreadMarker>)
            where F: (FnMut(&Self::Target, Slock) -> bool) + Send + 'static
        {
            let mut last_val = self.borrow(s).clone();
            self.listen(move |val, s| {
                if *val != last_val {
                    last_val = val.clone();
                    listener(val, s)
                }
                else {
                    true
                }
            }, s)
        }
    }
    impl<S> ActualDiffSignal for S where S::Target: Send + Clone + Eq + 'static, S: Signal { }

    trait InnerSignal {
        type Target: Send + 'static;
        fn borrow(&self) -> &Self::Target;
    }

    mod signal_audience {
        use crate::core::{Slock};
        use crate::util::marker::ThreadMarker;

        pub(super) struct SignalAudience<T> where T: Send + 'static {
            listeners: Vec<Box<dyn FnMut(&T, Slock) -> bool + Send>>
        }

        impl<T> SignalAudience<T> where T: Send + 'static {
            pub(super) fn new() -> SignalAudience<T> {
                SignalAudience {
                    listeners: Vec::new()
                }
            }

            pub(super) fn listen<F>(&mut self, listener: F, _s: Slock<impl ThreadMarker>)
                where F: (FnMut(&T, Slock) -> bool) + Send + 'static {
                self.listeners.push(Box::new(listener));
            }

            pub(super) fn listen_box(
                &mut self,
                listener: Box<dyn (FnMut(&T, Slock) -> bool) + Send + 'static>,
                _s: Slock<impl ThreadMarker>
            ) {
                self.listeners.push(listener);
            }

            pub(super) fn dispatch(&mut self, new_val: &T, s: Slock<impl ThreadMarker>) {
                self.listeners
                    .retain_mut(|listener| listener(new_val, s.to_general_slock()))
            }

            pub(super) fn is_empty(&self) -> bool {
                self.listeners.is_empty()
            }
        }
    }
    use signal_audience::*;

    mod signal_ref {
        use std::cell::Ref;
        use std::ops::Deref;
        use super::InnerSignal;

        pub(super) struct SignalRef<'a, U: InnerSignal> {
            pub(super) src: Ref<'a, U>,
        }

        impl<'a, U> Deref for SignalRef<'a, U> where U: InnerSignal {
            type Target = U::Target;

            fn deref(&self) -> &U::Target {
                self.src.borrow()
            }
        }
    }
    use signal_ref::*;

    mod fixed_signal {
        use std::ops::Deref;
        use std::sync::Arc;
        use crate::core::{Slock};
        use crate::state::Signal;
        use crate::state::slock_cell::SlockCell;
        use crate::util::marker::ThreadMarker;
        use crate::util::test_util::QuarveAllocTag;
        use super::SignalRef;
        use super::InnerSignal;

        struct InnerFixedSignal<T: Send + 'static>(QuarveAllocTag, T);

        impl<T> InnerSignal for InnerFixedSignal<T> where T: Send + 'static {
            type Target = T;

            fn borrow(&self) -> &Self::Target {
                &self.1
            }
        }

        pub struct FixedSignal<T> where T: Send + 'static {
            inner: Arc<SlockCell<InnerFixedSignal<T>>>
        }

        impl<T> FixedSignal<T> where T: Send + 'static {
            pub fn new(val: T) -> FixedSignal<T> {
                FixedSignal {
                    inner: Arc::new(SlockCell::new(InnerFixedSignal(QuarveAllocTag::new(), val)))
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

        impl<T> Signal for FixedSignal<T> where T: Send + 'static {
            type Target = T;

            fn borrow<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=T> + 'a {
                SignalRef {
                    src: self.inner.borrow(s),
                }
            }

            fn listen<F>(&self, _listener: F, _s: Slock<impl ThreadMarker>)
                where F: FnMut(&T, Slock) -> bool + Send {
                /* no op */
            }

            type MappedOutput<S: Send + 'static> = FixedSignal<S>;
            fn map<S, F>(&self, map: F, s: Slock<impl ThreadMarker>) -> FixedSignal<S>
                where S: Send + 'static,
                      F: Send + 'static + Fn(&T) -> S
            {
                let inner = self.inner.borrow(s);
                let data = map(&inner.1);

                FixedSignal {
                    inner: Arc::new(SlockCell::new(InnerFixedSignal(QuarveAllocTag::new(), data)))
                }
            }
        }
    }
    pub use fixed_signal::*;

    mod general_signal {
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::core::{Slock};
        use crate::state::Signal;
        use crate::state::slock_cell::SlockCell;
        use crate::util::marker::ThreadMarker;
        use crate::util::test_util::QuarveAllocTag;
        use super::SignalRef;
        use super::{InnerSignal, SignalAudience};

        struct GeneralInnerSignal<T> where T: Send + 'static {
            _quarve_tag: QuarveAllocTag,
            val: T,
            audience: SignalAudience<T>
        }

        impl<T> InnerSignal for GeneralInnerSignal<T> where T: Send + 'static {
            type Target = T;

            fn borrow(&self) -> &T {
                &self.val
            }
        }

        pub struct GeneralSignal<T> where T: Send + 'static {
            inner: Arc<SlockCell<GeneralInnerSignal<T>>>
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
            pub(crate) fn from<H, S, F, G>(dispatcher: &H, signal: &S, map: F, add_listener: G, s: Slock<impl ThreadMarker>)
                                           -> GeneralSignal<T>
                where S: Signal,
                      F: Send + 'static + Fn(&S::Target) -> T,
                      G: FnOnce(&H, Box<dyn FnMut(&S::Target, Slock) -> bool + Send>, Slock)
            {
                let inner;
                {
                    let val = signal.borrow(s);
                    inner = GeneralInnerSignal {
                        _quarve_tag: QuarveAllocTag::new(),
                        val: map(&*val),
                        audience: SignalAudience::new(),
                    };
                }

                let arc = Arc::new(SlockCell::new(inner));
                let pseudo_weak = arc.clone();
                add_listener(dispatcher, Box::new(move |val, s| {
                    let mut binding = pseudo_weak.borrow_mut(s);
                    let inner = binding.deref_mut();

                    // no longer any point
                    inner.val = map(val);
                    inner.audience.dispatch(&inner.val, s);

                    // races don't matter too much since it'll just mean late drop
                    // but nothing unsound
                    !inner.audience.is_empty() || Arc::strong_count(&pseudo_weak) > 1
                }), s.to_general_slock());

                GeneralSignal {
                    inner: arc
                }
            }
        }

        impl<T> Signal for GeneralSignal<T> where T: Send + 'static {
            type Target = T;

            fn borrow<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=T> + 'a {
                SignalRef {
                    src: self.inner.borrow(s),
                }
            }

            fn listen<F>(&self, listener: F, s: Slock<impl ThreadMarker>)
                where F: FnMut(&T, Slock) -> bool + Send + 'static {
                self.inner.borrow_mut(s).audience.listen(listener, s);
            }

            type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
            fn map<S, F>(&self, map: F, s: Slock<impl ThreadMarker>) -> GeneralSignal<S>
                where S: Send + 'static, F: Fn(&T) -> S + Send + 'static {
                GeneralSignal::from(self, self, map, |this, listener, s| {
                    this.inner.borrow_mut(s).audience.listen_box(listener, s);
                }, s)
            }
        }
    }
    pub use general_signal::*;

    mod joined_signal {
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU8};
        use std::sync::atomic::Ordering::{SeqCst};
        use crate::core::{Slock};
        use crate::state::{GeneralSignal, Signal};
        use crate::state::signal::InnerSignal;
        use crate::state::signal::signal_audience::SignalAudience;
        use crate::state::signal::signal_ref::SignalRef;
        use crate::state::slock_cell::SlockCell;
        use crate::util::marker::ThreadMarker;
        use crate::util::test_util::QuarveAllocTag;

        struct JoinedInnerSignal<T, U, V>
            where T: Send + 'static,
                  U: Send + 'static,
                  V: Send + 'static
        {
            _quarve_tag: QuarveAllocTag,
            t: T,
            u: U,
            ours: V,
            audience: SignalAudience<V>,
        }

        impl<T, U, V> InnerSignal for JoinedInnerSignal<T, U, V>
            where T: Send + 'static,
                  U: Send + 'static,
                  V: Send + 'static
        {
            type Target = V;

            fn borrow(&self) -> &V {
                &self.ours
            }
        }

        pub struct JoinedSignal<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {
            // first field is the number of parents (i.e. lhs, rhs) owning it
            inner: Arc<(AtomicU8, SlockCell<JoinedInnerSignal<T, U, V>>)>
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

        struct ParentOwner<T, U, V>(Arc<(AtomicU8, SlockCell<JoinedInnerSignal<T, U, V>>)>)
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static;

        impl<T, U, V> Drop for ParentOwner<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {
            fn drop(&mut self) {
                // it's important that this is subtracted at a time
                // strictly before the ARC strong counter
                // so that we do not falsely free early
                self.0.0.fetch_sub(1, SeqCst);
            }
        }


        impl<T, U> JoinedSignal<T, U, (T, U)>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
        {
            pub fn join(lhs: &impl Signal<Target=T>, rhs: &impl Signal<Target=U>, s: Slock<impl ThreadMarker>) -> Self
            {
                JoinedSignal::join_map(lhs, rhs, |t, u| (t.clone(), u.clone()), s)
            }
        }

        impl<T, U, V> JoinedSignal<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {

            pub fn join_map<F>(lhs: &impl Signal<Target=T>, rhs: &impl Signal<Target=U>, map: F, s: Slock<impl ThreadMarker>)
                               -> JoinedSignal<T, U, V>
                where F: Send + Clone + 'static + Fn(&T, &U) -> V
            {
                let inner = {
                    let l = lhs.borrow(s);
                    let r = rhs.borrow(s);

                    JoinedInnerSignal {
                        _quarve_tag: QuarveAllocTag::new(),
                        t: l.clone(),
                        u: r.clone(),
                        ours: map(&*l, &*r),
                        audience: SignalAudience::new(),
                    }
                };

                // initially, there are two parents owning this
                let arc = Arc::new((AtomicU8::new(2), SlockCell::new(inner)));

                let pseudo_weak = ParentOwner(arc.clone());
                let lhs_map = map.clone();
                lhs.listen(move |lhs, s| {
                    let ParentOwner(pseudo_weak) = &pseudo_weak;

                    let mut binding = pseudo_weak.1.borrow_mut(s);
                    let inner = binding.deref_mut();
                    inner.t = lhs.clone();
                    inner.ours = lhs_map(&inner.t, &inner.u);
                    inner.audience.dispatch(&inner.ours, s);

                    // certainly this can change, but we do not particular care
                    // since this is just an early exit, not necessarily the final

                    !inner.audience.is_empty() ||
                        Arc::strong_count(&pseudo_weak) > pseudo_weak.0.load(SeqCst) as usize
                }, s);

                let pseudo_weak = ParentOwner(arc.clone());
                rhs.listen(move |rhs, s| {
                    let ParentOwner(pseudo_weak) = &pseudo_weak;

                    let mut binding = pseudo_weak.1.borrow_mut(s);
                    let inner = binding.deref_mut();
                    inner.u = rhs.clone();
                    inner.ours = map(&inner.t, &inner.u);
                    inner.audience.dispatch(&inner.ours, s);

                    !inner.audience.is_empty() ||
                        Arc::strong_count(&pseudo_weak) > pseudo_weak.0.load(SeqCst) as usize
                }, s);

                JoinedSignal {
                    inner: arc
                }
            }
        }

        impl<T, U, V> Signal for JoinedSignal<T, U, V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static
        {
            type Target = V;

            fn borrow<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=V> + 'a {
                SignalRef {
                    src: self.inner.1.borrow(s),
                }
            }

            fn listen<F>(&self, listener: F, s: Slock<impl ThreadMarker>)
                where F: FnMut(&V, Slock) -> bool + Send + 'static {
                self.inner.1.borrow_mut(s).audience.listen(listener, s);
            }

            type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
            fn map<S, F>(&self, map: F, s: Slock<impl ThreadMarker>) -> GeneralSignal<S>
                where S: Send + 'static,
                      F: Send + 'static + Fn(&V) -> S
            {
                GeneralSignal::from(self, self, map, |this, listener, s| {
                    this.inner.1.borrow_mut(s).audience.listen_box(listener, s);
                }, s)
            }
        }
    }
    pub use joined_signal::*;

    mod timed_signal {
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use std::sync::atomic::AtomicU8;
        use std::sync::atomic::Ordering::{SeqCst};
        use std::time::Duration;
        use crate::core::{Slock, timed_worker};
        use crate::state::signal::InnerSignal;
        use crate::state::{GeneralSignal, Signal};
        use crate::state::capacitor::{Capacitor};
        use crate::state::signal::signal_audience::SignalAudience;
        use crate::state::signal::signal_ref::SignalRef;
        use crate::state::slock_cell::SlockCell;
        use crate::util::marker::ThreadMarker;
        use crate::util::test_util::QuarveAllocTag;

        pub trait WithCapacitor {
            type Target: Send;
            fn with_capacitor<C>(&self, capacitor: C, s: Slock<impl ThreadMarker>)
                                 -> CapacitatedSignal<C> where C: Capacitor<Target=Self::Target>;
        }

        impl<S> WithCapacitor for S where S: Signal {
            type Target = S::Target;

            fn with_capacitor<C>(&self, capacitor: C, s: Slock<impl ThreadMarker>)
                -> CapacitatedSignal<C> where C: Capacitor<Target=S::Target> {
                CapacitatedSignal::from(self, capacitor, s)
            }
        }

        struct CapacitatedInnerSignal<C> where C: Capacitor {
            _quarve_tag: QuarveAllocTag,
            curr: C::Target,
            capacitor: C,
            time_active: Option<Duration>,
            audience: SignalAudience<C::Target>,
        }

        impl<C> CapacitatedInnerSignal<C> where C: Capacitor {
            fn set_curr(&mut self, to: C::Target, s: Slock) {
                self.curr = to;
                self.audience.dispatch(&self.curr, s);
            }
        }

        impl<C> InnerSignal for CapacitatedInnerSignal<C> where C: Capacitor {
            type Target = C::Target;

            fn borrow(&self) -> &C::Target {
                &self.curr
            }
        }

        pub struct CapacitatedSignal<C> where C: Capacitor {
            // first field is parent retain count
            // i.e. worker thread or source signal
            inner: Arc<(AtomicU8, SlockCell<CapacitatedInnerSignal<C>>)>
        }

        impl<C> Clone for CapacitatedSignal<C> where C: Capacitor {
            fn clone(&self) -> Self {
                CapacitatedSignal {
                    inner: self.inner.clone()
                }
            }
        }

        struct ParentOwner<C>(Arc<(AtomicU8, SlockCell<CapacitatedInnerSignal<C>>)>) where C: Capacitor;

        // FIXME, I think SeqCst is overkill in this scenario
        // and likewise for JoinedSignal
        impl<C> Drop for ParentOwner<C> where C: Capacitor {
            fn drop(&mut self) {
                // it's important that this is subtracted at a time
                // strictly before the ARC strong counter
                // so that we do not falsely free early
                self.0.0.fetch_sub(1, SeqCst);
            }
        }
        impl<C> CapacitatedSignal<C> where C: Capacitor {

            #[inline]
            fn update_active(this: &Arc<(AtomicU8, SlockCell<CapacitatedInnerSignal<C>>)>, mut_ref: &mut CapacitatedInnerSignal<C>, _s: Slock) {
                if mut_ref.time_active.is_none() {
                    mut_ref.time_active = Some(Duration::from_secs(0));

                    /* spawn worker, increment parent count */
                    this.0.fetch_add(1, SeqCst);

                    let worker_arc = ParentOwner(this.clone());
                    timed_worker(move |duration, s| {
                        let ParentOwner(worker_arc) = &worker_arc;

                        let mut borrow = worker_arc.1.borrow_mut(s);
                        let mut_ref = borrow.deref_mut();

                        let (sample, cont) = mut_ref.capacitor.sample(duration);
                        mut_ref.set_curr(sample, s);

                        if !cont {
                            mut_ref.time_active = None;

                            false
                        }
                        else {
                            mut_ref.time_active = Some(duration);

                            // races don't matter too much since it'll just mean late drop
                            // but nothing unsound (since the parent_count will always be decremented first)
                            !mut_ref.audience.is_empty() ||
                                Arc::strong_count(&worker_arc) > worker_arc.0.load(SeqCst) as usize
                        }
                    })
                }
            }
        }

        impl<C> CapacitatedSignal<C> where C: Capacitor {
            pub fn from(source: &impl Signal<Target=C::Target>, mut capacitor: C, s: Slock<impl ThreadMarker>) -> Self {
                capacitor.target_set(&*source.borrow(s), None);
                let (curr, initial_thread) = capacitor.sample(Duration::from_secs(0));

                // initially, there is only one parent (the source signal)
                // hence the first field
                let arc = Arc::new((AtomicU8::new(1), SlockCell::new(CapacitatedInnerSignal {
                    _quarve_tag: QuarveAllocTag::new(),
                    curr,
                    capacitor,
                    time_active: None,
                    audience: SignalAudience::new(),
                })));

                // so we can't do just a weak signal
                // since then it may be dropped prematurely
                // the exact semantics we want is that the worker_thread (/parent signal) owns us
                // but if no one is listening, and no listeners in the future
                // which we can argue via retain count, only then can we cancel
                let parent_arc = ParentOwner(arc.clone());
                source.listen(move |curr, s| {
                    let ParentOwner(parent_arc) = &parent_arc;

                    let mut borrow = parent_arc.1.borrow_mut(s);
                    let mut_ref = borrow.deref_mut();
                    mut_ref.capacitor.target_set(curr, mut_ref.time_active);
                    CapacitatedSignal::update_active(&parent_arc, mut_ref, s);

                    // races don't matter too much since it'll just mean late drop
                    // but nothing unsounds
                    !mut_ref.audience.is_empty() ||
                        Arc::strong_count(&parent_arc) > parent_arc.0.load(SeqCst) as usize
                }, s);

                // start thread if necessary
                if initial_thread {
                    CapacitatedSignal::update_active(&arc, arc.1.borrow_mut(s).deref_mut(), s.to_general_slock());
                }

                CapacitatedSignal {
                    inner: arc
                }
            }
        }

        impl<C> Signal for CapacitatedSignal<C> where C: Capacitor {
            type Target = C::Target;

            fn borrow<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=C::Target> + 'a {
                SignalRef {
                    src: self.inner.1.borrow(s),
                }
            }

            fn listen<F>(&self, listener: F, s: Slock<impl ThreadMarker>)
                where F: FnMut(&C::Target, Slock) -> bool + Send + 'static {
                self.inner.1.borrow_mut(s).audience.listen(listener, s);
            }

            type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
            fn map<S, F>(&self, map: F, s: Slock<impl ThreadMarker>) -> GeneralSignal<S>
                where S: Send + 'static,
                      F: Send + 'static + Fn(&C::Target) -> S
            {
                GeneralSignal::from(self, self, map, |this, listener, s| {
                    this.inner.1.borrow_mut(s).audience.listen_box(listener, s);
                }, s)
            }
        }
    }
    pub use timed_signal::*;

    mod signal_or_value {
        use crate::core::{Environment, Slock};
        use crate::state::{FixedSignal, Signal};
        use crate::util::marker::ThreadMarker;
        use crate::view::WeakInvalidator;

        pub enum SignalOrValue<S> where S: Signal {
            Value(S::Target),
            Signal(S)
        }

        impl<T> SignalOrValue<FixedSignal<T>> where T: Send + 'static {
            pub fn value(t: T) -> Self {
                SignalOrValue::Value(t)
            }
        }

        impl<S> SignalOrValue<S> where S: Signal {
            pub fn add_invalidator<E: Environment>(&self, inv: &WeakInvalidator<E>, s: Slock<impl ThreadMarker>) {
                if let SignalOrValue::Signal(sig) = self  {
                    let weak_inv = inv.clone();
                    sig.listen(move |_, s| {
                        let Some(inv) = weak_inv.upgrade() else {
                            return false;
                        };

                        inv.invalidate(s);
                        true
                    }, s)
                }
            }
        }
        impl<S> SignalOrValue<S> where S::Target: Copy, S: Signal {
            pub fn inner(&self, s: Slock<impl ThreadMarker>) -> S::Target {
                match self {
                    SignalOrValue::Signal(sig) => *sig.borrow(s),
                    SignalOrValue::Value(val) => *val
                }
            }
        }
    }
    pub use signal_or_value::*;
}
pub use signal::*;

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;
    use rand::Rng;
    use crate::core::{clock_signal, setup_timing_thread, slock_owner, timed_worker};
    use crate::state::{Store, Signal, TokenStore, Binding, StoreContainer, NumericAction, DirectlyInvertible, Filterable, DerivedStore, StringActionBasis, Buffer, Word, GroupAction, WithCapacitor, JoinedSignal, FixedSignal, EditingString, Bindable};
    use crate::state::capacitor::{ConstantSpeedCapacitor, ConstantTimeCapacitor, SmoothCapacitor};
    use crate::state::SetAction::{Identity, Set};
    use crate::state::VecActionBasis::{Insert, Remove, Swap};
    use crate::util::numeric::Norm;
    use crate::util::test_util::HeapChecker;
    use crate::util::Vector;

    #[test]
    fn test_numeric() {
        let _h = HeapChecker::new();
        let c = slock_owner();

        let s: Store<i32> = Store::new(2);
        let derived_sig;
        let derived_derived;
        {
            derived_sig = s.map(|x| x * x, c.marker());
            let s = c.marker();
            let b = derived_sig.borrow(s);
            assert_eq!(*b, 4);
        }
        {
            derived_derived = derived_sig.map(|x| x - 4, c.marker());
        }

        s.apply(Set(6), c.marker());
        {
            let s = c.marker();
            let b = derived_sig.borrow(s);
            assert_eq!(*b, 36);
            let b = derived_derived.borrow(s);
            assert_eq!(*b, 32);
        }

        s.apply(Identity * Set(1), c.marker());
        {
            let b = derived_sig.borrow(c.marker());
            assert_eq!(*b, 1);
            let b = derived_derived.borrow(c.marker());
            assert_eq!(*b, -3);
        }

        let sig1;
        {
            let sig = FixedSignal::new(-1);

            sig1 = sig.map(|x| 2 * x, c.marker());
        }

        let b = sig1.borrow(c.marker());
        let c = *b;
        assert_eq!(c, -2);
    }


    #[test]
    fn test_join() {
        let _h = HeapChecker::new();
        let s = slock_owner();

        let x: Store<i32> = Store::new(3);
        let y: Store<bool> = Store::new(false);

        let join = JoinedSignal::join(&x, &y, s.marker());
        assert_eq!(*join.borrow(s.marker()), (3, false));

        x.apply(Set(4), s.marker());
        assert_eq!(*join.borrow(s.marker()), (4, false));

        y.apply(Set(true), s.marker());
        assert_eq!(*join.borrow(s.marker()), (4, true));

        x.apply(Set(-1), s.marker());
        y.apply(Set(false), s.marker());
        assert_eq!(*join.borrow(s.marker()), (-1, false));
    }

    #[test]
    fn test_join_map() {
        let _h = HeapChecker::new();
        let s = slock_owner();

        let x: Store<i32> = Store::new(3);
        let y: Store<bool> = Store::new(false);

        let join = JoinedSignal::join_map(&x.signal(), &y.signal(), |x, y|
            if *y {
                x + 4
            }
            else {
                x * x
            }, s.marker()
        );
        assert_eq!(*join.borrow(s.marker()), 9);

        x.apply(Set(4), s.marker());
        assert_eq!(*join.borrow(s.marker()), 16);

        y.apply(Set(true), s.marker());
        assert_eq!(*join.borrow(s.marker()), 8);

        x.apply(Set(-1), s.marker());
        y.apply(Set(false), s.marker());
        assert_eq!(*join.borrow(s.marker()), 1);

        drop(x);
        y.apply(Set(true), s.marker());
        assert_eq!(*join.borrow(s.marker()), 3);
    }

    #[test]
    fn test_token_store() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let token: TokenStore<i32> = TokenStore::new(1);
        // let token = Store::new(1);

        let mut listeners = Vec::new();
        // a bit hacky since this testing scenario is rather awkward
        let counts: [DerivedStore<usize>; 10] = Default::default();
        for i in 0usize..10usize {
            let equals = token.equals(i as i32, s.marker());
            let c = counts[i].binding();
            equals.listen(move |_, s| {
                let curr = *c.borrow(s);

                c.apply(Set(curr + 1), s);
                true
            }, s.marker());
            listeners.push(equals);
        }
        assert_eq!(*counts[1].borrow(s.marker()), 0);
        token.apply(Set(1), s.marker());
        assert_eq!(*counts[1].borrow(s.marker()), 0);
        token.apply(Set(2), s.marker());
        assert_eq!(*counts[1].borrow(s.marker()), 1);
        assert_eq!(*counts[2].borrow(s.marker()), 1);
        token.apply(Set(4), s.marker());
        assert_eq!(*counts[1].borrow(s.marker()), 1);
        assert_eq!(*counts[2].borrow(s.marker()), 2);
        assert_eq!(*counts[4].borrow(s.marker()), 1);
        token.apply(Set(1), s.marker());
        assert_eq!(*counts[1].borrow(s.marker()), 2);
        assert_eq!(*counts[2].borrow(s.marker()), 2);
        assert_eq!(*counts[4].borrow(s.marker()), 2);
    }

    #[test]
    fn test_action_listener() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let state = Store::new(0);
        // these are technically not "true" derived stores
        // but the restrictions are somewhat loose
        // we are just using them for testing purposes
        // it may happen that in the future, we will have to ArcMutex
        // instead of this hack
        let identity_counter = Buffer::new(0);
        let set_counter = Buffer::new(0);
        let scb = set_counter.downgrade();
        let icb = identity_counter.downgrade();
        state.action_listener( move |_, action, s| {
            let Some(icb) = icb.upgrade() else {
                return false;
            };

            let Identity = action else {
                return true
            };
            let mut old = icb.borrow_mut(s);
            if *old == 5 {
                // stop caring about events
                return false
            }
            *old += 1;
            true
        }, s.marker());
        state.action_listener( move |_, action, s| {
            let Some(scb) = scb.upgrade() else {
                return false;
            };
            let Set(_) = action else {
                return true
            };
            *scb.borrow_mut(s) += 1;
            true
        }, s.marker());
        for i in 0 .. 100 {
            assert_eq!(*set_counter.borrow(s.marker()), i);
            assert_eq!(*identity_counter.borrow(s.marker()), std::cmp::min(i, 5));
            state.apply(Identity, s.marker());
            state.apply(NumericAction::Incr(1), s.marker());
        }
    }

    #[test]
    fn test_action_filter() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let state = Store::new_with_filter(0);
        state.action_filter(|curr, action, _s| {
            match action {
                Identity => Set(*curr + 1),
                Set(_) => Identity
            }
        }, s.marker());
        state.apply(Set(1), s.marker());
        assert_eq!(*state.borrow(s.marker()), 0);
        state.apply(Identity, s.marker());
        state.apply(Identity, s.marker());
        assert_eq!(*state.borrow(s.marker()), 2);
    }

    #[test]
    fn test_inverse_listener() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let state = Store::new(0);
        let vec: Vec<Box<dyn DirectlyInvertible>> = Vec::new();
        let vectors = Arc::new(Mutex::new(Some(vec)));
        let c = vectors.clone();
        state.subtree_inverse_listener(move |inv, _s| {
            let mut l1 = c.lock().unwrap();
            let Some(l) = l1.as_mut() else {
                return false;
            };
            l.push(inv);
            true
        }, s.marker());
        for i in 0.. 100 {
            state.apply(Set(i * i), s.marker());
        }
        let mut l = vectors.lock().unwrap();
        assert_eq!(l.as_ref().unwrap().len(), 100);
        l.as_mut().unwrap().reverse();
        let res = l.take().unwrap().into_iter().enumerate();
        drop(l);
        for (i, mut item) in res.take(90) {
            assert_eq!(*state.borrow(s.marker()), (99 - i) * (99 - i));
            item.invert(s.marker());
        }
    }

    #[test]
    fn test_inverse_listener_combine() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let state = Store::new(0);
        let vec: Option<Box<dyn DirectlyInvertible>> = None;
        let vectors = Arc::new(Mutex::new(Some(vec)));
        let c = vectors.clone();
        state.subtree_inverse_listener(move |inv, s| {
            let mut l1 = c.lock().unwrap();
            let Some(l) = l1.as_mut() else {
                return false;
            };
            if l.is_none() {
                *l = Some(inv);
            }
            else {
                unsafe {
                    l.as_mut().unwrap().right_multiply(inv, s);
                }
            }
            true
        }, s.marker());
        for i in 0.. 100 {
            state.apply(Set(i * i), s.marker());
        }
        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        res.invert(s.marker());
        assert_eq!(*state.borrow(s.marker()), 0);
    }

    #[test]
    fn test_general_listener() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let state = Store::new(0);
        let set_counter = Buffer::new(0);
        let scb = set_counter.downgrade();
        state.subtree_general_listener(move |s| {
            let Some(scb) = scb.upgrade() else {
                return false;
            };

            *scb.borrow_mut(s) += 1;
            let x = *scb.borrow(s) < 63;
            x
        }, s.marker());

        for i in 0 .. 100 {
            assert_eq!(*set_counter.borrow(s.marker()), std::cmp::min(i, 63));
            state.apply(Identity, s.marker());
        }
    }

    #[test]
    fn test_properly_marked_derived_no_panic() {
        let s = slock_owner();
        let state = Store::new(0);
        let derived = DerivedStore::new(0);
        let b = derived.binding();
        state.action_listener(move |_, _, s| {
            b.apply(NumericAction::Incr(1), s);
            true
        }, s.marker());
        state.apply(Set(1), s.marker());
    }

    #[test]
    fn test_signal_no_early_freeing() {
        // even if intermediate signals are dropped
        // downstream signals remain unaffected
        let _h = HeapChecker::new();
        let s = slock_owner();
        let store = Store::new(0);
        let middle = store.map(|x| *x, s.marker());
        let bottom = middle.map(|x| *x, s.marker());
        let changes = Buffer::new(0);
        let binding = changes.downgrade();
        bottom.listen(move |_a, s| {
            let Some(binding) = binding.upgrade() else {
                return false;
            };

            *binding.borrow_mut(s) += 1;
            true
        }, s.marker());

        store.apply(Set(1), s.marker());
        drop(middle);
        store.apply(Set(-1), s.marker());
        drop(bottom);

        assert_eq!(*changes.borrow(s.marker()), 2);
    }

    #[test]
    fn test_signal_early_freeing() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let store = Store::new(0);
        {
            let _h = HeapChecker::new();
            let middle = store.map(|x| *x, s.marker());
            drop(middle);
            // this operation should clear ownership of the signal
            store.apply(Set(1), s.marker());
        }
    }

    #[test]
    #[should_panic]
    fn test_signal_no_early_freeing_without_clear() {
        let s = slock_owner();
        let store = Store::new(0);
        {
            let _h = HeapChecker::new();
            let middle = store.map(|x| *x, s.marker());
            drop(middle);
            // with no modification, signal will be owned by store
            // store.apply(Set(1), s.slock());
        }
    }

    #[test]
    fn test_join_no_early_freeing() {
        let h = HeapChecker::new();
        let s = slock_owner();

        let left = Store::new(0);
        let right = Store::new(0);
        let left_binding = left.binding();
        let middle = JoinedSignal::join(&left, &right, s.marker());
        {
            let hc2 = HeapChecker::new();
            let bottom = middle.map(|x| *x, s.marker());
            //
            drop(middle);
            drop(left);
            left_binding.apply(Set(1), s.marker());

            right.apply(Set(1), s.marker());
            drop(bottom);

            // at this point, both left and right have ownership of bottom
            hc2.assert_diff(1);

            left_binding.apply(Set(1), s.marker());
            // middle no longer sees bottom
            hc2.assert_diff(0);

            // left no longer sees middle, but right still doess
        }
        h.assert_diff(3);
        right.apply(Set(1), s.marker());
        // right no longer sees middle + middle dropped
        h.assert_diff(2);
    }

    #[test]
    fn test_string() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let actions: Arc<Mutex<Vec<Box<dyn DirectlyInvertible>>>> = Arc::new(Mutex::new(Vec::new()));
        let store = Store::new(EditingString("asdfasdf".to_string()));
        let mut strings: Vec<String> = Vec::new();
        let a = actions.clone();
        store.subtree_inverse_listener(move |invertible, _s| {
            a.lock().unwrap().push(invertible);
            true
        }, s.marker());
        for _i in 0 .. 127 {
            let curr = store.borrow(s.marker()).clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.0.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.0.len() - i);
            strings.push(curr.0);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            store.apply(StringActionBasis::ReplaceSubrange(i..u+i, str), s.marker());
        }

        let mut actions = std::mem::replace(&mut *actions.lock().unwrap(), Vec::new());
        actions.reverse();

        for (i, mut action) in actions.into_iter().enumerate() {
            action.invert(s.marker());
            assert_eq!(*store.borrow(s.marker()), EditingString(strings[strings.len() - 1 - i].clone()));
        }
    }

    #[test]
    fn test_string_compress() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let state = Store::new(EditingString("asfasdf".to_string()));
        let vec: Option<Box<dyn DirectlyInvertible>> = None;
        let vectors = Arc::new(Mutex::new(Some(vec)));
        let c = vectors.clone();
        state.subtree_inverse_listener(move |inv, s| {
            let mut l1 = c.lock().unwrap();
            let Some(l) = l1.as_mut() else {
                return false;
            };
            if l.is_none() {
                *l = Some(inv);
            }
            else {
                unsafe {
                    l.as_mut().unwrap().right_multiply(inv, s);
                }
            }
            true
        }, s.marker());

        for _i in 0 .. 100 {
            let curr = state.borrow(s.marker()).clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.0.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.0.len() - i);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            state.apply(StringActionBasis::ReplaceSubrange(i..u+i, str), s.marker());
        }
        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        res.invert(s.marker());
        assert_eq!(*state.borrow(s.marker()), EditingString("asfasdf".to_string()));
    }

    #[test]
    fn test_vec() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let actions: Arc<Mutex<Vec<Box<dyn DirectlyInvertible>>>> = Arc::new(Mutex::new(Vec::new()));
        let store: Store<Vec<Store<i32>>> = Store::new(vec![Store::new(2), Store::new(3)]);
        let mut items: Vec<Vec<i32>> = Vec::new();
        let a = Arc::downgrade(&actions);
        store.subtree_inverse_listener(move |invertible, _s| {
            let Some(a) = a.upgrade() else {
                return false;
            };
            a.lock().unwrap().push(invertible);

            true
        }, s.marker());
        for _i in 0..127 {
            let curr: Vec<_> = store.borrow(s.marker())
                .iter()
                .map(|x| *x.borrow(s.marker()))
                .collect();

            if !curr.is_empty() {
                let u = rand::thread_rng().gen_range(0..curr.len());
                let v = rand::thread_rng().gen_range(-100000..100000);
                items.push(curr);
                store.borrow(s.marker())[u]
                    .apply(Set(v), s.marker());
            }

            let curr: Vec<_> = store.borrow(s.marker())
                .iter()
                .map(|x| *x.borrow(s.marker()))
                .collect();

            let range = if curr.is_empty() {
                2..3
            } else {
                0..3
            };
            let act = match rand::thread_rng().gen_range(range) {
                0 => {
                    let u = rand::thread_rng().gen_range(0..curr.len());
                    Remove(u)
                },
                1 => {
                    let u = rand::thread_rng().gen_range(0..curr.len());
                    let v = rand::thread_rng().gen_range(0..curr.len());
                    Swap(u, v)
                },
                _ => {
                    let u = rand::thread_rng().gen_range(0..=curr.len());
                    let v = rand::thread_rng().gen_range(-100000..100000);

                    Insert(Store::new(v), u)
                },
            };
            items.push(curr);
            store.apply(act, s.marker());
        }

        let mut actions_ = std::mem::replace(&mut *actions.lock().unwrap(), Vec::new());
        actions_.reverse();

        for (i, mut action) in actions_.into_iter().enumerate() {
            action.invert(s.marker());
            assert_eq!(store.borrow(s.marker()).len(), items[items.len() - 1 - i].len());
            for j in 0..items[items.len() - 1 - i].len() {
                assert_eq!(*store.borrow(s.marker())[j].borrow(s.marker()), items[items.len() - 1 - i][j]);
            }
        }
    }

    #[test]
    fn test_vec_collapsed() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let store: Store<Vec<Store<i32>>> = Store::new(vec![Store::new(1)]);
        let vec: Option<Box<dyn DirectlyInvertible>> = None;
        let vectors = Arc::new(Mutex::new(Some(vec)));
        let c = vectors.clone();
        store.subtree_inverse_listener(move |inv, s| {
            let mut l1 = c.lock().unwrap();
            let Some(l) = l1.as_mut() else {
                return false;
            };
            if l.is_none() {
                *l = Some(inv);
            }
            else {
                unsafe {
                    l.as_mut().unwrap().right_multiply(inv, s);
                }
            }
            true
        }, s.marker());
        for _i in 0 .. 127 {
            let curr: Vec<_> = store.borrow(s.marker())
                .iter()
                .map(|x| *x.borrow(s.marker()))
                .collect();

            let range = if curr.is_empty() {
                2..3
            } else {
                0..3
            };
            let act = match rand::thread_rng().gen_range(range) {
                0 => {
                    let u = rand::thread_rng().gen_range(0..curr.len());
                    Remove(u)
                },
                1 => {
                    let u = rand::thread_rng().gen_range(0..curr.len());
                    let v = rand::thread_rng().gen_range(0..curr.len());
                    Swap(u, v)
                },
                _ => {
                    let u = rand::thread_rng().gen_range(0..= curr.len());
                    let v = rand::thread_rng().gen_range(-100000..100000);

                    Insert(Store::new(v), u)
                },
            };
            store.apply(act, s.marker());
        }

        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        res.invert(s.marker());

        assert_eq!(store.borrow(s.marker()).len(), 1);
    }

    #[test]
    fn test_subtree_general_listener() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let store = Store::new(vec![Store::new(1)]);
        let count = Arc::new(Mutex::new(0));
        let c = count.clone();
        store.subtree_general_listener(move |_s| {
            *c.lock().unwrap() += 1;
            true
        }, s.marker());
        store.apply(Insert(Store::new(2), 0), s.marker());
        store.borrow(s.marker())[0].apply(Set(1), s.marker());

        // 3 because an extra call is made to check
        // if it's still relevant
        assert_eq!(*count.lock().unwrap(), 3);
    }

    #[test]
    fn test_clock_signal() {
        setup_timing_thread();

        let _h = HeapChecker::new();
        let clock = {
            let s = slock_owner();
            clock_signal(s.marker())
        };

        sleep(Duration::from_millis(800));

        {
            let s = slock_owner();
            assert!((*clock.borrow(s.marker()) - 0.8).abs() < 0.16);
        }

        // wait for another tick to make sure clock is
        // freed from timer thread
        drop(clock);
        sleep(Duration::from_millis(100));
    }

    #[test]
    fn test_constant_time_capacitor() {
        setup_timing_thread();

        let _h = HeapChecker::new();
        let store = Store::new(0.0);
        let capacitated = {
            let s = slock_owner();
            let ret = store.with_capacitor(ConstantTimeCapacitor::new(1.0), s.marker());
            store.apply(Set(1.5), s.marker());

            ret
        };

        sleep(Duration::from_millis(100));

        {
            let s = slock_owner();
            assert!((*capacitated.borrow(s.marker()) - 0.15) < 0.05);
        }

        sleep(Duration::from_millis(1000));

        {
            let s = slock_owner();
            assert!((*capacitated.borrow(s.marker()) - 1.5) < 0.05);
            store.apply(Set(2.0), s.marker());
        }

        sleep(Duration::from_millis(400));

        {
            let s = slock_owner();
            assert!((*capacitated.borrow(s.marker()) - 1.7) < 0.05);
            store.apply(Set(10.0), s.marker());
        }

        {
            let s = slock_owner();
            assert!((*capacitated.borrow(s.marker()) - 2.0) < 0.05);
        }

        sleep(Duration::from_millis(100));

        {
            let s = slock_owner();
            assert!((*capacitated.borrow(s.marker()) - 2.8) < 0.05);
            store.apply(Set(3.0), s.marker());
        }

        sleep(Duration::from_millis(100));

        {
            let s = slock_owner();
            assert!((*capacitated.borrow(s.marker()) - 2.82) < 0.05);
        }

        sleep(Duration::from_millis(900));

        {
            let s = slock_owner();
            assert!((*capacitated.borrow(s.marker()) - 3.0) < 0.05);
        }

        sleep(Duration::from_millis(900));

        {
            let s = slock_owner();
            assert!((*capacitated.borrow(s.marker()) - 3.0) < 0.05);
        }

        // wait for another tick to make sure clock is
        // freed from timer thread
        drop(capacitated);
        sleep(Duration::from_millis(100));
    }

    #[test]
    fn test_constant_speed_capacitor() {
        setup_timing_thread();

        let _h = HeapChecker::new();
        let store = Store::new(Vector([0.0, 0.0]));
        let capacitated = {
            let s = slock_owner();
            let ret = store.with_capacitor(ConstantSpeedCapacitor::new(2.0), s.marker());

            ret
        };

        let first = thread::spawn(move || {
            let set = |u, v| {
                let s = slock_owner();
                store.apply([Set(u), Set(v)], s.marker());
            };

            set(1.0, 0.0);

            sleep(Duration::from_millis(1000));

            set(2.0, 3.0);

            sleep(Duration::from_millis(250));

            set(2.0,  1.0);
        });

        let second = thread::spawn(move || {
            let close_to = |u, v| {
                let s = slock_owner();
                let ret = (*capacitated.borrow(s.marker()) - Vector([u, v])).norm() < 0.1;
                ret
            };

            sleep(Duration::from_millis(250));

            assert!(close_to(0.5, 0.0));

            sleep(Duration::from_millis(1000));

            assert!(close_to(1.15, 0.474341649));

            sleep(Duration::from_millis(250));
            assert!(close_to(1.575, 0.737));

        });
        first.join().unwrap();
        second.join().unwrap();

        // wait for another tick to make sure clock is
        // freed from timer thread
        sleep(Duration::from_millis(1000));
    }

    #[test]
    fn test_smooth_capacitor() {
        setup_timing_thread();

        let store = Store::new(0.0);
        let c = {
            let s = slock_owner();
            store.with_capacitor(SmoothCapacitor::new(|t| {
                3.0 * t * t - 2.0 * t * t * t
            }, 1.5), s.marker())
        };
        let u = Arc::new(Mutex::new(vec![]));
        let v = Arc::new(Mutex::new(vec![]));

        let up = u.clone();
        let vp = v.clone();

        let binding = store.binding();
        let signal = c.clone();
        timed_worker(move |t, s| {
            up.lock().unwrap().push(*store.borrow(s));
            vp.lock().unwrap().push(*c.borrow(s));

            t < Duration::from_secs(5)
        });

        thread::spawn(move || {
            let set = |targ| {
                let s = slock_owner();
                binding.apply(Set(targ), s.marker());
            };
            set(10.0);
            sleep(Duration::from_millis(1000));
            set(30.0);
            sleep(Duration::from_millis(500));
            set(3.0);
            sleep(Duration::from_millis(1000));
            set(100.0);
        });

        thread::spawn(move || {
            let vals: [f64; 10] = [
                2.5165922397962865,
                7.358935059160058,
                15.27119632650832,
                17.815625429884776,
                9.614400059097994,
                29.786713432638333,
                75.9109455527237,
                99.95274972998922,
                99.95274972998922,
                99.95274972998922
            ];
            for i in 0..10 {
                sleep(Duration::from_millis(500));
                let s = slock_owner();
                // relatively high tolerance since
                // pretty steep
                assert!((*signal.borrow(s.marker()) / vals[i] - 1.0).abs() < 0.25);
            }
        }).join().unwrap();
    }

    #[test]
    fn test_vector_action() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let actions: Arc<Mutex<Vec<Box<dyn DirectlyInvertible>>>> = Arc::new(Mutex::new(Vec::new()));
        let store = Store::new(Vector([1, 2]));
        let weak = Arc::downgrade(&actions);
        store.subtree_inverse_listener(move |invertible, _s| {
            let Some(strong) = weak.upgrade() else {
                return false;
            };
            strong.lock().unwrap().push(invertible);
            true
        }, s.marker());
        store.apply([Set(2), Identity], s.marker());
        assert_eq!(*store.borrow(s.marker()).x(), 2);
        assert_eq!(*store.borrow(s.marker()).y(), 2);
        store.apply([Set(3), Set(1)], s.marker());
        assert_eq!(*store.borrow(s.marker()).x(), 3);
        assert_eq!(*store.borrow(s.marker()).y(), 1);

        let mut action = actions.lock().unwrap().pop().unwrap();
        let mut action2 = actions.lock().unwrap().pop().unwrap();

        action.invert(s.marker());
        assert_eq!(*store.borrow(s.marker()).x(), 2);
        assert_eq!(*store.borrow(s.marker()).y(), 2);

        action2.invert(s.marker());
        assert_eq!(*store.borrow(s.marker()).x(), 1);
        assert_eq!(*store.borrow(s.marker()).y(), 2);
    }

    #[test]
    fn test_vector_string() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let actions: Arc<Mutex<Vec<Box<dyn DirectlyInvertible>>>> = Arc::new(Mutex::new(Vec::new()));
        let store = Store::new(Vector([EditingString("asdfasdf".to_string())]));
        let mut strings: Vec<String> = Vec::new();
        let a = actions.clone();
        store.subtree_inverse_listener(move |invertible, _s| {
            a.lock().unwrap().push(invertible);
            true
        }, s.marker());
        for _i in 0 .. 127 {
            let curr = store.borrow(s.marker()).x().clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.0.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.0.len() - i);
            strings.push(curr.0);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            store.apply([StringActionBasis::ReplaceSubrange(i..u+i, str)], s.marker());
        }

        let mut actions = std::mem::replace(&mut *actions.lock().unwrap(), Vec::new());
        actions.reverse();

        for (i, mut action) in actions.into_iter().enumerate() {
            action.invert(s.marker());
            assert_eq!(*store.borrow(s.marker()).x(), EditingString(strings[strings.len() - 1 - i].clone()));
        }
    }

    #[test]
    fn test_vector_string_collapsed() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let state = Store::new(Vector([EditingString("asfasdf".to_string())]));
        let vec: Option<Box<dyn DirectlyInvertible>> = None;
        let vectors = Arc::new(Mutex::new(Some(vec)));
        let c = vectors.clone();
        state.subtree_inverse_listener(move |inv, s| {
            let mut l1 = c.lock().unwrap();
            let Some(l) = l1.as_mut() else {
                return false;
            };
            if l.is_none() {
                *l = Some(inv);
            }
            else {
                unsafe {
                    l.as_mut().unwrap().right_multiply(inv, s);
                }
            }
            true
        }, s.marker());

        for _i in 0 .. 100 {
            let curr = state.borrow(s.marker()).x().clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.0.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.0.len() - i);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            state.apply([StringActionBasis::ReplaceSubrange(i..u+i, str)], s.marker());
        }
        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        res.invert(s.marker());
        assert_eq!(*state.borrow(s.marker()).x(), EditingString("asfasdf".to_string()));
    }

    #[test]
    fn test_filter() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let state = Store::new_with_filter(0);
        state.action_filter(|curr, action, _s| {
            if *curr > 50 {
                Set(40)
            }
            else if let Set(target) = action {
                if target % 2 == 1 {
                    Set(target + 1)
                }
                else {
                    Set(target)
                }
            }
            else {
                Set(-1)
            }
        }, s.marker());
        let vec: Option<Box<dyn DirectlyInvertible>> = None;
        let vectors = Arc::new(Mutex::new(Some(vec)));
        let c = vectors.clone();
        state.subtree_inverse_listener(move |inv, s| {
            let mut l1 = c.lock().unwrap();
            let Some(l) = l1.as_mut() else {
                return false;
            };
            if l.is_none() {
                *l = Some(inv);
            }
            else {
                unsafe {
                    l.as_mut().unwrap().right_multiply(inv, s);
                }
            }
            true
        }, s.marker());
        for i in 0.. 100 {
            state.apply(Set(i * i), s.marker());
            assert_eq!(*state.borrow(s.marker()) % 2, 0)
        }
        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        res.invert(s.marker());
        assert_eq!(*state.borrow(s.marker()), 0);
    }

    #[test]
    fn test_buffer() {
        let _h = HeapChecker::new();
        let s = slock_owner();
        let state = Store::new(EditingString("asfasdf".to_string()));
        let buffer = Buffer::new(Word::identity());
        let buffer_writer = buffer.downgrade();
        state.action_listener(move |_, action, s| {
            let Some(buffer_writer) = buffer_writer.upgrade() else {
                return false;
            };

            buffer_writer.borrow_mut(s).left_multiply(action.clone());
            true
        }, s.marker());

        for _i in 0 .. 100 {
            let curr = state.borrow(s.marker()).clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.0.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.0.len() - i);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            state.apply(StringActionBasis::ReplaceSubrange(i..u+i, str), s.marker());
        }

        let state2 = Store::new(EditingString("asfasdf".to_string()));
        state2.apply(buffer.replace(Word::identity(), s.marker()), s.marker());
        assert_eq!(*state2.borrow(s.marker()), *state.borrow(s.marker()));
    }
}