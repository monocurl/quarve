use std::marker::PhantomData;

use crate::core::{Environment, MSlock};
use crate::prelude::ViewProvider;
use crate::view::IntoViewProvider;

struct FunctionalIVP<E, F, V>
    where F: FnOnce(&E::Const, MSlock) -> V + 'static,
          E: Environment,
          V: IntoViewProvider<E>
{
    function: F,
    phantom: PhantomData<(E, fn(&E::Const) -> V)>
}

impl<E, F, V> IntoViewProvider<E> for FunctionalIVP<E, F, V>
    where F: FnOnce(&E::Const, MSlock) -> V + 'static,
          E: Environment,
          V: IntoViewProvider<E>
{
    type UpContext = V::UpContext;
    type DownContext = V::DownContext;

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        let ivp = (self.function)(env, s);
        ivp.into_view_provider(env, s)
    }
}

pub fn ivp_using<E, F, V>(function: F) -> impl IntoViewProvider<E, UpContext=V::UpContext, DownContext=V::DownContext>
    where F: FnOnce(&E::Const, MSlock) -> V + 'static,
          E: Environment,
          V: IntoViewProvider<E>
{
    FunctionalIVP {
        function,
        phantom: Default::default(),
    }
}