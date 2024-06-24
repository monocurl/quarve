use std::marker::PhantomData;
use crate::core::Environment;
use crate::state::Signal;
use crate::view::{DummyProvider, IntoViewProvider, ViewProvider};

// if, else if, else
// TODO
pub trait ConditionalIVP<E>: IntoViewProvider<E> where E: Environment {
    fn into_conditional_ivp(self) -> impl ConditionalVP<E, DownContext=Self::DownContext>;
}

pub trait ConditionalVP<E>: ViewProvider<E> where E: Environment {
    fn attach(&mut self);
    fn detach(&mut self);
}

impl<E, U, D> ConditionalVP<E> for DummyProvider<E, U, D>
    where E: Environment, U: 'static, D: 'static
{
    fn attach(&mut self) { }

    fn detach(&mut self) { }
}

// pub struct IfIVP<S, E, P, N> where S: Signal<bool>, E: Environment, P: IntoViewProvider<E>, N: ConditionalIVP<E> {
//     cond: S,
//     next: Option<N>,
//     phantom: PhantomData<E>
// }

// pub fn sig_if<S, E, P>(cond: S, view: P) -> IfIVP<S, E, P, > where S: Signal<bool>, E: Environment, P: IntoViewProvider<E> {
//
// }
