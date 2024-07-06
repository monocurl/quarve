use std::hash::Hash;
use crate::state::{Stateful, TokenStore};

pub trait Focus<T> where T: Stateful + Copy + Hash + Eq {
    /// TODO some issues currently when multiple views try to take focus at the same time
    fn focus_iff_equal(self, focus: &TokenStore<T>, token: T) -> Self;
    fn default_focus(self) -> Self;
}