mod listener {
    use crate::core::Slock;
    use crate::state::Stateful;

    pub(super) mod sealed {
        use crate::core::Slock;
        use crate::state::listener::DirectlyInvertible;

        // I don't like this
        // Maybe it can be done with dyn Any in a better
        // fashion?
        pub(in crate::state) trait DirectlyInvertibleBase {
            // This function must only be called once per instance
            // (We cannot take ownership since the caller is often unsized)
            unsafe fn invert(&mut self, s: &Slock);

            /// It must be guaranteed by the caller
            /// the other type is exactly the same as our type
            /// and with the same id
            unsafe fn right_multiply(&mut self, by: Box<dyn DirectlyInvertible>);

            // gets a pointer to the action instance
            // (void pointer)
            unsafe fn action_pointer(&self) -> *const ();

            // forgets the reference action without dropping it
            unsafe fn forget_action(&mut self);
        }
    }

    #[allow(private_bounds)]
    pub trait DirectlyInvertible: Send + sealed::DirectlyInvertibleBase {

        fn id(&self) -> usize;
    }

    /* trait aliases */
    pub trait GeneralListener : Fn(&Slock) -> bool + Send + 'static {}
    pub trait InverseListener : Fn(Box<dyn DirectlyInvertible>, &Slock) -> bool + Send + 'static {}
    impl<T> GeneralListener for T where T: Fn(&Slock) -> bool + Send + 'static {}
    impl<T> InverseListener for T where T: Fn(Box<dyn DirectlyInvertible>, &Slock) -> bool + Send + 'static {}

    pub(super) type BoxInverseListener = Box<
        dyn Fn(Box<dyn DirectlyInvertible>, &Slock) -> bool + Send
    >;

    pub(super) enum StateListener<S: Stateful> {
        ActionListener(Box<dyn (Fn(&S, &S::Action, &Slock) -> bool) + Send>),
        SignalListener(Box<dyn (Fn(&S, &Slock) -> bool) + Send>),
        GeneralListener(Box<dyn Fn(&Slock) -> bool + Send>),
    }
}
pub use listener::*;

mod group {
    use std::ops::Mul;
    use crate::state::{Binding, GeneralListener, InverseListener};
    use crate::core::{Slock, ThreadMarker};

    pub trait Stateful: Send + Sized + 'static {
        type Action: GroupAction<Self>;

        fn subtree_general_listener<F, A>(&self, _container: &impl Binding<Self, A>, _f: F, _s: &Slock<impl ThreadMarker>)
            where F: GeneralListener + Clone, A: ActionFilter<Target=Self> {

        }

