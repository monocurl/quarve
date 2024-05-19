mod listener {
    use crate::core::Slock;
    use crate::state::Stateful;

    pub(super) mod sealed {
        use crate::core::Slock;
        use crate::state::listener::DirectlyInvertible;

        // I don't like this
        // Maybe it can be done with dyn Any in a better
        // fashion? I tried but it seems that right_multiply
        // is hard to take out the action, which may result in
        // requiring stateful to be clone, which is probably a no go
        pub(in crate::state) trait DirectlyInvertibleBase {
            // This function must only be called once per instance
            // (We cannot take ownership since the caller is often unsized)
            fn invert(&mut self, s: &Slock);

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
    pub trait GeneralListener : FnMut(&Slock) -> bool + Send + 'static {}
    pub trait InverseListener : FnMut(Box<dyn DirectlyInvertible>, &Slock) -> bool + Send + 'static {}
    impl<T> GeneralListener for T where T: FnMut(&Slock) -> bool + Send + 'static {}
    impl<T> InverseListener for T where T: FnMut(Box<dyn DirectlyInvertible>, &Slock) -> bool + Send + 'static {}

    pub(super) type BoxInverseListener = Box<
        dyn FnMut(Box<dyn DirectlyInvertible>, &Slock) -> bool + Send
    >;

    pub(super) enum StateListener<S: Stateful> {
        ActionListener(Box<dyn (FnMut(&S, &S::Action, &Slock) -> bool) + Send>),
        SignalListener(Box<dyn (FnMut(&S, &Slock) -> bool) + Send>),
        GeneralListener(Box<dyn FnMut(&Slock) -> bool + Send>),
    }
}
pub use listener::*;

mod group {
    use std::ops::Mul;
    use crate::state::{GeneralListener, InverseListener};
    use crate::core::{Slock};
    use crate::util::markers::{BoolMarker, ThreadMarker};


    pub trait Stateful: Send + Sized + 'static {
        type Action: GroupAction<Self>;
        type HasInnerStores: BoolMarker;

        // This method should return an action listener
        // to be applied on the surrounding container
        // (if it wants)
        fn subtree_general_listener<F>(&self, _f: F, _s: &Slock<impl ThreadMarker>)
            -> Option<impl Send + FnMut(&Self, &Self::Action, &Slock) -> bool + 'static>
            where F: GeneralListener + Clone {
            None::<fn(&Self, &Self::Action, &Slock) -> bool>
        }

        // Returns an action listener to be applied on the parent container
        // (if necessary)
        fn subtree_inverse_listener<F>(&self, _f: F, _s: &Slock<impl ThreadMarker>)
            -> Option<impl Send + FnMut(&Self, &Self::Action, &Slock) -> bool + 'static>
            where F: InverseListener + Clone {
            None::<fn(&Self, &Self::Action, &Slock) -> bool>
        }
    }

    pub trait GroupBasis<T>: Send + Sized + 'static {
        // returns inverse action
        fn apply(self, to: &mut T) -> Self;
    }

    pub trait GroupAction<T>: GroupBasis<T> + Mul<Output=Self>
        where T: Stateful {

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
                let build = bases.into_iter()
                    .rev()
                    .map(|b| b.apply(to))
                    .collect::<Vec<_>>();

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
        use crate::core::{Slock};
        use crate::state::{Stateful};
        use crate::util::markers::ThreadMarker;

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
            use crate::util::markers::FalseMarker;

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
                bool,
                i8, u8,
                i16, u16,
                i32, u32,
                i64, u64,
                isize, usize,
                f32, f64
            );
        }
        pub use set_action::*;

        mod string_action {
            use std::ops::Range;
            use crate::state::{GroupBasis,  Stateful, Word};
            use crate::util::markers::FalseMarker;

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

            impl Stateful for String {
                type Action = Word<String, StringActionBasis>;
                type HasInnerStores = FalseMarker;
            }
        }
        pub use string_action::*;

        mod vec_action {
            use crate::core::{Slock};
            use crate::state::{GeneralListener, GroupBasis, InverseListener, Stateful, StoreContainer, Word};
            use crate::util::markers::{ThreadMarker, TrueMarker};

            #[derive(Clone)]
            pub enum VecActionBasis<T> {
                /* indices */
                Insert(T, usize),
                Remove(usize),
                Swap(usize, usize)
            }

            impl<T> GroupBasis<Vec<T>> for VecActionBasis<T>
                where T: StoreContainer
            {
                fn apply(self, to: &mut Vec<T>) -> Self {
                    match self {
                        VecActionBasis::Insert(elem, at) => {
                            to.insert(at, elem);
                            VecActionBasis::Remove(at)
                        },
                        VecActionBasis::Remove(at) => {
                            let removed = to.remove(at);
                            VecActionBasis::Insert(removed, at)
                        }
                        VecActionBasis::Swap(a, b) => {
                            to.swap(a, b);
                            VecActionBasis::Swap(a, b)
                        }
                    }
                }
            }

            /* the amount of stores can be variable so that we must add the listeners dynamically */
            /* in certain cases (for inverse listener), some listeners can be held on a bit longer than they ideally should be */
            /* but this is somewhat hard to avoid */
            impl<T> Stateful for Vec<T> where T: StoreContainer {
                type Action = Word<Vec<T>, VecActionBasis<T>>;
                type HasInnerStores = TrueMarker;

                fn subtree_general_listener<F>(&self, mut f: F, s: &Slock<impl ThreadMarker>)
                    -> Option<impl Send + FnMut(&Self, &Self::Action, &Slock) -> bool + 'static>
                    where F: GeneralListener + Clone {

                    for store in self {
                        store.subtree_general_listener(f.clone(), s);
                    }

                    Some(move |_v: &Vec<T>, w: &Word<Vec<T>, VecActionBasis<T>>, s: &Slock| {
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

                fn subtree_inverse_listener<F>(&self, f: F, s: &Slock<impl ThreadMarker>)
                    -> Option<impl Send + FnMut(&Self, &Self::Action, &Slock) -> bool + 'static>
                    where F: InverseListener + Clone {
                    for store in self {
                        store.subtree_inverse_listener(f.clone(), s);
                    }

                    Some(move |_v: &Vec<T>, w: &Word<Vec<T>, VecActionBasis<T>>, s: &Slock| {
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
            use crate::util::markers::FalseMarker;
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

pub mod coupler {
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

pub mod capacitor {
    use std::collections::VecDeque;
    use std::fmt::Debug;
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
        where T: Debug + Stateful + Lerp + Copy + Add<Output=T> + Sub<Output=T>,
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

            let cont = self.points.back().unwrap().0 + self.trans_time >= time;
            let mut val = self.points[0].1;
            for i in 0 .. self.points.len() - 1 {
                let diff = self.points[i + 1].1 - self.points[i].1;
                let alpha = (self.ease_function)((time - self.points[i + 1].0) / self.trans_time).min(1.0);

                val = T::lerp(val, alpha, val + diff)
            }

            (val, cont)
        }
    }
}

mod store {
    use crate::core::{Slock};
    use crate::state::{ActionFilter, Filterless, IntoAction, Signal, Stateful};
    use crate::state::listener::{GeneralListener, InverseListener};
    use crate::util::markers::ThreadMarker;

    /// It is the implementors job to guarantee that subtree_listener
    /// and relatives do not get into call cycles
    pub trait StoreContainer: Send + Sized + 'static {
        // Only ONE general listener
        // can ever be present for a subtree
        fn subtree_general_listener<F: GeneralListener + Clone>(&self, f: F, s: &Slock<impl ThreadMarker>);

        // Only ONE active general listener
        // can ever be present for a subtree
        fn subtree_inverse_listener<F: InverseListener + Clone>(&self, f: F, s: &Slock<impl ThreadMarker>);
    }

    pub trait ActionDispatcher<S: Stateful, F: ActionFilter<Target=S>> {
        fn action_listener<G>(&self, listener: G, s: &Slock<impl ThreadMarker>)
            where G: Send + Fn(&S, &S::Action, &Slock) -> bool + 'static;
    }

    pub trait Filterable<S: Stateful> {
        fn action_filter<G>(&self, filter: G, s: &Slock<impl ThreadMarker>)
            where G: Send + Fn(&S, S::Action, &Slock) -> S::Action + 'static;
    }

    pub trait Bindable<S: Stateful, F: ActionFilter<Target=S>> {
        type Binding: Binding<S, F> + Clone;

        fn binding(&self) -> Self::Binding;
        fn signal(&self) -> impl Signal<S> + Clone;
    }

    // Like with signal, I believe it makes more sense for
    // S to be an associated type, but then we can't have default
    // filterless? So, it is done for consistency as a generic parameter
    pub trait Binding<S: Stateful, F: ActionFilter<Target=S>=Filterless<S>>: ActionDispatcher<S, F> + Signal<S> {
        fn apply(&self, action: impl IntoAction<S::Action, S>, s: &Slock);
    }

    mod sealed_base {
        use std::cell::RefCell;
        use std::sync::Arc;
        use crate::core::{Slock};
        use crate::state::{Signal};
        use crate::util::markers::ThreadMarker;
        use super::{ActionFilter, IntoAction, Stateful};

        pub(super) trait RawStoreBase<S, F>: 'static where S: Stateful, F: ActionFilter<Target=S> {
            type InverseListenerHolder: super::inverse_listener_holder::InverseListenerHolder;

            fn dispatcher(&self) -> &super::store_dispatcher::StoreDispatcher<S, F, Self::InverseListenerHolder>;

            fn dispatcher_mut(&mut self) -> &mut super::store_dispatcher::StoreDispatcher<S, F, Self::InverseListenerHolder>;

            // may introduce some additional behavior that the dispatcher does not handle
            fn apply(inner: &Arc<RefCell<Self>>, action: impl IntoAction<S::Action, S>, skip_filters: bool, s: &Slock<impl ThreadMarker>,);

            // Must be careful with these two methods
            // since generally not called with the state lock
            fn strong_count_decrement(&mut self) {

            }

            fn strong_count_increment(&mut self) {

            }
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
            fn arc_clone(&self) -> Self;
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
        use std::marker::PhantomData;
        use crate::core::{Slock};
        use crate::state::{ActionDispatcher, ActionFilter, Bindable, Binding, Filter, Filterable, GeneralBinding, IntoAction, Signal, Stateful};
        use crate::state::listener::StateListener;
        use crate::util::markers::ThreadMarker;
        use super::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        #[allow(private_bounds)]
        pub trait RawStoreSharedOwner<S, F>: RawStoreSharedOwnerBase<S, F>
            where S: Stateful, F: ActionFilter<Target=S> {}

        impl<S, F, R> RawStoreSharedOwner<S, F> for R
            where S: Stateful, F: ActionFilter<Target=S>, R: RawStoreSharedOwnerBase<S, F> {

        }

        impl<S, F, I> ActionDispatcher<S, F> for I
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F> {
            fn action_listener<G>(&self, listener: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + FnMut(&S, &S::Action, &Slock) -> bool + 'static {
                self.get_ref().borrow_mut().dispatcher_mut().add_listener(StateListener::ActionListener(Box::new(listener)));
            }
        }

        impl<S: Stateful, I: RawStoreSharedOwner<S, Filter<S>>> Filterable<S> for I {
            fn action_filter<G>(&self, filter: G, s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, &Slock) -> S::Action + 'static {
                self.get_ref().borrow_mut().dispatcher_mut().action_filter(filter, s);
            }
        }

        impl<S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F>> Bindable<S, F> for I {
            type Binding = GeneralBinding<S, F, I>;

            fn binding(&self) -> Self::Binding {
                self.get_ref().borrow_mut().strong_count_increment();

                GeneralBinding {
                    inner: self.arc_clone(),
                    phantom_state: PhantomData,
                    phantom_filter: PhantomData
                }
            }

            fn signal(&self) -> impl Signal<S> + Clone {
                self.binding()
            }
        }

        // Unfortunately can't do this for signal as well
        // Since FixedSignal 'might' implement RawStoreSharedOwnerBase
        // It's therefore done as macros
        impl<S, F, R> Binding<S, F> for R where
            S: Stateful, F: ActionFilter<Target=S>, R: RawStoreSharedOwnerBase<S, F> {
            fn apply(&self, action: impl IntoAction<S::Action, S>, s: &Slock) {
                R::Inner::apply(self.get_ref(), action, false, s);
            }
        }
    }
    pub use raw_store_shared_owner::*;

    /* MARK: utilities */
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
            fn invert(&mut self, s: &Slock) {
                let Some(state) = self.state.upgrade() else {
                    return;
                };

                // skip filters on inversion to avoid weird behavior
                I::apply(&state, self.action.take().unwrap(), true, s);
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

        // safety: all operations are either unsafe or require the s
        unsafe impl<S, F, I> Send for ActionInverter<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStore<S, F> {
        }
    }

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

            fn invoke_listener(&mut self, action: impl FnOnce() -> Box<dyn DirectlyInvertible>, s: &Slock);
        }

        pub(super) struct NullInverseListenerHolder;

        impl InverseListenerHolder for NullInverseListenerHolder {
            fn new() -> Self {
                NullInverseListenerHolder
            }

            fn set_listener(&mut self, _listener: BoxInverseListener) {

            }

            fn invoke_listener(&mut self, _action: impl FnOnce() -> Box<dyn DirectlyInvertible>, _s: &Slock) {

            }
        }

        pub(super) struct ActualInverseListenerHolder(Option<BoxInverseListener>);

        impl InverseListenerHolder for ActualInverseListenerHolder {
            fn new() -> Self {
                ActualInverseListenerHolder(None)
            }

            fn set_listener(&mut self, listener: BoxInverseListener) {
                self.0 = Some(listener);
            }

            fn invoke_listener(&mut self, action: impl FnOnce() -> Box<dyn DirectlyInvertible>, s: &Slock) {
                if let Some(ref mut func) = self.0 {
                    if !(func)(action(), s) {
                        self.0 = None;
                    }
                }
            }
        }
    }

    mod store_dispatcher {
        use crate::core::Slock;
        use crate::state::{ActionFilter, DirectlyInvertible, GeneralListener, GroupBasis, IntoAction, InverseListener, Stateful};
        use crate::state::listener::{ StateListener};
        use crate::state::store::inverse_listener_holder::InverseListenerHolder;
        use crate::util::markers::ThreadMarker;
        use crate::util::test_util::QuarveAllocTag;

        pub(crate) struct StoreDispatcher<S, F, I>
            where S: Stateful, F: ActionFilter, I: InverseListenerHolder
        {
            _quarve_tag: QuarveAllocTag,
            data: S,
            listeners: Vec<StateListener<S>>,
            inverse_listener: I,
            filter: F,
        }

        impl<S, F, I> StoreDispatcher<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: InverseListenerHolder {

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
                s: &Slock<impl ThreadMarker>
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
                        StateListener::ActionListener(listener) => listener(&self.data, &filtered_action, s.as_ref()),
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
                            action(s.as_ref())
                        },
                        StateListener::SignalListener(action) => {
                            action(data, s.as_ref())
                        },
                        _ => true
                    }
                );

                // tell inverse listener
                self.inverse_listener.invoke_listener(move || make_inverter(inverse), s.as_ref());
            }

            pub fn apply(
                &mut self,
                into_action: impl IntoAction<S::Action, S>,
                make_inverter: impl FnOnce(S::Action) -> Box<dyn DirectlyInvertible>,
                skip_filters: bool,
                s: &Slock<impl ThreadMarker>
            ) {
                self.apply_post_filter(into_action, make_inverter, |_, f| f, skip_filters, s);
            }

            pub fn add_listener(&mut self, listener: StateListener<S>) {
                debug_assert!(! matches!(listener, StateListener::GeneralListener(_)),
                              "Should be set via set_general_listener"
                );
                self.listeners.push(listener);
            }

            pub fn action_filter<G>(&mut self, filter: G, _s: &Slock<impl ThreadMarker>)
                where G: Send + Fn(&S, S::Action, &Slock) -> S::Action + 'static {
                self.filter.add_filter(filter);
            }

            pub fn set_general_listener(&mut self, f: impl GeneralListener + Clone, s: &Slock) {
                self.listeners.retain(|x| !matches!(x, StateListener::GeneralListener(_)));
                self.listeners.push(StateListener::GeneralListener(Box::new(f.clone())));

                if let Some(action) = self.data.subtree_general_listener(f, s) {
                    self.listeners.push(StateListener::ActionListener(Box::new(action)));
                }
            }

            pub fn set_inverse_listener(&mut self, f: impl InverseListener + Clone, s: &Slock) {
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
                fn subtree_general_listener<Q>(&self, f: Q, s: &Slock<impl ThreadMarker>)
                    where Q: GeneralListener + Clone {
                    self.inner.borrow_mut().dispatcher_mut().set_general_listener(f, s.as_ref());
                }

                fn subtree_inverse_listener<Q>(&self, f: Q, s: &Slock<impl ThreadMarker>)
                    where Q: InverseListener + Clone {
                    self.inner.borrow_mut().dispatcher_mut().set_inverse_listener(f, s.as_ref());
                }
            }
        }

        macro_rules! impl_signal_inner {
            ($s:ty) => {
                fn borrow<'a>(&'a self, _s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=$s> {
                    StateRef {
                        main_ref: self.get_ref().borrow(),
                        lifetime: PhantomData,
                        filter: PhantomData,
                    }
                }

                fn listen<Q>(&self, listener: Q, _s: &Slock<impl ThreadMarker>)
                    where Q: FnMut(&$s, &Slock) -> bool + Send + 'static {
                    self.get_ref().borrow_mut().dispatcher_mut().add_listener(StateListener::SignalListener(Box::new(listener)));
                }

                type MappedOutput<U: Send + 'static> = GeneralSignal<U>;
                fn map<U, Q>(&self, map: Q, s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<U>
                    where U: Send + 'static, Q: Send + 'static + Fn(&$s) -> U {
                    GeneralSignal::from(self, map, |this, listener, _s| {
                        this.get_ref().borrow_mut().dispatcher_mut().add_listener(StateListener::SignalListener(listener))
                    }, s)
                }
            };
        }

        pub(super) use {impl_store_container_inner, impl_signal_inner};
    }

    /* MARK: Stores */
    mod store {
        use std::cell::RefCell;
        use std::marker::PhantomData;
        use std::ops::{Deref, DerefMut};
        use std::sync::Arc;
        use crate::{
            state::{ActionFilter, Filterless, IntoAction, Signal, Stateful, StoreContainer, GeneralSignal},
            core::Slock,
        };
        use crate::state::StateListener;
        use crate::state::store::state_ref::StateRef;
        use crate::state::{Filter};
        use crate::state::listener::{GeneralListener, InverseListener};
        use crate::state::store::action_inverter::ActionInverter;
        use crate::state::store::inverse_listener_holder::ActualInverseListenerHolder;
        use crate::state::store::macros::{impl_signal_inner, impl_store_container_inner};
        use crate::state::store::store_dispatcher::StoreDispatcher;
        use crate::util::markers::ThreadMarker;
        use super::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        pub(super) struct InnerStore<S: Stateful, F: ActionFilter<Target=S>> {
            dispatcher: StoreDispatcher<S, F, ActualInverseListenerHolder>
        }

        impl<S, F> RawStoreBase<S, F> for InnerStore<S, F>
            where S: Stateful, F: ActionFilter<Target=S>
        {
            type InverseListenerHolder = ActualInverseListenerHolder;

            fn dispatcher(&self) -> &StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &self.dispatcher
            }

            fn dispatcher_mut(&mut self) -> &mut StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &mut self.dispatcher
            }

            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<S::Action, S>, skip_filters: bool, s: &Slock<impl ThreadMarker>) {
                #[cfg(debug_assertions)] {
                    debug_assert_eq!(s.debug_info.applying_transaction.borrow().len(), 0, "Fatal: store \
                    changed as a result of the change of another state variable. \
                    Stores, by default, are to be independent of other stores. If you would like one store to \
                    be dependent on another, check out DerivedStore (or in some circumstances, maybe CoupledStore)");
                    s.debug_info.applying_transaction.borrow_mut().push(arc.as_ptr() as usize);
                }
                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();

                inner.dispatcher.apply(alt_action, |action| {
                    Box::new(ActionInverter {
                        action: Some(action),
                        state: Arc::downgrade(arc),
                        filter: PhantomData,
                    })
                }, skip_filters, s);

                #[cfg(debug_assertions)]
                {
                    s.debug_info.applying_transaction.borrow_mut().pop();
                }
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
                    inner: Arc::new(RefCell::new(InnerStore {
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
            where S: Stateful, M: ActionFilter<Target=S>
        {
            impl_store_container_inner!();
        }

        impl<S, A> Signal<S> for Store<S, A>
            where S: Stateful, A: ActionFilter<Target=S>
        {
            impl_signal_inner!(S);
        }

        impl<S, F> RawStoreSharedOwnerBase<S, F> for Store<S, F>
            where S: Stateful, F: ActionFilter<Target=S>
        {
            type Inner = InnerStore<S, F>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }

            fn arc_clone(&self) -> Self {
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
        use crate::state::StateListener;
        use crate::state::store::state_ref::StateRef;
        use crate::core::{Slock};
        use crate::state::{ActionFilter, Filter, Filterless, GeneralListener, GeneralSignal, IntoAction, Signal, Stateful, StoreContainer};
        use crate::state::store::action_inverter::ActionInverter;
        use crate::state::store::inverse_listener_holder::ActualInverseListenerHolder;
        use crate::state::store::macros::{impl_signal_inner, impl_store_container_inner};
        use crate::state::store::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};
        use crate::state::store::store_dispatcher::StoreDispatcher;
        use crate::state::InverseListener;
        use crate::util::markers::ThreadMarker;

        pub(super) struct InnerTokenStore<S: Stateful + Copy + Hash + Eq, F: ActionFilter<Target=S>> {
            dispatcher: StoreDispatcher<S, F, ActualInverseListenerHolder>,
            equal_listeners: HashMap<S, Vec<Box<dyn FnMut(&S, &Slock) -> bool + Send>>>,
        }
        impl<S, F> RawStoreBase<S, F> for InnerTokenStore<S, F>
            where S: Stateful + Copy + Hash + Eq,
                  F: ActionFilter<Target=S>
        {
            type InverseListenerHolder = ActualInverseListenerHolder;

            fn dispatcher(&self) -> &StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &self.dispatcher
            }

            fn dispatcher_mut(&mut self) -> &mut StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &mut self.dispatcher
            }

            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<S::Action, S>, skip_filters: bool, s: &Slock<impl ThreadMarker>) {
                #[cfg(debug_assertions)] {
                    debug_assert_eq!(s.debug_info.applying_transaction.borrow().len(), 0, "Fatal: token store \
                    changed as a result of the change of another state variable. \
                    Stores, by default, are to be independent of other stores. If you would like one store to \
                    be dependent on another, check out DerivedStore (or in some circumstances, maybe CoupledStore)");
                    s.debug_info.applying_transaction.borrow_mut().push(arc.as_ptr() as usize);
                }
                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();

                let old = *inner.dispatcher.data();

                inner.dispatcher.apply(alt_action, |action| {
                    Box::new(ActionInverter {
                        action: Some(action),
                        state: Arc::downgrade(&arc),
                        filter: PhantomData
                    })
                }, skip_filters, s);

                // relevant equal listeners (old and new)
                let new = *inner.dispatcher.data();

                if old != new {
                    for class in [old, new] {
                        inner.equal_listeners.entry(class)
                            .and_modify(|l|
                                l.retain_mut(|f| f(&new, s.as_ref()))
                            );
                    }
                }


                #[cfg(debug_assertions)]
                {
                    s.debug_info.applying_transaction.borrow_mut().pop();
                }
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
                        dispatcher: StoreDispatcher::new(initial),
                        equal_listeners: HashMap::new(),
                    }))
                }
            }
        }

        impl<S> TokenStore<S, Filter<S>> where S: Stateful + Copy + Hash + Eq {
            pub fn new_with_filter(initial: S) -> Self {
                TokenStore {
                    inner: Arc::new(RefCell::new(InnerTokenStore {
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
            where S: Stateful + Copy + Hash + Eq, M: ActionFilter<Target=S> {
            impl_store_container_inner!();
        }

        impl<S, A> Signal<S> for TokenStore<S, A>
            where S: Stateful + Copy + Hash + Eq, A: ActionFilter<Target=S> {
            impl_signal_inner!(S);
        }

        impl<S, A> RawStoreSharedOwnerBase<S, A> for TokenStore<S, A>
            where S: Stateful + Copy + Hash + Eq, A: ActionFilter<Target=S> {
            type Inner = InnerTokenStore<S, A>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }

            fn arc_clone(&self) -> Self {
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
        use std::marker::PhantomData;
        use crate::state::store::state_ref::StateRef;
        use crate::{
            state::{
                ActionFilter, Filterless, IntoAction, Signal, Stateful, StoreContainer, GeneralSignal,
            },
            core::Slock,
        };
        use crate::state::StateListener;
        use crate::state::{Filter};
        use crate::state::listener::{GeneralListener, InverseListener};
        use crate::state::store::inverse_listener_holder::NullInverseListenerHolder;
        use crate::state::store::macros::{impl_signal_inner, impl_store_container_inner};
        use crate::state::store::store_dispatcher::StoreDispatcher;
        use crate::util::markers::ThreadMarker;
        use super::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        pub(super) struct InnerDerivedStore<S: Stateful, F: ActionFilter<Target=S>> {
            dispatcher: StoreDispatcher<S, F, NullInverseListenerHolder>
        }

        impl<S, F> RawStoreBase<S, F> for InnerDerivedStore<S, F>
            where S: Stateful, F: ActionFilter<Target=S>
        {
            type InverseListenerHolder = NullInverseListenerHolder;

            fn dispatcher(&self) -> &StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &self.dispatcher
            }

            fn dispatcher_mut(&mut self) -> &mut StoreDispatcher<S, F, Self::InverseListenerHolder> {
                &mut self.dispatcher
            }

            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<S::Action, S>, skip_filters: bool, s: &Slock<impl ThreadMarker>) {
                #[cfg(debug_assertions)] {
                    debug_assert_ne!(s.debug_info.applying_transaction.borrow().len(), 0, "Fatal: derived store \
                    changed in a context that was NOT initiated by the change of another store. \
                    Derived store, being 'derived', must only be a function of other state variables. ");
                    s.debug_info.applying_transaction.borrow_mut().push(arc.as_ptr() as usize);
                }

                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();

                inner.dispatcher.apply(alt_action, |_| unreachable!(), skip_filters, s);

                #[cfg(debug_assertions)]
                {
                    s.debug_info.applying_transaction.borrow_mut().pop();
                }
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
                    inner: Arc::new(RefCell::new(InnerDerivedStore {
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
            where S: Stateful, M: ActionFilter<Target=S>
        {
            impl_store_container_inner!();
        }

        impl<S, A> Signal<S> for DerivedStore<S, A>
            where S: Stateful, A: ActionFilter<Target=S>
        {
            impl_signal_inner!(S);
        }

        impl<S, F> RawStoreSharedOwnerBase<S, F> for DerivedStore<S, F>
            where S: Stateful, F: ActionFilter<Target=S>
        {
            type Inner = InnerDerivedStore<S, F>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }

            fn arc_clone(&self) -> Self {
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
        use std::sync::{Arc, Mutex};
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering::Release;
        use crate::state::StateListener;
        use crate::state::store::state_ref::StateRef;
        use crate::{
            state::{
                ActionFilter, Filterless, IntoAction, Signal, Stateful, StoreContainer, GeneralSignal,
            },
            core::Slock,
        };
        use crate::state::{Binding};
        use crate::state::coupler::Coupler;
        use crate::state::listener::{GeneralListener, InverseListener};
        use crate::state::store::inverse_listener_holder::NullInverseListenerHolder;
        use crate::state::store::macros::{impl_signal_inner, impl_store_container_inner};
        use crate::state::store::store_dispatcher::StoreDispatcher;
        use crate::util::{UnsafeForceSend};
        use crate::util::markers::ThreadMarker;
        use super::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

        // note that IB is purposefully filterless
        // The reference counting of this is particularly tricky
        // But the premise is that both each other until the couple
        // has a ref count of 1, at which point the couple removes ownership
        // of the intrinsic, to avoid cycles
        pub(super) struct InnerCoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I, Filterless<I>>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F> {
            dispatcher: StoreDispatcher<M, F, NullInverseListenerHolder>,
            coupler: C,
            // intrinsic maintains a weak ownership of us
            // we maintain strong ownership of intrinsic
            // this may seem a bit backwards, but if you think about it
            // it's okay if intrinsic outlives us, but not ok if
            // we outlive intrinsic
            phantom_intrinsic: PhantomData<&'static I>,
            // set to None once we have a ref count of 1
            // therefore, we need a mutex since we may not have state lock
            intrinsic: Mutex<Option<IB>>,
            intrinsic_performing_transaction: bool,
            performing_transaction: bool,
            strong_count: AtomicUsize,
        }

        impl<I, IB, M, F, C> InnerCoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            // logic for this is somewhat convoluted
            // in large part due to the awkwardness of this
            // and the borrow rules
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
                let mut borrow = arc.borrow_mut();
                let inner = borrow.deref_mut();

                /* this is a bit awkward, but it's easiest way to get around borrowing errors */
                let mut intrinsic_action = None;

                inner.dispatcher.apply_post_filter(alt_action, |_| unreachable!(), |data, action| {
                    let (intrinsic_transaction, action) = {
                        if inner.intrinsic_performing_transaction {
                            inner.coupler.filter_mapped_and_mirror_to_intrinsic(
                                data,
                                intrinsic.unwrap(),
                                action
                            )
                        } else {
                            // if we are originating the transaction
                            // then surely the strong count > 1 so intrinsic
                            // exists
                            inner.coupler.filter_mapped_and_mirror_to_intrinsic(
                                data,
                                inner.intrinsic.lock().unwrap()
                                    .as_ref().unwrap()
                                    .borrow(s).deref(),
                                action
                            )
                        }
                    };

                    intrinsic_action = Some(intrinsic_transaction);

                    action
                }, false, s);


                /* in this case, it's fine that it's being changed due to another store */
                // yeah the order of operations is a bit weird
                // i think it would be easier in oop
                // but not sure what's the rust way to do something like this?
                #[cfg(debug_assertions)]
                {
                    s.debug_info.applying_transaction.borrow_mut().pop();
                }

                // convert borrow to immutable
                drop(borrow);
                let borrow_immut = arc.borrow();
                let inner_immut = borrow_immut.deref();

                // tell intrinsic if it didn't originate (it's filterless so doesn't matter about filters)
                if !inner_immut.intrinsic_performing_transaction {

                    if let Some(intr_ref) = inner_immut.intrinsic.lock().unwrap().as_ref() {
                        intr_ref.apply(intrinsic_action.unwrap(), s.as_ref());
                    };
                }

                drop(borrow_immut);
                arc.borrow_mut().performing_transaction = false;
            }
        }

        impl<I, IB, M, F, C> RawStoreBase<M, F> for InnerCoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            type InverseListenerHolder = NullInverseListenerHolder;

            fn dispatcher(&self) -> &StoreDispatcher<M, F, Self::InverseListenerHolder> {
                &self.dispatcher
            }

            fn dispatcher_mut(&mut self) -> &mut StoreDispatcher<M, F, Self::InverseListenerHolder> {
                &mut self.dispatcher
            }

            fn apply(arc: &Arc<RefCell<Self>>, alt_action: impl IntoAction<M::Action, M>, _skip_filters: bool, s: &Slock<impl ThreadMarker>) {
                InnerCoupledStore::fully_apply(arc, None, alt_action, s);
            }

            fn strong_count_decrement(&mut self) {
                if self.strong_count.fetch_sub(1, Release) == 2 {
                    // in some testing code validating should_panic
                    // we want to avoid non-unwinding panic
                    // in production, this will not be an issue however
                    if let Ok(mut res) = self.intrinsic.lock() {
                        *res = None;
                    }
                }
            }

            fn strong_count_increment(&mut self) {
                self.strong_count.fetch_add(1, Release);
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
                        dispatcher: StoreDispatcher::new(data),
                        coupler,
                        phantom_intrinsic: PhantomData,
                        intrinsic: Mutex::new(Some(intrinsic)),
                        intrinsic_performing_transaction: false,
                        performing_transaction: false,
                        // one is the obvious one, other is the one owned by intrinsic
                        strong_count: AtomicUsize::new(2)
                    }))
                };

                // intrinsic listener
                // (our listener is handled manually in apply)
                let strong = UnsafeForceSend(ret.inner.clone());
                ret.inner.borrow()
                    .intrinsic.lock().unwrap()
                    .as_ref().unwrap()
                    .action_listener(move |intrinsic, a, s| {
                        let UnsafeForceSend(strong) = &strong;

                        let this = strong.borrow();
                        // if we didn't originate, then mirror the action
                        if !this.performing_transaction {
                            let coupler = &this.coupler;
                            let our_action = coupler.mirror_intrinsic_to_mapped(this.dispatcher.data(), intrinsic, a);

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

        impl<I, IB, M, F, C> Drop for CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F> {
            fn drop(&mut self) {
                self.inner.borrow_mut().strong_count_decrement();
            }
        }

        // In this case, inverse is a no op
        // since the respective action should've been handled
        // by the source store
        impl<I, IB, M, F, C> StoreContainer for CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            impl_store_container_inner!();
        }

        impl<I, IB, M, F, C> Signal<M> for CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            impl_signal_inner!(M);
        }

        impl<I, IB, M, F, C> RawStoreSharedOwnerBase<M, F> for CoupledStore<I, IB, M, F, C>
            where I: Stateful, IB: Binding<I>, M: Stateful, F: ActionFilter<Target=M>, C: Coupler<I, M, F>
        {
            type Inner = InnerCoupledStore<I, IB, M, F, C>;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                &self.inner
            }

            fn arc_clone(&self) -> Self {
                self.inner.borrow_mut().strong_count_increment();

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
        use crate::core::{Slock};
        use crate::state::StateListener;
        use crate::state::store::state_ref::StateRef;
        use crate::state::{RawStoreSharedOwner, Signal};
        use crate::state::signal::GeneralSignal;
        use crate::state::store::macros::impl_signal_inner;
        use crate::util::markers::ThreadMarker;
        use super::{ActionFilter, Stateful};
        use super::sealed_base::{RawStoreBase, RawStoreSharedOwnerBase};

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
                    inner: self.inner.arc_clone(),
                    phantom_state: PhantomData,
                    phantom_filter: PhantomData
                }
            }
        }

        impl<S, A, I> Signal<S> for GeneralBinding<S, A, I>
            where S: Stateful, A: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, A> {
            impl_signal_inner!(S);
        }

        impl<S, A, I> RawStoreSharedOwnerBase<S, A> for GeneralBinding<S, A, I>
            where S: Stateful, A: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, A> {
            type Inner = I::Inner;

            fn get_ref(&self) -> &Arc<RefCell<Self::Inner>> {
                self.inner.get_ref()
            }

            fn arc_clone(&self) -> Self {
                self.inner.get_ref().borrow_mut().strong_count_increment();

                GeneralBinding {
                    inner: self.inner.arc_clone(),
                    phantom_state: PhantomData,
                    phantom_filter: PhantomData
                }
            }
        }

        impl<S, A, I> Drop for GeneralBinding<S, A, I>
            where S: Stateful, A: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, A> {
            fn drop(&mut self) {
                self.inner.get_ref().borrow_mut().strong_count_decrement();
            }
        }

        // Safety: all operations require the state lock
        unsafe impl<S, F, I> Send for GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F> {}
        unsafe impl<S, F, I> Sync for GeneralBinding<S, F, I>
            where S: Stateful, F: ActionFilter<Target=S>, I: RawStoreSharedOwner<S, F> {}
    }
    pub use general_binding::*;
}
pub use store::*;

mod buffer {
    use std::cell::{Ref, RefCell, RefMut};
    use std::ops::DerefMut;
    use std::sync::Arc;
    use crate::core::Slock;
    use crate::util::markers::ThreadMarker;
    use crate::util::test_util::QuarveAllocTag;

    pub struct Buffer<T>(Arc<(RefCell<T>, QuarveAllocTag)>, ) where T: Send;

    impl<T> Buffer<T>
        where T: Send
    {
        pub fn new(initial: T) -> Buffer<T> {
            Buffer(Arc::new((RefCell::new(initial), QuarveAllocTag::new())))
        }

        pub fn borrow(&self, _s: &'_ Slock<impl ThreadMarker>) -> Ref<'_, T> {
            self.0.0.borrow()
        }

        pub fn borrow_mut(&self, _s: &'_ Slock<impl ThreadMarker>) -> RefMut<'_, T> {
            self.0.0.borrow_mut()
        }

        pub fn replace(&self, with: T, s: &'_ Slock<impl ThreadMarker>) -> T {
            std::mem::replace(self.borrow_mut(s).deref_mut(), with)
        }
    }

    impl<T> Clone for Buffer<T>
        where T: Send
    {
        fn clone(&self) -> Self {
            Buffer(Arc::clone(&self.0))
        }
    }

    // safety: accesses are done using the state lock
    // and T: Send
    unsafe impl<T> Send for Buffer<T> where T: Send {}
    unsafe impl<T> Sync for Buffer<T> where T: Send {}
}
pub use buffer::*;

mod signal {
    use std::ops::{Deref};
    use crate::core::{Slock};

    pub trait Signal<T: Send + 'static> : Sized + Send + Sync + 'static {
        fn borrow<'a>(&'a self, s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=T>;

        fn listen<F>(&self, listener: F, _s: &Slock<impl ThreadMarker>)
            where F: (FnMut(&T, &Slock) -> bool) + Send + 'static;

        type MappedOutput<S: Send + 'static>: Signal<S>;
        fn map<S, F>(&self, map: F, _s: &Slock<impl ThreadMarker>) -> Self::MappedOutput<S>
            where S: Send + 'static,
                  F: Send + 'static + Fn(&T) -> S;

        fn with_capacitor(&self, capacitor: impl Capacitor<Target=T>, s: &Slock) -> impl Signal<T> + Clone {
            CapacitatedSignal::from(self, capacitor, s)
        }
    }

    trait InnerSignal<T: Send> {
        fn borrow(&self) -> &T;
    }

    mod signal_audience {
        use crate::core::{Slock};
        use crate::util::markers::ThreadMarker;

        pub(super) struct SignalAudience<T: Send> {
            listeners: Vec<Box<dyn FnMut(&T, &Slock) -> bool + Send>>
        }

        impl<T> SignalAudience<T> where T: Send {
            pub(super) fn new() -> SignalAudience<T> {
                SignalAudience {
                    listeners: Vec::new()
                }
            }

            pub(super) fn listen<F>(&mut self, listener: F, _s: &Slock<impl ThreadMarker>)
                where F: (FnMut(&T, &Slock) -> bool) + Send + 'static {
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

            pub(super) fn is_empty(&self) -> bool {
                self.listeners.is_empty()
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
        use crate::core::{Slock};
        use crate::state::Signal;
        use crate::util::markers::ThreadMarker;
        use crate::util::test_util::QuarveAllocTag;
        use super::SignalRef;
        use super::InnerSignal;

        struct InnerFixedSignal<T: Send>(QuarveAllocTag, T);

        impl<T> InnerSignal<T> for InnerFixedSignal<T> where T: Send {
            fn borrow(&self) -> &T {
                &self.1
            }
        }

        pub struct FixedSignal<T: Send + 'static> {
            inner: Arc<RefCell<InnerFixedSignal<T>>>
        }

        impl<T> FixedSignal<T> where T: Send + 'static {
            pub fn new(val: T) -> FixedSignal<T> {
                FixedSignal {
                    inner: Arc::new(RefCell::new(InnerFixedSignal(QuarveAllocTag::new(), val)))
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

            fn listen<F>(&self, _listener: F, _s: &Slock<impl ThreadMarker>)
                where F: FnMut(&T, &Slock) -> bool + Send {
                /* no op */
            }

            type MappedOutput<S: Send + 'static> = FixedSignal<S>;
            fn map<S, F>(&self, map: F, _s: &Slock<impl ThreadMarker>) -> FixedSignal<S>
                where S: Send + 'static,
                      F: Send + 'static + Fn(&T) -> S
            {
                let inner = self.inner.borrow();
                let data = map(&inner.1);

                FixedSignal {
                    inner: Arc::new(RefCell::new(InnerFixedSignal(QuarveAllocTag::new(), data)))
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
        use crate::core::{Slock};
        use crate::state::Signal;
        use crate::util::markers::ThreadMarker;
        use crate::util::test_util::QuarveAllocTag;
        use super::SignalRef;
        use super::{InnerSignal, SignalAudience};

        struct GeneralInnerSignal<T: Send> {
            _quarve_tag: QuarveAllocTag,
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
                        _quarve_tag: QuarveAllocTag::new(),
                        val: map(&*val),
                        audience: SignalAudience::new(),
                    };
                }

                let arc = Arc::new(GeneralSyncCell(RefCell::new(inner)));
                let pseudo_weak = arc.clone();
                add_listener(source, Box::new(move |val, s| {
                    let mut binding = pseudo_weak.0.borrow_mut();
                    let inner = binding.deref_mut();

                    // no longer any point
                    inner.val = map(val);
                    inner.audience.dispatch(&inner.val, s);

                    !inner.audience.is_empty() || Arc::strong_count(&pseudo_weak) > 1
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
                GeneralSignal::from(self, map, |this, listener, s| {
                    this.inner.0.borrow_mut().audience.listen_box(listener, s);
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
        use std::sync::atomic::{AtomicU8};
        use std::sync::atomic::Ordering::{SeqCst};
        use crate::core::{Slock};
        use crate::state::{GeneralSignal, Signal};
        use crate::state::signal::InnerSignal;
        use crate::state::signal::signal_audience::SignalAudience;
        use crate::state::signal::signal_ref::SignalRef;
        use crate::util::markers::ThreadMarker;
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
            // how many parents are causing a strong count for arc
            num_parents_owning: AtomicU8,
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

        struct ParentOwner<T, U, V>(Arc<JoinedSyncCell<T, U, V>>)
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
                self.0.0.borrow_mut().num_parents_owning.fetch_sub(1, SeqCst);
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
                let inner = {
                    let l = lhs.borrow(s);
                    let r = rhs.borrow(s);

                    JoinedInnerSignal {
                        _quarve_tag: QuarveAllocTag::new(),
                        t: l.clone(),
                        u: r.clone(),
                        ours: map(&*l, &*r),
                        audience: SignalAudience::new(),
                        num_parents_owning: AtomicU8::new(2),
                    }
                };

                let arc = Arc::new(JoinedSyncCell(RefCell::new(inner)));

                let pseudo_weak = ParentOwner(arc.clone());
                let lhs_map = map.clone();
                lhs.listen(move |lhs, s| {
                    let ParentOwner(pseudo_weak) = &pseudo_weak;

                    let mut binding = pseudo_weak.0.borrow_mut();
                    let inner = binding.deref_mut();
                    inner.t = lhs.clone();
                    inner.ours = lhs_map(&inner.t, &inner.u);
                    inner.audience.dispatch(&inner.ours, s);

                    // certainly this can change, but we do not particular care
                    // since this is just an early exit, not necessarily the final

                    !inner.audience.is_empty() ||
                        Arc::strong_count(&pseudo_weak) > inner.num_parents_owning.load(SeqCst) as usize
                }, s);

                let pseudo_weak = ParentOwner(arc.clone());
                rhs.listen(move |rhs, s| {
                    let ParentOwner(pseudo_weak) = &pseudo_weak;

                    let mut binding = pseudo_weak.0.borrow_mut();
                    let inner = binding.deref_mut();
                    inner.u = rhs.clone();
                    inner.ours = map(&inner.t, &inner.u);
                    inner.audience.dispatch(&inner.ours, s);

                    !inner.audience.is_empty() ||
                        Arc::strong_count(&pseudo_weak) > inner.num_parents_owning.load(SeqCst) as usize
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

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>)
                where F: FnMut(&V, &Slock) -> bool + Send + 'static {
                self.inner.0.borrow_mut().audience.listen(listener, s);
            }

            type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
            fn map<S, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> GeneralSignal<S>
                where S: Send + 'static,
                      F: Send + 'static + Fn(&V) -> S
            {
                GeneralSignal::from(self, map, |this, listener, s| {
                    this.inner.0.borrow_mut().audience.listen_box(listener, s);
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

    mod timed_signal {
        use std::cell::RefCell;
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
        use crate::util::markers::ThreadMarker;
        use crate::util::test_util::QuarveAllocTag;

        struct CapacitatedInnerSignal<C> where C: Capacitor {
            _quarve_tag: QuarveAllocTag,
            curr: C::Target,
            capacitor: C,
            time_active: Option<Duration>,
            audience: SignalAudience<C::Target>,
            parent_retain_count: AtomicU8
        }

        impl<C> CapacitatedInnerSignal<C> where C: Capacitor {
            fn set_curr(&mut self, to: C::Target, s: &Slock) {
                self.curr = to;
                self.audience.dispatch(&self.curr, s);
            }
        }

        impl<C> InnerSignal<C::Target> for CapacitatedInnerSignal<C> where C: Capacitor {
            fn borrow(&self) -> &C::Target {
                &self.curr
            }
        }

        pub struct CapacitatedSignal<C> where C: Capacitor {
            inner: Arc<RefCell<CapacitatedInnerSignal<C>>>
        }

        impl<C> Clone for CapacitatedSignal<C> where C: Capacitor {
            fn clone(&self) -> Self {
                CapacitatedSignal {
                    inner: self.inner.clone()
                }
            }
        }

        struct ParentOwner<C>(Arc<RefCell<CapacitatedInnerSignal<C>>>) where C: Capacitor;

        // TODO, I think SeqCst is overkill in this scenario
        // and likewise for JoinedSignal
        impl<C> Drop for ParentOwner<C> where C: Capacitor {
            fn drop(&mut self) {
                // it's important that this is subtracted at a time
                // strictly before the ARC strong counter
                // so that we do not falsely free early
                self.0.borrow_mut().parent_retain_count.fetch_sub(1, SeqCst);
            }
        }
        impl<C> CapacitatedSignal<C> where C: Capacitor {

            #[inline]
            fn update_active(this: &Arc<RefCell<CapacitatedInnerSignal<C>>>, mut_ref: &mut CapacitatedInnerSignal<C>, _s: &Slock) {
                if mut_ref.time_active.is_none() {
                    mut_ref.time_active = Some(Duration::from_secs(0));

                    /* spawn worker */
                    mut_ref.parent_retain_count.fetch_add(1, SeqCst);
                    let worker_arc = ParentOwner(this.clone());
                    timed_worker(move |duration, s| {
                        let ParentOwner(worker_arc) = &worker_arc;

                        let mut borrow = worker_arc.borrow_mut();
                        let mut_ref = borrow.deref_mut();

                        let (sample, cont) = mut_ref.capacitor.sample(duration);
                        mut_ref.set_curr(sample, s);

                        if !cont {
                            mut_ref.time_active = None;

                            false
                        }
                        else {
                            mut_ref.time_active = Some(duration);

                            !mut_ref.audience.is_empty() ||
                                Arc::strong_count(&worker_arc) > mut_ref.parent_retain_count.load(SeqCst) as usize
                        }
                    })
                }
            }
        }

        impl<C> CapacitatedSignal<C> where C: Capacitor {
            pub fn from(source: &impl Signal<C::Target>, mut capacitor: C, s: &Slock<impl ThreadMarker>) -> Self {
                capacitor.target_set(&*source.borrow(s), None);
                let (curr, initial_thread) = capacitor.sample(Duration::from_secs(0));

                let arc = Arc::new(RefCell::new(CapacitatedInnerSignal {
                    _quarve_tag: QuarveAllocTag::new(),
                    curr,
                    capacitor,
                    time_active: None,
                    audience: SignalAudience::new(),
                    // parent signal (timer thread incremented whenever)
                    parent_retain_count: AtomicU8::new(1)
                }));

                // so we can't do just a weak signal
                // since then it may be dropped prematurely
                // the exact semantics we want is that the worker_thread (/parent signal) owns us
                // but if no one is listening, and no listeners in the future
                // which we can argue via retain count, only then can we cancel
                let parent_arc = ParentOwner(arc.clone());
                source.listen(move |curr, s| {
                    let ParentOwner(parent_arc) = &parent_arc;

                    let mut borrow = parent_arc.borrow_mut();
                    let mut_ref = borrow.deref_mut();
                    mut_ref.capacitor.target_set(curr, mut_ref.time_active);
                    CapacitatedSignal::update_active(&parent_arc, mut_ref, s);

                    !mut_ref.audience.is_empty() ||
                        Arc::strong_count(&parent_arc) > mut_ref.parent_retain_count.load(SeqCst) as usize
                }, s);

                // start thread if necessary
                if initial_thread {
                    CapacitatedSignal::update_active(&arc, arc.borrow_mut().deref_mut(), s.as_ref());
                }

                CapacitatedSignal {
                    inner: arc
                }
            }
        }

        impl<C> Signal<C::Target> for CapacitatedSignal<C> where C: Capacitor {
            fn borrow<'a>(&'a self, _s: &'a Slock<impl ThreadMarker>) -> impl Deref<Target=C::Target> {
                SignalRef {
                    src: self.inner.borrow(),
                    marker: Default::default(),
                }
            }

            fn listen<F>(&self, listener: F, s: &Slock<impl ThreadMarker>)
                where F: FnMut(&C::Target, &Slock) -> bool + Send + 'static {
                self.inner.borrow_mut().audience.listen(listener, s);
            }

            type MappedOutput<S: Send + 'static> = GeneralSignal<S>;
            fn map<S, F>(&self, map: F, s: &Slock<impl ThreadMarker>) -> GeneralSignal<S>
                where S: Send + 'static,
                      F: Send + 'static + Fn(&C::Target) -> S
            {
                GeneralSignal::from(self, map, |this, listener, s| {
                    this.inner.borrow_mut().audience.listen_box(listener, s);
                }, s)
            }
        }

        unsafe impl<C> Send for ParentOwner<C> where C: Capacitor {}
        unsafe impl<C> Send for CapacitatedSignal<C> where C: Capacitor {}
        unsafe impl<C> Sync for CapacitatedSignal<C> where C: Capacitor {}
    }
    pub use timed_signal::*;
    use crate::state::capacitor::Capacitor;
    use crate::util::markers::ThreadMarker;
}
pub use signal::*;

#[cfg(test)]
mod test {
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;
    use rand::Rng;
    use crate::core::{setup_timing_thread, slock, timed_worker};
    use crate::state::{Store, Signal, TokenStore, Binding, Bindable, ActionDispatcher, StoreContainer, NumericAction, DirectlyInvertible, Filterable, DerivedStore, Stateful, CoupledStore, StringActionBasis, Buffer, Word, GroupAction};
    use crate::state::capacitor::{ConstantSpeedCapacitor, ConstantTimeCapacitor, SmoothCapacitor};
    use crate::state::coupler::{FilterlessCoupler, NumericStringCoupler};
    use crate::state::SetAction::{Identity, Set};
    use crate::state::VecActionBasis::{Insert, Remove, Swap};
    use crate::util::numeric::Norm;
    use crate::util::test_util::HeapChecker;
    use crate::util::Vector;

    #[test]
    fn test_numeric() {
        let _h = HeapChecker::new();
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
            let sig = c.fixed_signal(-1);

            sig1 = Signal::map(&sig, |x| 2 * x, &c);
        }

        let b = sig1.borrow(&c);
        let c = *b;
        assert_eq!(c, -2);


    }


    #[test]
    fn test_join() {
        let _h = HeapChecker::new();
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
        let _h = HeapChecker::new();
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
        let _h = HeapChecker::new();
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
        let _h = HeapChecker::new();
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
        let _h = HeapChecker::new();
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
        let _h = HeapChecker::new();
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
            item.invert(&s);
        }
    }

    #[test]
    fn test_inverse_listener_combine() {
        let _h = HeapChecker::new();
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
        res.invert(&s);
        assert_eq!(*state.borrow(&s), 0);
    }

    #[test]
    fn test_general_listener() {
        let _h = HeapChecker::new();
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
        let _h = HeapChecker::new();
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
        let _h = HeapChecker::new();
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
        let _h = HeapChecker::new();
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

    #[test]
    fn test_signal_no_early_freeing() {
        // even if intermediate signals are dropped
        // downstream signals remain unaffected
        let _h = HeapChecker::new();
        let s = slock();
        let store = Store::new(0);
        let middle = store.map(|x| *x, &s);
        let bottom = middle.map(|x| *x, &s);
        let changes = DerivedStore::new(0);
        let binding = changes.binding();
        bottom.listen(move |_a, s| {
            binding.apply(NumericAction::Incr(1), s);
            true
        }, &s);

        store.apply(Set(1), &s);
        drop(middle);
        store.apply(Set(-1), &s);
        drop(bottom);

        assert_eq!(*changes.borrow(&s), 2);
    }

    #[test]
    fn test_signal_early_freeing() {
        let _h = HeapChecker::new();
        let s = slock();
        let store = Store::new(0);
        {
            let _h = HeapChecker::new();
            let middle = store.map(|x| *x, &s);
            drop(middle);
            // this operation should clear ownership of the signal
            store.apply(Set(1), &s);
        }
    }

    #[test]
    #[should_panic]
    fn test_signal_no_early_freeing_without_clear() {
        let s = slock();
        let store = Store::new(0);
        {
            let _h = HeapChecker::new();
            let middle = store.map(|x| *x, &s);
            drop(middle);
            // with no modification, signal will be owned by store
            // store.apply(Set(1), &s);
        }
    }

    #[test]
    fn test_join_no_early_freeing() {
        let h = HeapChecker::new();
        let s = slock();

        let left = Store::new(0);
        let right = Store::new(0);
        let left_binding = left.binding();
        let middle = s.join(&left, &right);
        {
            let hc2 = HeapChecker::new();
            let bottom = middle.map(|x| *x, &s);
            //
            drop(middle);
            drop(left);
            left_binding.apply(Set(1), &s);

            right.apply(Set(1), &s);
            drop(bottom);

            // at this point, both left and right have ownership of bottom
            hc2.assert_diff(1);

            left_binding.apply(Set(1), &s);
            // middle no longer sees bottom
            hc2.assert_diff(0);

            // left no longer sees middle, but right still doess
        }
        h.assert_diff(3);
        right.apply(Set(1), &s);
        // right no longer sees middle + middle dropped
        h.assert_diff(2);
    }

    #[test]
    fn test_couple_early_free() {
        let s = slock();

        {
            let _h = HeapChecker::new();
            let store = Store::new(0.0);
            let _coupled = CoupledStore::new(store.binding(), NegatedCoupler {}, &s);
        }

        {
            let _h = HeapChecker::new();
            let store = Store::new(0.0);
            let coupled = CoupledStore::new(store.binding(), NegatedCoupler {}, &s);
            store.listen(|_a, _s| true, &s);
            coupled.listen(|_a, _s| true, &s);
        }

        {
            let _h = HeapChecker::new();
            let store = Store::new(0.0);
            let coupled = CoupledStore::new(store.binding(), NegatedCoupler {}, &s);
            let s_bind = store.binding();
            let _c_bind = coupled.binding();
            drop(store);
            drop(s_bind);
        }

        {
            let _h = HeapChecker::new();
            let store = Store::new(0.0);
            let coupled = CoupledStore::new(store.binding(), NegatedCoupler {}, &s);
            let _coupled2 = CoupledStore::new(store.binding(), NegatedCoupler {}, &s);
            let _coupled_coupled = CoupledStore::new(coupled.binding(), NegatedCoupler {}, &s);
            let s_bind = store.binding();
            let _c_bind = coupled.binding();
            drop(store);
            drop(s_bind);
        }
    }

    #[test]
    fn test_string() {
        let _h = HeapChecker::new();
        let s = slock();
        let actions: Arc<Mutex<Vec<Box<dyn DirectlyInvertible>>>> = Arc::new(Mutex::new(Vec::new()));
        let store = Store::new("asdfasdf".to_string());
        let mut strings: Vec<String> = Vec::new();
        let a = actions.clone();
        store.subtree_inverse_listener(move |invertible, _s| {
            a.lock().unwrap().push(invertible);
            true
        }, &s);
        for _i in 0 .. 127 {
            let curr = store.borrow(&s).clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.len() - i);
            strings.push(curr);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            store.apply(StringActionBasis::ReplaceSubrange(i..u+i, str), &s);
        }

        let mut actions = std::mem::replace(&mut *actions.lock().unwrap(), Vec::new());
        actions.reverse();

        for (i, mut action) in actions.into_iter().enumerate() {
            action.invert(&s);
            assert_eq!(*store.borrow(&s), strings[strings.len() - 1 - i].clone());
        }
    }

    #[test]
    fn test_string_compress() {
        let _h = HeapChecker::new();
        let s = slock();
        let state = Store::new("asfasdf".to_string());
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

        for _i in 0 .. 100 {
            let curr = state.borrow(&s).clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.len() - i);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            state.apply(StringActionBasis::ReplaceSubrange(i..u+i, str), &s);
        }
        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        res.invert(&s);
        assert_eq!(*state.borrow(&s), "asfasdf".to_string());
    }

    #[test]
    fn test_vec() {
        let _h = HeapChecker::new();
        let s = slock();
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
        }, &s);
        for _i in 0..127 {
            let curr: Vec<_> = store.borrow(&s)
                .iter()
                .map(|x| *x.borrow(&s))
                .collect();

            if !curr.is_empty() {
                let u = rand::thread_rng().gen_range(0..curr.len());
                let v = rand::thread_rng().gen_range(-100000..100000);
                items.push(curr);
                store.borrow(&s)[u]
                    .apply(Set(v), &s);
            }

            let curr: Vec<_> = store.borrow(&s)
                .iter()
                .map(|x| *x.borrow(&s))
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
            store.apply(act, &s);
        }

        let mut actions_ = std::mem::replace(&mut *actions.lock().unwrap(), Vec::new());
        actions_.reverse();

        for (i, mut action) in actions_.into_iter().enumerate() {
            action.invert(&s);
            assert_eq!(store.borrow(&s).len(), items[items.len() - 1 - i].len());
            for j in 0..items[items.len() - 1 - i].len() {
                assert_eq!(*store.borrow(&s)[j].borrow(&s), items[items.len() - 1 - i][j]);
            }
        }
    }

    #[test]
    fn test_vec_collapsed() {
        let _h = HeapChecker::new();
        let s = slock();
        let store: Store<Vec<Store<i32>>> = Store::new(vec![Store::new(1)]);
        let vec: Option<Box<dyn DirectlyInvertible>> = None;
        let vectors = Arc::new(Mutex::new(Some(vec)));
        let c = vectors.clone();
        store.subtree_inverse_listener(move |inv, _s| {
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
        for _i in 0 .. 127 {
            let curr: Vec<_> = store.borrow(&s)
                .iter()
                .map(|x| *x.borrow(&s))
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
            store.apply(act, &s);
        }

        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        res.invert(&s);

        assert_eq!(store.borrow(&s).len(), 1);
    }

    #[test]
    fn test_subtree_general_listener() {
        let _h = HeapChecker::new();
        let s = slock();
        let store = Store::new(vec![Store::new(1)]);
        let count = Arc::new(Mutex::new(0));
        let c = count.clone();
        store.subtree_general_listener(move |_s| {
            *c.lock().unwrap() += 1;
            true
        }, &s);
        s.apply(Insert(Store::new(2), 0), &store);
        s.apply(Set(1), &store.borrow(&s)[0]);

        // 3 because an extra call is made to check
        // if it's still relevant
        assert_eq!(*count.lock().unwrap(), 3);
    }

    #[test]
    fn test_clock_signal() {
        setup_timing_thread();

        let _h = HeapChecker::new();
        let clock = {
            let s = slock();
            s.clock_signal()
        };

        thread::sleep(Duration::from_millis(800));

        {
            let s = slock();
            assert!((*clock.borrow(&s) - 0.8).abs() < 0.16);
        }

        // wait for another tick to make sure clock is
        // freed from timer thread
        drop(clock);
        thread::sleep(Duration::from_millis(100));
    }

    #[test]
    fn test_constant_time_capacitor() {
        setup_timing_thread();

        let _h = HeapChecker::new();
        let store = Store::new(0.0);
        let capacitated = {
            let s = slock();
            let ret = store.with_capacitor(ConstantTimeCapacitor::new(1.0), &s);
            store.apply(Set(1.5), &s);

            ret
        };

        sleep(Duration::from_millis(100));

        {
            let s = slock();
            assert!((*capacitated.borrow(&s) - 0.15) < 0.05);
        }

        sleep(Duration::from_millis(1000));

        {
            let s = slock();
            assert!((*capacitated.borrow(&s) - 1.5) < 0.05);
            store.apply(Set(2.0), &s);
        }

        sleep(Duration::from_millis(400));

        {
            let s = slock();
            assert!((*capacitated.borrow(&s) - 1.7) < 0.05);
            store.apply(Set(10.0), &s);
        }

        {
            let s = slock();
            assert!((*capacitated.borrow(&s) - 2.0) < 0.05);
        }

        sleep(Duration::from_millis(100));

        {
            let s = slock();
            assert!((*capacitated.borrow(&s) - 2.8) < 0.05);
            store.apply(Set(3.0), &s);
        }

        sleep(Duration::from_millis(100));

        {
            let s = slock();
            assert!((*capacitated.borrow(&s) - 2.82) < 0.05);
        }

        sleep(Duration::from_millis(900));

        {
            let s = slock();
            assert!((*capacitated.borrow(&s) - 3.0) < 0.05);
        }

        sleep(Duration::from_millis(900));

        {
            let s = slock();
            assert!((*capacitated.borrow(&s) - 3.0) < 0.05);
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
            let s = slock();
            let ret = store.with_capacitor(ConstantSpeedCapacitor::new(2.0), &s);

            ret
        };

        let first = thread::spawn(move || {
            let set = |u, v| {
                let s = slock();
                store.apply([Set(u), Set(v)], &s);
            };

            set(1.0, 0.0);

            sleep(Duration::from_millis(1000));

            set(2.0, 3.0);

            sleep(Duration::from_millis(250));

            set(2.0,  1.0);
        });

        let second = thread::spawn(move || {
            let close_to = |u, v| {
                let s = slock();
                let ret = (*capacitated.borrow(&s) - Vector([u, v])).norm() < 0.1;
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
            let s = slock();
            store.with_capacitor(SmoothCapacitor::new(|t| {
                3.0 * t * t - 2.0 * t * t * t
            }, 1.5), &s)
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
                let s = slock();
                binding.apply(Set(targ), &s);
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
                let s = slock();
                // relatively high tolerance since
                // pretty steep
                assert!((*signal.borrow(&s) / vals[i] - 1.0).abs() < 0.2);
            }
        }).join().unwrap();
    }

    #[test]
    fn test_vector_action() {
        let _h = HeapChecker::new();
        let s = slock();
        let actions: Arc<Mutex<Vec<Box<dyn DirectlyInvertible>>>> = Arc::new(Mutex::new(Vec::new()));
        let store = Store::new(Vector([1, 2]));
        let weak = Arc::downgrade(&actions);
        store.subtree_inverse_listener(move |invertible, _s| {
            let Some(strong) = weak.upgrade() else {
                return false;
            };
            strong.lock().unwrap().push(invertible);
            true
        }, &s);
        store.apply([Set(2), Identity], &s);
        assert_eq!(*store.borrow(&s).x(), 2);
        assert_eq!(*store.borrow(&s).y(), 2);
        store.apply([Set(3), Set(1)], &s);
        assert_eq!(*store.borrow(&s).x(), 3);
        assert_eq!(*store.borrow(&s).y(), 1);

        let mut action = actions.lock().unwrap().pop().unwrap();
        let mut action2 = actions.lock().unwrap().pop().unwrap();

        action.invert(&s);
        assert_eq!(*store.borrow(&s).x(), 2);
        assert_eq!(*store.borrow(&s).y(), 2);

        action2.invert(&s);
        assert_eq!(*store.borrow(&s).x(), 1);
        assert_eq!(*store.borrow(&s).y(), 2);
    }

    #[test]
    fn test_vector_string() {
        let _h = HeapChecker::new();
        let s = slock();
        let actions: Arc<Mutex<Vec<Box<dyn DirectlyInvertible>>>> = Arc::new(Mutex::new(Vec::new()));
        let store = Store::new(Vector(["asdfasdf".to_string()]));
        let mut strings: Vec<String> = Vec::new();
        let a = actions.clone();
        store.subtree_inverse_listener(move |invertible, _s| {
            a.lock().unwrap().push(invertible);
            true
        }, &s);
        for _i in 0 .. 127 {
            let curr = store.borrow(&s).x().clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.len() - i);
            strings.push(curr);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            store.apply([StringActionBasis::ReplaceSubrange(i..u+i, str)], &s);
        }

        let mut actions = std::mem::replace(&mut *actions.lock().unwrap(), Vec::new());
        actions.reverse();

        for (i, mut action) in actions.into_iter().enumerate() {
            action.invert(&s);
            assert_eq!(*store.borrow(&s).x(), strings[strings.len() - 1 - i].clone());
        }
    }

    #[test]
    fn test_vector_string_collapsed() {
        let _h = HeapChecker::new();
        let s = slock();
        let state = Store::new(Vector(["asfasdf".to_string()]));
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

        for _i in 0 .. 100 {
            let curr = state.borrow(&s).x().clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.len() - i);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            state.apply([StringActionBasis::ReplaceSubrange(i..u+i, str)], &s);
        }
        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        res.invert(&s);
        assert_eq!(*state.borrow(&s).x(), "asfasdf".to_string());
    }

    #[test]
    fn test_filter() {
        let _h = HeapChecker::new();
        let s = slock();
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
        }, &s);
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
            assert_eq!(*state.borrow(&s) % 2, 0)
        }
        let mut l = vectors.lock().unwrap();
        let mut res = l.take().unwrap().unwrap();
        drop(l);
        res.invert(&s);
        assert_eq!(*state.borrow(&s), 0);
    }

    #[test]
    fn test_buffer() {
        let _h = HeapChecker::new();
        let s = slock();
        let state = Store::new("asfasdf".to_string());
        let buffer = Buffer::new(Word::identity());
        let buffer_writer = buffer.clone();
        state.action_listener(move |_, action, s| {
            buffer_writer.borrow_mut(s).left_multiply(action.clone());
            true
        }, &s);

        for _i in 0 .. 100 {
            let curr = state.borrow(&s).clone();
            let i = rand::thread_rng().gen_range(0 .. std::cmp::max(1, curr.len()));
            let u = rand::thread_rng().gen_range(0 ..= curr.len() - i);
            let mut str = rand::thread_rng().gen_range(0..100).to_string();
            str = str[0..rand::thread_rng().gen_range(0..= str.len())].to_string();
            state.apply(StringActionBasis::ReplaceSubrange(i..u+i, str), &s);
        }

        let state2 = Store::new("asfasdf".to_string());
        state2.apply(buffer.replace(Word::identity(), &s), &s);
        assert_eq!(*state2.borrow(&s), *state.borrow(&s));
    }
}