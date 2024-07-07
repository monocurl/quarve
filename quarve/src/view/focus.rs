use std::hash::Hash;
use crate::core::{Environment, MSlock};
use crate::state::{Stateful, TokenStore};
use crate::view::{IntoViewProvider, ViewProvider};

// Don't like this all that much
// Might be a better way to go about htings
pub trait FocusIVP<E, T>
    where E: Environment,
          Option<T>: Stateful + Copy + Hash + Eq
{
    type UpContext: 'static;
    type DownContext: 'static;

    fn into_view_provider(self, indicator: &TokenStore<Option<T>>, token: T, env: &E::Const, s: MSlock)
        -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
}

pub trait FocusWhenEqual<T> where Option<T>: Stateful + Copy + Hash + Eq {
    fn focus_when_equal(self, indicator: &TokenStore<Option<T>>, token: T) -> Self;
    fn default_focus(self) -> Self;
}