        fn subtree_inverse_listener<F, A>(&self, _container: &impl Binding<Self, A>, _f: F, _s: &Slock<impl ThreadMarker>)
            where F: InverseListener + Clone, A: ActionFilter<Target=Self> {

        }
    }

    pub trait GroupBasis<T>: Send + Sized + 'static {
        // returns inverse action
        fn apply(self, to: &mut T) -> Self;
    }

    pub trait GroupAction<T>: GroupBasis<T> + Mul<Output=Self>
        where T: Stateful {

        fn identity() -> Self;
    }

    /// it's more natural to make this just an impl<T: Stateful>
    /// but this runs into (what I believe are provably wrong) errors
    /// of possibly conflicting implementations
    pub trait IntoAction<A, T> where A: GroupAction<T>, T: Stateful {
        fn into_action(self, target: &T) -> A;
    }

    impl<A, T> IntoAction<A, T> for A where A: GroupAction<T>, T: Stateful {
        fn into_action(self, _target: &T) -> A {
            self
        }
    }

    mod word {
        use std::marker::PhantomData;
        use std::ops::Mul;
        use crate::state::{GroupAction, GroupBasis, IntoAction, Stateful};

        pub struct Word<T, B> where T: Stateful, B: GroupBasis<T> {
            items: Vec<B>,
            stateful: PhantomData<T>
        }

        impl<T, B> Clone for Word<T, B> where B: GroupBasis<T> + Clone, T: Stateful {
            fn clone(&self) -> Self {
                Word {
                    items: self.items.clone(),
                    stateful: PhantomData
                }
            }
        }
        impl<T, B> Word<T, B> where B: GroupBasis<T>, T: Stateful {
            pub fn new(word: Vec<B>) -> Self {
                Word {
                    items: word,
                    stateful: PhantomData
                }
            }

            pub fn iter(&self) -> impl Iterator<Item=&B> {
                self.items.iter()
            }
        }

        impl<T, B> IntoIterator for Word<T, B> where B: GroupBasis<T>, T: Stateful {
            type Item = B;
            type IntoIter = <Vec<B> as IntoIterator>::IntoIter;

            fn into_iter(self) -> Self::IntoIter {
                self.items.into_iter()
            }
        }

        impl<T, B> Mul for Word<T, B> where B: GroupBasis<T>, T: Stateful {
            type Output = Self;

            fn mul(mut self, mut rhs: Self) -> Self::Output {
                self.items.append(&mut rhs.items);

                self
            }
        }

        impl<T, B> GroupBasis<T> for Word<T, B> where T: Stateful, B: GroupBasis<T> {
            fn apply(self, to: &mut T) -> Self {
                let bases = self.items;

                // find inverse
                let mut build = bases.into_iter()
                    .map(|b| b.apply(to))
                    .collect::<Vec<_>>();
                build.reverse();

                Word::new(build)
            }
        }

        impl<T, B> GroupAction<T> for Word<T, B> where T: Stateful, B: GroupBasis<T> {

            fn identity() -> Self {
                Word::new(Vec::new())
            }
        }

        impl<T, B> IntoAction<Word<T, B>, T> for B where T: Stateful, B: GroupBasis<T> {
            fn into_action(self, _target: &T) -> Word<T, B> {
                Word::new(vec![self])
            }
        }
    }
    pub use word::*;

    mod filter {
        use std::marker::PhantomData;
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{Stateful};

        pub trait ActionFilter: Send + 'static {
            type Target: Stateful;
            
            fn new() -> Self;

            fn add_filter<F>(&mut self, f: F)
                where F: Send + 'static + Fn(&Self::Target, <Self::Target as Stateful>::Action, &Slock) -> <Self::Target as Stateful>::Action;

            fn filter(&self, val: &Self::Target, a: <Self::Target as Stateful>::Action, s: &Slock<impl ThreadMarker>) -> <Self::Target as Stateful>::Action;
        }

        pub struct Filter<S: Stateful>(
            Vec<Box<dyn Send + Fn(&S, S::Action, &Slock) -> S::Action>>
        );

        // generic parameter is needed for some weird things...
        pub struct Filterless<S>(PhantomData<S>);

        impl<S> ActionFilter for Filterless<S> where S: Stateful {
            type Target = S;

            fn new() -> Self {
                Filterless(PhantomData)
            }

            fn add_filter<F>(&mut self, _f: F) where F: Send + 'static + Fn(&S, S::Action, &Slock) -> S::Action {

            }

            #[inline]
            fn filter(&self, _val: &S, a: S::Action, _s: &Slock<impl ThreadMarker>) -> S::Action {
                a
            }
        }

        impl<S> ActionFilter for Filter<S> where S: Stateful {
            type Target = S;

            fn new() -> Self {
                Filter(Vec::new())
            }

            fn add_filter<F>(&mut self, f: F) where F: Send + 'static + Fn(&S, S::Action, &Slock) -> S::Action {
                self.0.push(Box::new(f));
            }

            fn filter(&self, val: &S, a: S::Action, s: &Slock<impl ThreadMarker>) -> S::Action {
                self.0
                    .iter()
                    .rfold(a, |a, action| action(val, a, s.as_ref()))
            }
        }
    }
    pub use filter::*;

    mod action {
        mod set_action {
            use std::ops::Mul;
            use crate::state::{GroupAction, GroupBasis, Stateful};

            #[derive(Clone)]
            pub enum SetAction<T>
                where T: Stateful + Copy
            {
                Set(T),
                Identity
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

            impl<T> GroupBasis<T> for SetAction<T>
                where T: Stateful + Copy + 'static
            {
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


            impl<T> GroupAction<T> for SetAction<T>
                where T: Stateful + Copy + 'static
            {
                fn identity() -> Self {
                    SetAction::Identity
                }
            }
        }
        pub use set_action::*;

        mod string_action {
            use std::ops::Range;
            use crate::core::{Slock, ThreadMarker};
            use crate::state::{GeneralListener, GroupBasis, InverseListener, Stateful, StoreContainer, Word};

            #[derive(Clone)]
            pub enum StringActionBasis {
                // start, length, with
                ReplaceSubrange(Range<usize>, String),
            }

            impl GroupBasis<String> for StringActionBasis {
                fn apply(self, to: &mut String) -> Self {
                    match self {
                        StringActionBasis::ReplaceSubrange(range, content) => {
                            let replaced = to[range.clone()].to_owned();
                            let next_range = range.start .. range.start + content.len();
                            to.replace_range(range, &content);

                            StringActionBasis::ReplaceSubrange(next_range, replaced)
                        }
                    }
                }
            }

            impl StoreContainer for String {
                fn subtree_general_listener<F: GeneralListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

                }

                fn subtree_inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: &Slock<impl ThreadMarker>) {

                }
            }

            impl Stateful for String {
                type Action = Word<String, StringActionBasis>;
            }
        }
        pub use string_action::*;

        mod vector_action {
            use crate::core::{Slock, ThreadMarker};
            use crate::state::{ActionFilter, Binding, GeneralListener, GroupBasis, InverseListener, Stateful, StoreContainer, Word};

            #[derive(Clone)]
            pub enum VectorActionBasis<T> {
                /* indices */
                Insert(T, usize),
                Remove(usize),
                Swap(usize, usize)
            }

            impl<T> GroupBasis<Vec<T>> for VectorActionBasis<T>
                where T: StoreContainer
            {
                fn apply(self, to: &mut Vec<T>) -> Self {
                    match self {
                        VectorActionBasis::Insert(elem, at) => {
                            to.insert(at, elem);
                            VectorActionBasis::Remove(at)
                        },
                        VectorActionBasis::Remove(at) => {
                            let removed = to.remove(at);
                            VectorActionBasis::Insert(removed, at)
                        }
                        VectorActionBasis::Swap(a, b) => {
                            to.swap(a, b);
                            VectorActionBasis::Swap(a, b)
                        }
                    }
                }
            }

            /* the amount of stores can be variable so that we must add the listeners dynamically */
            /* in certain cases (for inverse listener), some listeners can be held on a bit longer than they ideally should be */
            /* but this is somewhat hard to avoid */
            impl<T> Stateful for Vec<T> where T: StoreContainer {
                type Action = Word<Vec<T>, VectorActionBasis<T>>;

                fn subtree_general_listener<F, A>(&self, container: &impl Binding<Self, A>, f: F, s: &Slock<impl ThreadMarker>)
                    where F: GeneralListener + Clone, A: ActionFilter<Target=Self> {
                    for store in self {
                        store.subtree_general_listener(f.clone(), s);
                    }

                    container.action_listener(move |_v, w, s| {
                        for a in w.iter() {
                            match a {
                                VectorActionBasis::Insert(store, _) => {
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
                    }, s);
                }

                fn subtree_inverse_listener<F, A>(&self, container: &impl Binding<Self, A>, f: F, s: &Slock<impl ThreadMarker>)
                    where F: InverseListener + Clone, A: ActionFilter<Target=Self> {

                    container.action_listener(move |_, w, s| {
                        for a in w.iter() {
                            match a {
                                VectorActionBasis::Insert(store, _) => {
                                    /* make sure it is updated of the inverse listener */
                                    store.subtree_inverse_listener(f.clone(), s);
                                }
                                _ => {
                                    /* nothing necessary here either (only care about updates) */
                                }
                            }
                        }

                        // no way around this, must subscribe forever (??)
                        true
                    }, s);
                }
            }
        }
        pub use vector_action::*;

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

    mod stateful {
        use super::action::{SetAction};
        use crate::state::{Stateful};

        impl Stateful for bool { type Action = SetAction<bool>; }

        impl Stateful for usize { type Action = SetAction<Self>; }

        impl Stateful for isize { type Action = SetAction<Self>; }

        impl Stateful for u8 { type Action = SetAction<Self>; }

        impl Stateful for u16 { type Action = SetAction<Self>; }

        impl Stateful for u32 { type Action = SetAction<Self>; }

        impl Stateful for u64 { type Action = SetAction<Self>; }

        impl Stateful for i8 { type Action = SetAction<Self>; }

        impl Stateful for i16 { type Action = SetAction<Self>; }

        impl Stateful for i32 { type Action = SetAction<Self>; }

        impl Stateful for i64 { type Action = SetAction<Self>; }

        impl Stateful for f32 { type Action = SetAction<Self>; }

        impl Stateful for f64 { type Action = SetAction<Self>; }
    }
    #[allow(unused_imports)]
    pub use stateful::*;
}
pub use group::*;

mod coupler {
    use std::marker::PhantomData;
    use std::str::FromStr;
    use crate::state::{ActionFilter, Filter, Filterless, GroupAction, GroupBasis, IntoAction, SetAction, Stateful, StringActionBasis, Word};
    use crate::state::coupler::sealed_base::CouplerBase;

    mod sealed_base {
        use crate::state::{ActionFilter, Stateful};

        // Associated types make more sense, but then
        // we get conflicting implementations...
        pub trait CouplerBase<I, M, F>: Send + 'static
            where I: Stateful, M: Stateful, F: ActionFilter<Target=M>
        {

            fn initial_mapped(&self, initial_intrinsic: &I) -> M;

            fn mirror_intrinsic_to_mapped(
                &self,
                mapped: &M,
                intrinsic: &I,
                intrinsic_transaction: &I::Action
            ) -> M::Action;

            fn filter_mapped_and_mirror_to_intrinsic(
                &self,
                mapped: &M,
                intrinsic: &I,
                mapped_transaction: M::Action,
            ) -> (I::Action, M::Action);
        }
    }

    pub trait Coupler<I, M, F>: CouplerBase<I, M, F>
        where I: Stateful, M: Stateful, F: ActionFilter<Target=M>
    {

    }

    pub trait FilteringCoupler: Send + 'static {
        type Intrinsic: Stateful;
        type Mapped: Stateful;

        fn initial_mapped(&self, initial_intrinsic: &Self::Intrinsic) -> Self::Mapped;
        fn mirror_intrinsic_to_mapped(
            &self,
            current_mapped: &Self::Mapped,
            prior_intrinsic: &Self::Intrinsic,
            intrinsic_transaction: &<Self::Intrinsic as Stateful>::Action
        ) -> <Self::Mapped as Stateful>::Action;

        fn filter_mapped_and_mirror_to_intrinsic(
            &self,
            prior_mapped: &Self::Mapped,
            current_intrinsic: &Self::Intrinsic,
            mapped_transaction: <Self::Mapped as Stateful>::Action,
        ) -> (<Self::Intrinsic as Stateful>::Action, <Self::Mapped as Stateful>::Action);
    }

    impl<FC> CouplerBase<FC::Intrinsic, FC::Mapped, Filter<FC::Mapped>> for FC
        where FC: FilteringCoupler {
        fn initial_mapped(&self, initial_intrinsic: &FC::Intrinsic) -> FC::Mapped {
            FC::initial_mapped(self, initial_intrinsic)
        }

        fn mirror_intrinsic_to_mapped(&self, current_mapped: &FC::Mapped, prior_intrinsic: &FC::Intrinsic, intrinsic_transaction: &<FC::Intrinsic as Stateful>::Action) -> <FC::Mapped as Stateful>::Action {
            FC::mirror_intrinsic_to_mapped(self, current_mapped, prior_intrinsic, intrinsic_transaction)
        }

        fn filter_mapped_and_mirror_to_intrinsic(
            &self,
            prior_mapped: &FC::Mapped,
            current_intrinsic: &FC::Intrinsic,
            mapped_transaction: <FC::Mapped as Stateful>::Action
        ) -> (<FC::Intrinsic as Stateful>::Action, <FC::Mapped as Stateful>::Action) {
            FC::filter_mapped_and_mirror_to_intrinsic(self, prior_mapped, current_intrinsic, mapped_transaction, )
        }
    }

    impl<FC> Coupler<FC::Intrinsic, FC::Mapped, Filter<FC::Mapped>> for FC
        where FC: FilteringCoupler {

    }

    pub trait FilterlessCoupler: Send + 'static {
        type Intrinsic: Stateful;
        type Mapped: Stateful;

        fn initial_mapped(&self, initial_intrinsic: &Self::Intrinsic) -> Self::Mapped;

        fn mirror_intrinsic_to_mapped(
            &self,
            current_mapped: &Self::Mapped,
            prior_intrinsic: &Self::Intrinsic,
            intrinsic_transaction: &<Self::Intrinsic as Stateful>::Action
        ) -> <Self::Mapped as Stateful>::Action;

        fn mirror_mapped_to_intrinsic(
            &self,
            prior_mapped: &Self::Mapped,
            current_intrinsic: &Self::Intrinsic,
            mapped_transaction: &<Self::Mapped as Stateful>::Action
        ) -> <Self::Intrinsic as Stateful>::Action;
    }

    impl<FC> CouplerBase<FC::Intrinsic, FC::Mapped, Filterless<FC::Mapped>> for FC
        where FC: FilterlessCoupler {
        fn initial_mapped(&self, initial_intrinsic: &FC::Intrinsic) -> FC::Mapped {
            FC::initial_mapped(self, initial_intrinsic)
        }

        fn mirror_intrinsic_to_mapped(&self, mapped: &FC::Mapped, intrinsic: &FC::Intrinsic, intrinsic_transaction: &<FC::Intrinsic as Stateful>::Action) -> <FC::Mapped as Stateful>::Action {
            FC::mirror_intrinsic_to_mapped(self, mapped, intrinsic, intrinsic_transaction)
        }

        fn filter_mapped_and_mirror_to_intrinsic(&self, mapped: &FC::Mapped, intrinsic: &FC::Intrinsic, mapped_transaction: <FC::Mapped as Stateful>::Action) -> (<FC::Intrinsic as Stateful>::Action, <FC::Mapped as Stateful>::Action) {
            let intrinsic_transaction = FC::mirror_mapped_to_intrinsic(self, mapped, intrinsic, &mapped_transaction);

            (intrinsic_transaction, mapped_transaction)
        }
    }

    impl<FC> Coupler<FC::Intrinsic, FC::Mapped, Filterless<FC::Mapped>> for FC where FC: FilterlessCoupler {

    }

    pub struct NumericStringCoupler<N>
        where N: Stateful<Action=SetAction<N>> + FromStr + ToString + Copy
    {
        numb: PhantomData<N>
    }
    impl<N> NumericStringCoupler<N>
        where N: Stateful<Action=SetAction<N>> + FromStr + ToString + Copy {
        pub fn new() -> Self {
            NumericStringCoupler {
                numb: PhantomData
            }
        }
    }

    impl<N> FilteringCoupler for NumericStringCoupler<N>
        where N: Stateful<Action=SetAction<N>> + FromStr + ToString + Copy
    {
        type Intrinsic = N;
        type Mapped = String;

        fn initial_mapped(&self, initial_intrinsic: &Self::Intrinsic) -> Self::Mapped {
            initial_intrinsic.to_string()
        }

        fn mirror_intrinsic_to_mapped(&self, current_mapped: &Self::Mapped, _prior_intrinsic: &Self::Intrinsic, intrinsic_transaction: &<Self::Intrinsic as Stateful>::Action) -> <Self::Mapped as Stateful>::Action {
            match intrinsic_transaction {
                SetAction::Set(new_val) => {
                   StringActionBasis::ReplaceSubrange(0 .. current_mapped.len(), new_val.to_string()).into_action(current_mapped)
                },
                SetAction::Identity => {
                    Word::identity()
                }
            }
        }

        fn filter_mapped_and_mirror_to_intrinsic(&self, prior_mapped: &Self::Mapped, _current_intrinsic: &Self::Intrinsic, mapped_transaction: <Self::Mapped as Stateful>::Action) -> (<Self::Intrinsic as Stateful>::Action, <Self::Mapped as Stateful>::Action) {
            let cloned_action = mapped_transaction.clone();
            let mut new_mapped = prior_mapped.clone();
            cloned_action.apply(&mut new_mapped);
            match new_mapped.parse::<N>() {
                Ok(res) => (SetAction::Set(res), mapped_transaction),
                Err(_) => (SetAction::identity(), Word::identity())
            }
        }
    }
}

mod store {
    use crate::core::{Slock, ThreadMarker};
    use crate::state::{ActionFilter, Filterless, GeneralSignal, IntoAction, Signal, Stateful};
    use crate::state::listener::{GeneralListener, InverseListener, StateListener};

    /// It is the implementors job to guarantee that subtree_listener
    /// and relatives do not get into call cycles
    pub trait StoreContainer: Send + Sized + 'static {
        fn subtree_general_listener<F: GeneralListener + Clone>(&self, f: F, s: &Slock<impl ThreadMarker>);

        fn subtree_inverse_listener<F: InverseListener + Clone>(&self, f: F, s: &Slock<impl ThreadMarker>);
    }

    pub trait ActionDispatcher<S: Stateful, F: ActionFilter<Target=S>> {
        fn action_listener<G>(&self, listener: G, s: &Slock<impl ThreadMarker>)
            where G: Send + Fn(&S, &S::Action, &Slock) -> bool + 'static;
    }

    // Like with signal, I believe it makes more sense for
    // S to be an associated type, but then we we can't have default
    // filterless? So, it is done for consistency as a generic parameter
    pub trait Binding<S: Stateful, F: ActionFilter<Target=S>=Filterless<S>>: ActionDispatcher<S, F> + Signal<S> {
        fn apply(&self, action: impl IntoAction<S::Action, S>, s: &Slock);
    }

    mod sealed_base {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::ops::Deref;
        use std::sync::Arc;
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{Binding, Signal};
        use super::{ActionFilter, GeneralSignal, IntoAction, Stateful};
        use super::{GeneralListener, InverseListener, StateListener};
        use super::StateRef;

        pub(super) trait RawStoreBase<S, F>: 'static where S: Stateful, F: ActionFilter<Target=S> {
            fn apply(inner: &Arc<RefCell<Self>>, action: impl IntoAction<S::Action, S>, s: &Slock<impl ThreadMarker>, skip_filters: bool);

            fn listen(&mut self, listener: StateListener<S>, s: &Slock<impl ThreadMarker>);

            fn subtree_general_listener(&mut self, binding: &impl Binding<S, F>,  f: impl GeneralListener + Clone, s: &Slock<impl ThreadMarker>);
            fn subtree_inverse_listener(&mut self, binding: &impl Binding<S, F>, f: impl InverseListener + Clone, s: &Slock<impl ThreadMarker>);

            fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, &Slock) -> S::Action + 'static;

            fn data(&self) -> &S;
        }

        /* IMO this is a bad side effect of rust's insistence on
           having no duplicate implementations. What could be done
           as impl<R: RawStore...> Binding for R now becomes an awkward
           derivation, with lots of duplicate code
         */
        pub(super) trait RawStoreSharedOwnerBase<S, F> : Sized + Signal<S> where S: Stateful, F: ActionFilter<Target=S> {
            type Inner: RawStoreBase<S, F>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>>;

            // used only for creating the binding
            fn clone(&self) -> Self;

            fn _action_listener(&self, f: Box<dyn Fn(&S, &S::Action, &Slock) -> bool + Send>, s: &Slock<impl ThreadMarker>) {
                self.get_ref().borrow_mut().listen(StateListener::ActionListener(f), s);
            }

            fn _borrow<'a>(&'a self, _s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=S> {
                StateRef {
                    main_ref: self.get_ref().borrow(),
                    lifetime: PhantomData,
                    filter: PhantomData,
                }
            }

            fn _listen<G>(&self, listener: G, s: &Slock<impl ThreadMarker>)
                where G: Fn(&S, &Slock) -> bool + Send + 'static {
                self.get_ref().borrow_mut().listen(StateListener::SignalListener(Box::new(listener)), s)
            }

            fn _map<T, G>(&self, map: G, s: &Slock<impl ThreadMarker>) -> GeneralSignal<T>
                where T: Send + 'static, G: Send + 'static + Fn(&S) -> T {
                GeneralSignal::from(self, map, |this, listener, slock| {
                    this.get_ref().borrow_mut().listen(StateListener::SignalListener(listener), slock)
                }, s)
            }
        }
    }

    mod raw_store {
        use crate::state::{ActionFilter, Stateful};
        use crate::state::store::sealed_base::RawStoreBase;

        #[allow(private_bounds)]
        pub trait RawStore<S, F>: RawStoreBase<S, F>
            where S: Stateful, F: ActionFilter<Target=S> {}

        impl<S, F, R> RawStore<S, F> for R where S: Stateful, F: ActionFilter<Target=S>, R: RawStoreBase<S, F> {

        }
    }
    pub use raw_store::*;

    mod raw_store_shared_owner {
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{ActionDispatcher, ActionFilter, Binding, IntoAction, Stateful};
        use super::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        #[allow(private_bounds)]
        pub trait RawStoreSharedOwner<S, F>: RawStoreSharedOwnerBase<S, F>
            where S: Stateful, F: ActionFilter<Target=S> {}

        impl<S, F, R> RawStoreSharedOwner<S, F> for R
            where S: Stateful, F: ActionFilter<Target=S>, R: RawStoreSharedOwnerBase<S, F> {

        }

        impl<S, F, I> ActionDispatcher<S, F> for I
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F> {
            fn action_listener<G>(&self, listener: G, s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, &S::Action, &Slock) -> bool + 'static {
                self._action_listener(Box::new(listener), s);
            }
        }

        // Unfortunately can't do this for signal as well
        // Since FixedSignal 'might' implement RawStoreSharedOwnerBase
        impl<S, F, R> Binding<S, F> for R where
            S: Stateful, F: ActionFilter<Target=S>, R: RawStoreSharedOwnerBase<S, F> {
            fn apply(&self, action: impl IntoAction<S::Action, S>, s: &Slock) {
                R::Inner::apply(self.get_ref(), action, s, false);
            }
        }
    }
    pub use raw_store_shared_owner::*;

    mod action_inverter {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::sync::Weak;
        use crate::core::Slock;
        use crate::state::listener::DirectlyInvertible;
        use crate::state::listener::sealed::DirectlyInvertibleBase;
        use crate::state::{ActionFilter, Stateful};
        use super::RawStore;

        pub(super) struct ActionInverter<S, F, I> where S: Stateful, F: ActionFilter<Target=S>, I: RawStore<S, F> {
            pub action: Option<S::Action>,
            pub state: Weak<RefCell<I>>,
            pub filter: PhantomData<F>
        }

        impl<S, F, I> DirectlyInvertibleBase for ActionInverter<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStore<S, F> {
            unsafe fn invert(&mut self, s: &Slock) {
                let Some(state) = self.state.upgrade() else {
                    return;
                };

                // skip filters on inversion to avoid weird behavior
                I::apply(&state, self.action.take().unwrap(), s, true);
            }

            unsafe fn right_multiply(&mut self, mut by: Box<dyn DirectlyInvertible>) {
                /* we are free to assume by is of type Self, allowing us to do this conversion */
                let ptr = by.action_pointer() as *const S::Action;
                self.action = Some(self.action.take().unwrap() * std::ptr::read(ptr));
                /* we have implicitly moved the other's action, now we must tell it to forget to
                   avoid double free
                 */
                by.forget_action();
            }

            unsafe fn action_pointer(&self) -> *const () {
                self.action.as_ref().unwrap() as *const S::Action as *const ()
            }

            unsafe fn forget_action(&mut self) {
                std::mem::forget(self.action.take());
            }
        }

        impl<S, F, I> DirectlyInvertible for ActionInverter<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStore<S, F> {

            fn id(&self) -> usize {
                self.state.as_ptr() as usize
            }
        }

        // safety: all operations are either unsafe or require the slock
        unsafe impl<S, F, I> Send for ActionInverter<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStore<S, F> {
        }
    }
    use action_inverter::*;

    mod state_ref {
        use std::cell::Ref;
        use std::marker::PhantomData;
        use std::ops::Deref;
        use crate::state::{ActionFilter, Stateful};
        use crate::state::store::RawStore;

        pub(super) struct StateRef<'a, S, M, I>
            where S: Stateful, M: ActionFilter<Target=S>, I: RawStore<S, M> {
            pub(super) main_ref: Ref<'a, I>,
            pub(super) lifetime: PhantomData<&'a S>,
            pub(super) filter: PhantomData<&'a M>,
        }

        impl<'a, S, M, I> Deref for StateRef<'a, S, M, I>
            where S: Stateful, M: ActionFilter<Target=S>, I: RawStore<S, M> {
            type Target = S;
            fn deref(&self) -> &S {
                self.main_ref.data()
            }
        }
    }
    use state_ref::*;

    mod bindable {
        use std::marker::PhantomData;
        use crate::state::{ActionFilter, Binding, GeneralBinding, Signal, Stateful};
        use crate::state::store::RawStoreSharedOwner;

        pub trait Bindable<S: Stateful, F: ActionFilter<Target=S>> {
            type Binding: Binding<S, F> + Clone;

            fn binding(&self) -> Self::Binding;
            fn signal(&self) -> impl Signal<S> + Clone;
        }


        impl<S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F>> Bindable<S, F> for I {
            type Binding = GeneralBinding<S, F, I>;

            fn binding(&self) -> Self::Binding {
                GeneralBinding {
                    inner: self.clone(),
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
            fn action_filter<G>(&self, filter: G, s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, &Slock) -> S::Action + 'static;
        }

        impl<S: Stateful, I: RawStoreSharedOwner<S, Filter<S>>> Filterable<S> for I {
            fn action_filter<G>(&self, filter: G, s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, &Slock) -> S::Action + 'static {
                self.get_ref().borrow_mut().action_filter(filter, s);
            }
        }
    }
    pub use filterable::*;

    mod store {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::{
            state::{ActionFilter, BoxInverseListener, Filterless, IntoAction, Signal, Stateful, StoreContainer, GeneralSignal},
            core::Slock,
            core::ThreadMarker,
        };
        use crate::state::{Binding, Filter, GroupBasis};
        use crate::state::listener::{GeneralListener, InverseListener, StateListener};
        use super::ActionInverter;
        use super::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        pub(super) struct InnerStore<S: Stateful, F: ActionFilter<Target=S>> {
            data: S,
            listeners: Vec<StateListener<S>>,
            inverse_listener: Option<BoxInverseListener>,
            filter: F,
        }

        impl<S, F> RawStoreBase<S, F> for InnerStore<S, F>
            where S: Stateful, F: ActionFilter<Target=S>
        {
            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<S::Action, S>, s: &Slock<impl ThreadMarker>, skip_filters: bool) {
                #[cfg(debug_assertions)] {
                    debug_assert_eq!(s.debug_info.applying_transaction.borrow().len(), 0, "Fatal: store \
                    changed as a result of the change of another state variable. \
                    Stores, by default, are to be independent of other stores. If you would like one store to \
                    be dependent on another, check out DerivedStore (or in some circumstances, maybe CoupledStore)");
                    s.debug_info.applying_transaction.borrow_mut().push(arc.as_ptr() as usize);
                }
                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();
                let transaction = alt_action.into_action(&inner.data);

                let action = if skip_filters {
                    transaction
                }
                else {
                    inner.filter.filter(&inner.data, transaction, s)
                };

                // tell action listeners
                inner.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::ActionListener(listener) => listener(&inner.data, &action, s.as_ref()),
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
                    let inverter = ActionInverter {
                        action: Some(inverse),
                        state: Arc::downgrade(arc),
                        filter: PhantomData
                    };
                    if !inv_listener(Box::new(inverter), s.as_ref()) {
                        inner.inverse_listener = None;
                    }
                }

                #[cfg(debug_assertions)]
                {
                    s.debug_info.applying_transaction.borrow_mut().pop();
                }
            }

            fn listen(&mut self, listener: StateListener<S>, _s: &Slock<impl ThreadMarker>) {
                self.listeners.push(listener);
            }

            fn subtree_general_listener(&mut self, binding: &impl Binding<S, F>, f: impl GeneralListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.subtree_general_listener(binding, f.clone(), s);
                self.listen(StateListener::GeneralListener(Box::new(f)), s);
            }

            fn subtree_inverse_listener(&mut self, binding: &impl Binding<S, F>, f: impl InverseListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.subtree_inverse_listener(binding, f.clone(), s);
                self.inverse_listener = Some(Box::new(f));
            }

            fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, &Slock) -> S::Action + 'static {
                self.filter.add_filter(filter);
            }

            fn data(&self) -> &S {
                &self.data
            }
        }

        pub struct Store<S: Stateful, F: ActionFilter<Target=S>=Filterless<S>>
        {
            pub(super) inner: Arc<RefCell<InnerStore<S, F>>>
        }

        impl<S> Store<S, Filterless<S>>
            where S: Stateful
        {
            pub fn new(initial: S) -> Self {
                Store {
                    inner: Arc::new(RefCell::new(InnerStore {
                        data: initial,
                        listeners: Vec::new(),
                        inverse_listener: None,
                        filter: Filterless::new()
                    }))
                }
            }
        }

        impl<S> Store<S, Filter<S>>
            where S: Stateful
        {
            pub fn new_with_filter(initial: S) -> Self {
                Store {
                    inner: Arc::new(RefCell::new(InnerStore {
                        data: initial,
                        listeners: Vec::new(),
                        inverse_listener: None,
                        filter: Filter::new()
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

        impl<S> Default for Store<S, Filter<S>>
            where S: Stateful + Default
        {
            fn default() -> Self {
                Self::new_with_filter(S::default())
            }
        }

        impl<S, M> StoreContainer for Store<S, M>
            where S: Stateful, M: ActionFilter<Target=S>
        {
            fn subtree_general_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: GeneralListener + Clone {
                self.inner.borrow_mut().subtree_general_listener(self, f, s);
            }

            fn subtree_inverse_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: InverseListener + Clone {
                self.inner.borrow_mut().subtree_inverse_listener(self, f, s);
            }
        }

        impl<S, A> Signal<S> for Store<S, A>
            where S: Stateful, A: ActionFilter<Target=S>
        {
            fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=S> {
                self._borrow(s)
            }

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>)
                where F: Fn(&S, &Slock) -> bool + Send + 'static {
                self._listen(listener, s);
            }

            type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
            fn map<U, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U>
                where U: Send + 'static, F: Send + 'static + Fn(&S) -> U {
                self._map(map, s)
            }
        }

        impl<S, F> RawStoreSharedOwnerBase<S, F> for Store<S, F>
            where S: Stateful, F: ActionFilter<Target=S>
        {
            type Inner = InnerStore<S, F>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }

            fn clone(&self) -> Self {
                Store {
                    inner: Arc::clone(&self.inner)
                }
            }
        }

        // safety: all accesses to inner are done using the global state lock
        // and Stateful: Send
        unsafe impl<S, F> Send for Store<S, F> where S: Stateful, F: ActionFilter<Target=S> { }
        unsafe impl<S, F> Sync for Store<S, F> where S: Stateful, F: ActionFilter<Target=S> { }
    }
    pub use store::*;

    mod token_store {
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::hash::Hash;
        use std::marker::PhantomData;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{ActionFilter, Binding, BoxInverseListener, Filter, Filterless, GeneralListener, GeneralSignal, GroupBasis, IntoAction, InverseListener, Signal, Stateful, StoreContainer};
        use crate::state::listener::StateListener;
        use crate::state::store::action_inverter::ActionInverter;
        use crate::state::store::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        pub(super) struct InnerTokenStore<S: Stateful + Copy + Hash + Eq, F: ActionFilter<Target=S>> {
            data: S,
            listeners: Vec<StateListener<S>>,
            equal_listeners: HashMap<S, Vec<Box<dyn Fn(&S, &Slock) -> bool + Send>>>,
            inverse_listener: Option<BoxInverseListener>,
            filter: F
        }
        impl<S, F> RawStoreBase<S, F> for InnerTokenStore<S, F>
            where S: Stateful + Copy + Hash + Eq,
                  F: ActionFilter<Target=S>
        {
            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<S::Action, S>, s: &Slock<impl ThreadMarker>, skip_filters: bool) {
                #[cfg(debug_assertions)] {
                    debug_assert_eq!(s.debug_info.applying_transaction.borrow().len(), 0, "Fatal: token store \
                    changed as a result of the change of another state variable. \
                    Stores, by default, are to be independent of other stores. If you would like one store to \
                    be dependent on another, check out DerivedStore (or in some circumstances, maybe CoupledStore)");
                    s.debug_info.applying_transaction.borrow_mut().push(arc.as_ptr() as usize);
                }
                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();
                let transaction = alt_action.into_action(&inner.data);

                let action = if skip_filters {
                    transaction
                }
                else {
                    inner.filter.filter(&inner.data, transaction, s)
                };

                // tell action listeners
                inner.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::ActionListener(listener) => {
                            listener(&inner.data, &action, s.as_ref())
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
                    let inverter = ActionInverter {
                        action: Some(inverse),
                        state: Arc::downgrade(arc),
                        filter: PhantomData
                    };
                    if !inv_listener(Box::new(inverter), s.as_ref()) {
                        inner.inverse_listener = None;
                    }
                }

                #[cfg(debug_assertions)]
                {
                    s.debug_info.applying_transaction.borrow_mut().pop();
                }
            }

            fn listen(&mut self, listener: StateListener<S>, _s: &Slock<impl ThreadMarker>) {
                self.listeners.push(listener);
            }

            fn subtree_general_listener(&mut self, binding: &impl Binding<S, F>, f: impl GeneralListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.subtree_general_listener(binding, f.clone(), s);
                self.listen(StateListener::GeneralListener(Box::new(f)), s);
            }

            fn subtree_inverse_listener(&mut self, binding: &impl Binding<S, F>, f: impl InverseListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.subtree_inverse_listener(binding, f.clone(), s);
                self.inverse_listener = Some(Box::new(f));
            }

            fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, &Slock) -> S::Action + 'static {
                self.filter.add_filter(filter);
            }

            fn data(&self) -> &S {
                &self.data
            }
        }

        pub struct TokenStore<S, F=Filterless<S>>
            where S: Stateful + Copy + Hash + Eq, F: ActionFilter<Target=S> {
            inner: Arc<RefCell<InnerTokenStore<S, F>>>
        }

        impl<S, F> TokenStore<S, F> where S: Stateful + Copy + Hash + Eq, F: ActionFilter<Target=S> {
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

        impl<S> TokenStore<S, Filterless<S>> where S: Stateful + Copy + Hash + Eq {
            pub fn new(initial: S) -> Self {
                TokenStore {
                    inner: Arc::new(RefCell::new(InnerTokenStore {
                        data: initial,
                        listeners: Vec::new(),
                        inverse_listener: None,
                        equal_listeners: HashMap::new(),
                        filter: Filterless::new()
                    }))
                }
            }
        }

        impl<S> TokenStore<S, Filter<S>> where S: Stateful + Copy + Hash + Eq {
            pub fn new_with_filter(initial: S) -> Self {
                TokenStore {
                    inner: Arc::new(RefCell::new(InnerTokenStore {
                        data: initial,
                        listeners: Vec::new(),
                        inverse_listener: None,
                        equal_listeners: HashMap::new(),
                        filter: Filter::new()
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
            where S: Stateful + Copy + Hash + Eq, M: ActionFilter<Target=S> {
            fn subtree_general_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: GeneralListener + Clone {
                self.inner.borrow_mut().subtree_general_listener(self, f, s);
            }

            fn subtree_inverse_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: InverseListener + Clone {
                self.inner.borrow_mut().subtree_inverse_listener(self, f, s);
            }
        }

        impl<S, A> Signal<S> for TokenStore<S, A>
            where S: Stateful + Copy + Hash + Eq, A: ActionFilter<Target=S> {
            fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=S> {
                self._borrow(s)
            }

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>)
                where F: Fn(&S, &Slock) -> bool + Send + 'static {
                self._listen(listener, s);
            }

            type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
            fn map<U, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U>
                where U: Send + 'static, F: Send + 'static + Fn(&S) -> U {
                self._map(map, s)
            }
        }

        impl<S, A> RawStoreSharedOwnerBase<S, A> for TokenStore<S, A>
            where S: Stateful + Copy + Hash + Eq, A: ActionFilter<Target=S> {
            type Inner = InnerTokenStore<S, A>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }

            fn clone(&self) -> Self {
                TokenStore {
                    inner: Arc::clone(&self.inner)
                }
            }
        }

        // safety: all accesses to inner are done using the global state lock
        // and Stateful: Send
        unsafe impl<S, F> Send for TokenStore<S, F> where S: Stateful + Copy + Hash + Eq, F: ActionFilter<Target=S> { }
        unsafe impl<S, F> Sync for TokenStore<S, F> where S: Stateful + Copy + Hash + Eq, F: ActionFilter<Target=S> { }
    }
    pub use token_store::*;

    mod derived_store {
        use std::cell::RefCell;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::{
            state::{
                ActionFilter, Filterless, IntoAction, Signal, Stateful, StoreContainer, GeneralSignal,
            },
            core::Slock,
            core::ThreadMarker,
        };
        use crate::state::{Binding, Filter, GroupBasis};
        use crate::state::listener::{GeneralListener, InverseListener, StateListener};
        use super::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        pub(super) struct InnerDerivedStore<S: Stateful, F: ActionFilter<Target=S>> {
            data: S,
            listeners: Vec<StateListener<S>>,
            filter: F,
        }

        impl<S, F> RawStoreBase<S, F> for InnerDerivedStore<S, F>
            where S: Stateful, F: ActionFilter<Target=S>
        {
            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<S::Action, S>, s: &Slock<impl ThreadMarker>, skip_filters: bool) {
                #[cfg(debug_assertions)] {
                    debug_assert_ne!(s.debug_info.applying_transaction.borrow().len(), 0, "Fatal: derived store \
                    changed in a context that was NOT initiated by the change of another store. \
                    Derived store, being 'derived', must only be a function of other state variables. ");
                    s.debug_info.applying_transaction.borrow_mut().push(arc.as_ptr() as usize);
                }

                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();
                let transaction = alt_action.into_action(&inner.data);

                let action = if skip_filters {
                    transaction
                }
                else {
                    inner.filter.filter(&inner.data, transaction, s)
                };

                // tell action listeners
                inner.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::ActionListener(listener) => listener(&inner.data, &action, s.as_ref()),
                        _ => true
                    }
                );

                // apply action
                let _ = action.apply(&mut inner.data);

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

                #[cfg(debug_assertions)]
                {
                    s.debug_info.applying_transaction.borrow_mut().pop();
                }
            }

            fn listen(&mut self, listener: StateListener<S>, _s: &Slock<impl ThreadMarker>) {
                self.listeners.push(listener);
            }

            fn subtree_general_listener(&mut self, binding: &impl Binding<S, F>, f: impl GeneralListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.subtree_general_listener(binding, f.clone(), s);
                self.listen(StateListener::GeneralListener(Box::new(f)), s);
            }

            /// no-op, see store container impl below for why
            fn subtree_inverse_listener(&mut self, _binding: &impl Binding<S, F>, _f: impl InverseListener + Clone, _s: &Slock<impl ThreadMarker>) {
            }

            fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, &Slock) -> S::Action + 'static {
                self.filter.add_filter(filter);
            }

            fn data(&self) -> &S {
                &self.data
            }
        }

        pub struct DerivedStore<S: Stateful, F: ActionFilter<Target=S>=Filterless<S>>
        {
            inner: Arc<RefCell<InnerDerivedStore<S, F>>>
        }

        impl<S> DerivedStore<S, Filterless<S>>
            where S: Stateful
        {
            pub fn new(initial: S) -> Self {
                DerivedStore {
                    inner: Arc::new(RefCell::new(InnerDerivedStore {
                        data: initial,
                        listeners: Vec::new(),
                        filter: Filterless::new()
                    }))
                }
            }
        }

        impl<S> DerivedStore<S, Filter<S>>
            where S: Stateful
        {
            pub fn new_with_filter(initial: S) -> Self {
                DerivedStore {
                    inner: Arc::new(RefCell::new(InnerDerivedStore {
                        data: initial,
                        listeners: Vec::new(),
                        filter: Filter::new()
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

        // In this case, inverse is a no op
        // since the respective action should've been handled
        // by the source store
        impl<S, M> StoreContainer for DerivedStore<S, M>
            where S: Stateful, M: ActionFilter<Target=S>
        {
            fn subtree_general_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                where F: GeneralListener + Clone {
                self.inner.borrow_mut().subtree_general_listener(self, f, s);
            }

            fn subtree_inverse_listener<F>(&self, _f: F, _s: &Slock<impl ThreadMarker>)
                where F: InverseListener + Clone {
            }
        }

        impl<S, A> Signal<S> for DerivedStore<S, A>
            where S: Stateful, A: ActionFilter<Target=S>
        {
            fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=S> {
                self._borrow(s)
            }

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>)
                where F: Fn(&S, &Slock) -> bool + Send + 'static {
                self._listen(listener, s);
            }

            type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
            fn map<U, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U>
                where U: Send + 'static, F: Send + 'static + Fn(&S) -> U {
                self._map(map, s)
            }
        }

        impl<S, F> RawStoreSharedOwnerBase<S, F> for DerivedStore<S, F>
            where S: Stateful, F: ActionFilter<Target=S>
        {
            type Inner = InnerDerivedStore<S, F>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }

            fn clone(&self) -> Self {
                DerivedStore {
                    inner: Arc::clone(&self.inner)
                }
            }
        }

        // safety: all accesses to inner are done using the global state lock
        // and Stateful: Send
        unsafe impl<S, F> Send for DerivedStore<S, F> where S: Stateful, F: ActionFilter<Target=S> { }
        unsafe impl<S, F> Sync for DerivedStore<S, F> where S: Stateful, F: ActionFilter<Target=S> { }
    }
    pub use derived_store::*;

    mod coupled_store {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::ops::{Deref, DerefMut};
        use std::sync::{Arc};
        use crate::state::group::GroupBasis;
        use crate::{
            state::{
                ActionFilter, Filterless, IntoAction, Signal, Stateful, StoreContainer, GeneralSignal,
            },
            core::Slock,
            core::ThreadMarker,
        };
        use crate::state::{Binding};
        use crate::state::coupler::Coupler;
        use crate::state::listener::{GeneralListener, InverseListener, StateListener};
        use crate::util::{UnsafeForceSend};
        use super::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        // note that IB is purposefully filterless
        pub(super) struct InnerCoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I, Filterless<I>>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F> {
            data: M,
            listeners: Vec<StateListener<M>>,
            filter: F,
            coupler: C,
            // intrinsic maintains a weak ownership of us
            // we maintain strong ownership of intrinsic
            // this may seem a bit backwards, but if you think about it
            // it's okay if intrinsic outlives us, but not ok if
            // we outlive intrinsic
            phantom_intrinsic: PhantomData<&'static I>,
            intrinsic: IB,
            intrinsic_performing_transaction: bool,
            performing_transaction: bool,
        }
        impl<I, IB, M, F, C> InnerCoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            fn fully_apply(arc: &Arc<RefCell<Self>>, intrinsic: Option<&I>, alt_action: impl IntoAction<M::Action, M>, s: &Slock<impl ThreadMarker>) {
                /* must only be changed by itself, or by the parent */
                #[cfg(debug_assertions)] {
                    // no need for self checks since if someone other than the parent initiates a transaction
                    // this will be caught by the parent anyways
                    s.debug_info.applying_transaction.borrow_mut().push(arc.as_ptr() as usize);
                }

                arc.borrow_mut().performing_transaction = true;

                // do have to be a bit careful with the reentry on the intrinsic action listener
                // hence the many borrows
                let borrow = arc.borrow();
                let inner = borrow.deref();
                let transaction = alt_action.into_action(&inner.data);

                let action = inner.filter.filter(&inner.data, transaction, s);

                let (intrinsic_transaction, action) = {
                    if inner.intrinsic_performing_transaction {
                        inner.coupler.filter_mapped_and_mirror_to_intrinsic(
                            &inner.data,
                            intrinsic.unwrap(),
                            action
                        )
                    }
                    else {
                        inner.coupler.filter_mapped_and_mirror_to_intrinsic(
                            &inner.data,
                            inner.intrinsic.borrow(s).deref(),
                            action
                        )
                    }
                };

                // tell intrinsic if it didn't originate (it's filterless so doesn't matter about filters)
                if !inner.intrinsic_performing_transaction {
                    /* in this case, it's fine that it's being changed due to another store */
                    #[cfg(debug_assertions)]
                    {
                        s.debug_info.applying_transaction.borrow_mut().pop();
                    }

                    inner.intrinsic.apply(intrinsic_transaction, s.as_ref());

                    #[cfg(debug_assertions)]
                    {
                        s.debug_info.applying_transaction.borrow_mut().push(arc.as_ptr() as usize);
                    }
                }

                // tell action listeners
                drop(borrow);
                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();

                inner.listeners.retain_mut(
                    |listener| match listener {
                        StateListener::ActionListener(listener) => listener(&inner.data, &action, s.as_ref()),
                        _ => true
                    }
                );

                // apply action
                let _ = action.apply(&mut inner.data);

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

                inner.performing_transaction = false;

                #[cfg(debug_assertions)]
                {
                    s.debug_info.applying_transaction.borrow_mut().pop();
                }
            }
        }

        impl<I, IB, M, F, C> RawStoreBase<M, F> for InnerCoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<M::Action, M>, s: &Slock<impl ThreadMarker>, _skip_filters: bool) {
                InnerCoupledStore::fully_apply(arc, None, alt_action, s);
            }

            fn listen(&mut self, listener: StateListener<M>, _s: &Slock<impl ThreadMarker>) {
                self.listeners.push(listener);
            }

            fn subtree_general_listener(&mut self, binding: &impl Binding<M, F>, f: impl GeneralListener + Clone, s: &Slock<impl ThreadMarker>) {
                self.data.subtree_general_listener(binding, f.clone(), s);
                self.listen(StateListener::GeneralListener(Box::new(f)), s);
            }

            fn subtree_inverse_listener(&mut self, _binding: &impl Binding<M, F>, _f: impl InverseListener + Clone, _s: &Slock<impl ThreadMarker>) {

            }

            fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&M, M::Action, &Slock) -> M::Action + 'static {
                self.filter.add_filter(filter);
            }

            fn data(&self) -> &M {
                &self.data
            }
        }

        // IB purposefully filterless
        pub struct CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F> {
            inner: Arc<RefCell<InnerCoupledStore<I, IB, M, F, C>>>
        }

        impl<I, IB, M, F, C> CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            pub fn new(intrinsic: IB, coupler: C, s: &Slock<impl ThreadMarker>) -> Self {
                let data = coupler.initial_mapped(intrinsic.borrow(s).deref());
                let ret = CoupledStore {
                    inner: Arc::new(RefCell::new(InnerCoupledStore {
                        data,
                        listeners: Vec::new(),
                        filter: F::new(),
                        coupler,
                        phantom_intrinsic: PhantomData,
                        intrinsic,
                        intrinsic_performing_transaction: false,
                        performing_transaction: false
                    }))
                };

                // intrinsic listener
                // (our listener is handled manually in apply)
                let this = UnsafeForceSend(Arc::downgrade(&ret.inner));
                ret.inner.borrow().intrinsic.action_listener(move |intrinsic, a, s| {
                    let UnsafeForceSend(weak) = &this;
                    let Some(strong) = weak.upgrade() else {
                        return false;
                    };

                    let this = strong.borrow();
                    // if we didn't originate, then mirror the action
                    if !this.performing_transaction {
                        let coupler = &this.coupler;
                        let our_action = coupler.mirror_intrinsic_to_mapped(this.data(), intrinsic, a);

                        drop(this);
                        strong.borrow_mut().intrinsic_performing_transaction = true;
                        InnerCoupledStore::fully_apply(&strong, Some(intrinsic), our_action, s);
                        strong.borrow_mut().intrinsic_performing_transaction = false;
                    }

                    true
                }, s);

                ret
            }
        }

        // In this case, inverse is a no op
        // since the respective action should've been handled
        // by the source store
        impl<I, IB, M, F, C> StoreContainer for CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            fn subtree_general_listener<G>(&self, f: G, s: &Slock<impl ThreadMarker>)
                where G: GeneralListener + Clone {
                self.inner.borrow_mut().subtree_general_listener(self, f, s);
            }

            fn subtree_inverse_listener<G>(&self, _f: G, _s: &Slock<impl ThreadMarker>)
                where G: InverseListener + Clone {
            }
        }

        impl<I, IB, M, F, C> Signal<M> for CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=M> {
                self._borrow(s)
            }

            fn listen<G>(&self, listener: G, s: &Slock<impl ThreadMarker>)
                where G: Fn(&M, &Slock) -> bool + Send + 'static {
                self._listen(listener, s);
            }

            type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
            fn map<U, G>(&self, map: G, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U>
                where U: Send + 'static, G: Send + 'static + Fn(&M) -> U {
                self._map(map, s)
            }
        }

        impl<I, IB, M, F, C> RawStoreSharedOwnerBase<M, F> for CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            type Inner = InnerCoupledStore<I, IB, M, F, C>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }

            fn clone(&self) -> Self {
                CoupledStore {
                    inner: Arc::clone(&self.inner)
                }
            }
        }

        // safety: all accesses to inner are done using the global state lock
        // and Stateful: Send
        unsafe impl<I, IB, M, F, C> Send for CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F> {}

        unsafe impl<I, IB, M, F, C> Sync for CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F> {}
    }
    pub use coupled_store::*;

    mod general_binding {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::ops::Deref;
        use std::sync::Arc;
        use crate::core::{Slock, ThreadMarker};
        use crate::state::{RawStoreSharedOwner, Signal};
        use crate::state::signal::GeneralSignal;
        use super::{ActionFilter, Stateful};
        use super::sealed_base::RawStoreSharedOwnerBase;

        pub struct GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F> {
            pub(super) inner: I,
            pub(super) phantom_state: PhantomData<S>,
            pub(super) phantom_filter: PhantomData<F>,
        }

        impl<S, F, I> Clone for GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F> {
            fn clone(&self) -> Self {
                GeneralBinding {
                    inner: self.inner.clone(),
                    phantom_state: PhantomData,
                    phantom_filter: PhantomData
                }
            }
        }

        impl<S, A, I> Signal<S> for GeneralBinding<S, A, I>
            where S: Stateful, A: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, A> {
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

        impl<S, A, I> RawStoreSharedOwnerBase<S, A> for GeneralBinding<S, A, I>
            where S: Stateful, A: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, A> {
            type Inner = I::Inner;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                self.inner.get_ref()
            }

            fn clone(&self) -> Self {
                GeneralBinding {
                    inner: self.inner.clone(),
                    phantom_state: PhantomData,
                    phantom_filter: PhantomData
                }
            }
        }

        // Safety: all operations require the slock
        unsafe impl<S, F, I> Send for GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F> {}
        unsafe impl<S, F, I> Sync for GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F> {}
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
            where F: (Fn(&T, &Slock) -> bool) + Send + 'static;

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
            listeners: Vec<Box<dyn Fn(&T, &Slock) -> bool + Send>>
        }

        impl<T> SignalAudience<T> where T: Send {
            pub(super) fn new() -> SignalAudience<T> {
                SignalAudience {
                    listeners: Vec::new()
                }
            }

            pub(super) fn listen<F>(&mut self, listener: F, _s: &Slock<impl ThreadMarker>) where F: (Fn(&T, &Slock) -> bool) + Send + 'static {
                self.listeners.push(Box::new(listener));
            }

            pub(super) fn listen_box(
                &mut self,
                listener: Box<dyn (Fn(&T, &Slock) -> bool) + Send + 'static>,
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

            fn listen<F>(&self, _listener: F, _s: &Slock<impl ThreadMarker>) where F: Fn(&T, &Slock) -> bool + Send {
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
                      G: FnOnce(&S, Box<dyn Fn(&U, &Slock) -> bool + Send>, &Slock)
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
                where F: Fn(&T, &Slock) -> bool + Send + 'static {
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

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>) where F: Fn(&V, &Slock) -> bool + Send + 'static {
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

    /*
    mod timed_signal {
        use std::cell::RefCell;
        use std::sync::Arc;
        use crate::core::timer_subscriber;
        use crate::state::signal::InnerSignal;

        pub trait Capacitor<T>  {
            fn epsilon() -> f64 {
                1e-6
            }

            fn derivative() -> f64 {

            }
        }

        struct TimedInnerSignal<T, C> {
            target: T,
            curr: T,
            capacitor: C
        }

        impl<T, C> InnerSignal<T> for TimedInnerSignal<T, C> where T: Into<f64> + From<f64>, C: Capacitor<T> {
            fn borrow(&self) -> &T {
                &self.curr
            }
        }

        pub struct TimedSignal<T, C> where T: Into<f64> + From<f64>, C: Capacitor<T> {
            inner: Arc<RefCell<TimedInnerSignal<T, C>>>
        }

        impl<T, C> Clone for TimedSignal<T, C> where T: Into<f64> + From<f64>, C: Capacitor<T> {
            fn clone(&self) -> Self {
                TimedSignal {
                    inner: self.inner.clone()
                }
            }
        }

        impl<T, C> TimedSignal<T, C> where T: Into<f64> + From<f64>, C: Capacitor<T> {
            fn clock() -> TimedSignal<T, C> {
                timer_subscriber(Box::new(|| {

                    true
                }));
            }
        }
    }
    pub use timed_signal::*;
     */
}
pub use signal::*;

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};
    use crate::core::{slock};
    use crate::state::{Store, Signal, TokenStore, Binding, Bindable, ActionDispatcher, StoreContainer, NumericAction, DirectlyInvertible, Filterable, DerivedStore, Stateful, CoupledStore, StringActionBasis};
    use crate::state::coupler::{FilterlessCoupler, NumericStringCoupler};
    use crate::state::SetAction::{Identity, Set};

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

        c.apply(Identity *
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
        let counts: [DerivedStore<usize>; 10] = Default::default();
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

    #[test]
    fn test_action_listener() {
        let s = slock();
        let state = Store::new(0);
        // these are technically not "true" derived stores
        // but the restrictions are somewhat loose
        // we are just using them for testing purposes
        // it may happen that in the future, we will have to ArcMutex
        // instead of this hack
        let identity_counter = DerivedStore::new(0);
        let set_counter = DerivedStore::new(0);
        let scb = set_counter.binding();
        let icb = identity_counter.binding();
        state.action_listener( move |_, action, s| {
            let Identity = action else {
                return true
            };
            let old = *icb.borrow(s);
            if old == 5 {
                // stop caring about events
                return false
            }
            icb.apply(NumericAction::Incr(1), s);
            true
        }, &s);
        state.action_listener( move |_, action, s| {
            let Set(_) = action else {
                return true
            };
            scb.apply(NumericAction::Incr(1), s);
            true
        }, &s);
        for i in 0 .. 100 {
            assert_eq!(*set_counter.borrow(&s), i);
            assert_eq!(*identity_counter.borrow(&s), std::cmp::min(i, 5));
            state.apply(Identity, &s);
            state.apply(NumericAction::Incr(1), &s);
        }
    }

    #[test]
    fn test_action_filter() {
        let s = slock();
        let state = Store::new_with_filter(0);
        state.action_filter(|curr, action, _s| {
            match action {
                Identity => Set(*curr + 1),
                Set(_) => Identity
            }
        }, &s);
        state.apply(Set(1), &s);
        assert_eq!(*state.borrow(&s), 0);
        state.apply(Identity, &s);
        state.apply(Identity, &s);
        assert_eq!(*state.borrow(&s), 2);
    }

    #[test]
    fn test_inverse_listener() {
        let s = slock();
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
        }, &s);
        for i in 0.. 100 {
            state.apply(Set(i * i), &s);
        }
        let mut l = vectors.lock().unwrap();
        assert_eq!(l.as_ref().unwrap().len(), 100);
        l.as_mut().unwrap().reverse();
        let res = l.take().unwrap().into_iter().enumerate();
        drop(l);
        for (i, mut item) in res.take(90) {
            assert_eq!(*state.borrow(&s), (99 - i) * (99 - i));
            unsafe {
                item.invert(&s);
            }
        }
    }

    #[test]
    fn test_inverse_listener_combine() {
        let s = slock();
        let state = Store::new(0);
        let vec: Option<Box<dyn DirectlyInvertible>> = None;
        let vectors = Arc::new(Mutex::new(Some(vec)));
        let c = vectors.clone();
        state.subtree_inverse_listener(move |inv, _s| {
            let mut l1 = c.lock().unwrap();
            let Some(l) = l1.as_mut() else {
                return false;
            };
            if l.is_none() {
                *l = Some(inv);
            }
            else {
                unsafe {
                    l.as_mut().unwrap().right_multiply(inv);
                }
            }
            true
        }, &s);
        for i in 0.. 100 {
            state.apply(Set(i * i), &s);
        }
        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        unsafe {
            res.invert(&s);
        }
        assert_eq!(*state.borrow(&s), 0);
    }

    #[test]
    fn test_general_listener() {
        let s = slock();
        let state = Store::new(0);
        let set_counter = DerivedStore::new(0);
        let scb = set_counter.binding();
        state.subtree_general_listener(move |s| {
            scb.apply(NumericAction::Incr(1), s);
            *scb.borrow(s) < 63
        }, &s);

        for i in 0 .. 100 {
            assert_eq!(*set_counter.borrow(&s), std::cmp::min(i, 63));
            state.apply(Identity, &s);
        }
    }

    #[test]
    #[should_panic]
    fn test_not_marked_derived_causes_panic() {
        let s = slock();
        let state1 = Store::new(0);
        let state2 = Store::new(1);
        let b = state2.binding();
        state1.action_listener(move |_, _a, s| {
            b.apply(Set(1), s);
            true
        }, &s);
        state1.apply(Set(0), &s);
    }

    #[test]
    #[should_panic]
    fn test_falsely_marked_derived_causes_panic() {
        let s = slock();
        let state = DerivedStore::new(0);
        state.apply(Set(1), &s);
    }

    struct NegatedCoupler {

    }

    impl FilterlessCoupler for NegatedCoupler {
        type Intrinsic = f32;
        type Mapped = f32;

        fn initial_mapped(&self, initial_intrinsic: &Self::Intrinsic) -> Self::Mapped {
            -*initial_intrinsic
        }

        fn mirror_intrinsic_to_mapped(&self, _mapped: &Self::Mapped, _intrinsic: &Self::Intrinsic, intrinsic_transaction: &<Self::Intrinsic as Stateful>::Action) -> <Self::Mapped as Stateful>::Action {
            match *intrinsic_transaction {
                Set(s) => Set(-s),
                Identity => Identity
            }
        }

        fn mirror_mapped_to_intrinsic(&self, _mapped: &Self::Mapped, _intrinsic: &Self::Intrinsic, mapped_transaction: &<Self::Mapped as Stateful>::Action) -> <Self::Intrinsic as Stateful>::Action {
            match *mapped_transaction {
                Set(s) => Set(-s),
                Identity => Identity
            }
        }
    }

    #[test]
    fn test_negated_coupler() {
        let s = slock();
        let intrinsic = Store::new(-1.0);
        let coupled = CoupledStore::new(intrinsic.binding(), NegatedCoupler {}, &s);

        assert_eq!(*coupled.borrow(&s), 1.0);

        coupled.apply(Set(-3.0), &s);

        assert_eq!(*coupled.borrow(&s), -3.0);
        assert_eq!(*intrinsic.borrow(&s), 3.0);

        intrinsic.apply(Set(2.0), &s);

        assert_eq!(*coupled.borrow(&s), -2.0);
    }

    #[test]
    fn test_string_number_coupler() {
        let s = slock();
        let intrinsic = Store::new(1);
        let mapped = CoupledStore::new(intrinsic.binding(), NumericStringCoupler::new(), &s);

        assert_eq!(*mapped.borrow(&s), "1");
        intrinsic.apply(NumericAction::Incr(5), &s);

        assert_eq!(*mapped.borrow(&s), "6");

        intrinsic.apply(NumericAction::Decr(10), &s);

        assert_eq!(*mapped.borrow(&s), "-4");

        mapped.apply(StringActionBasis::ReplaceSubrange(0..1, "1".to_string()), &s);

        assert_eq!(*mapped.borrow(&s), "14".to_string());
        assert_eq!(*intrinsic.borrow(&s), 14);

        mapped.apply(StringActionBasis::ReplaceSubrange(0..1, "a".to_string()), &s);

        assert_eq!(*mapped.borrow(&s), "14".to_string());
        assert_eq!(*intrinsic.borrow(&s), 14);

        mapped.apply(StringActionBasis::ReplaceSubrange(0..2, "-11231".to_string()), &s);
        assert_eq!(*mapped.borrow(&s), "-11231".to_string());
        assert_eq!(*intrinsic.borrow(&s), -11231);

        drop(intrinsic);

        mapped.apply(StringActionBasis::ReplaceSubrange(0..1, "+".to_string()), &s);

        assert_eq!(*mapped.borrow(&s), "+11231");
    }

    #[test]
    #[should_panic]
    fn test_faulty_coupler_access() {
        let s = slock();
        let intrinsic = Store::new(0.0);
        let random = Store::new(0.0);
        let coupler = CoupledStore::new(intrinsic.binding(), NegatedCoupler {}, &s);
        random.listen(move |_n, s| {
            coupler.apply(Set(-1.0), s);

            true
        }, &s);
        random.apply(Set(-3.0), &s);
    }

    #[test]
    #[should_panic]
    fn test_faulty_coupler_dispatch() {
        let s = slock();
        let intrinsic = Store::new(0.0);
        let random = Store::new(0.0);
        let coupler = CoupledStore::new(intrinsic.binding(), NegatedCoupler {}, &s);
        coupler.listen(move |_n, s| {
            random.apply(Set(-1.0), s);

            true
        }, &s);
        coupler.apply(Set(-3.0), &s);
    }
}