mod inner_view;
pub use inner_view::*;

mod view;
pub use view::*;

mod into_view_provider {
    use crate::core::{Environment, MSlock};
    use crate::view::modifers::{ProviderIVPModifier, ProviderModifier};
    use crate::view::ViewProvider;

    // it may seem like we will have to wait a while for
    // TAIT but in the meantime it's not so bad
    // since 99% of the time intoviewprovider is only called
    // from intoviewprovider methods, which means capturing
    // rules arent that bad. Otherwise, it's fine to elide
    // the capture rules anyways since ViewProvider references static data
    // (does require unsafe still though)
    pub trait IntoViewProvider<E: Environment>: Sized {
        type UpContext: 'static;
        type DownContext: 'static;

        fn into_view_provider(self, env: &E::Const, s: MSlock)
            -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;

        fn provider_modifier<M>(self, modifier: M) -> ProviderIVPModifier<E, Self, M>
            where M: ProviderModifier<E, Self::UpContext, Self::DownContext>
        {
            ProviderIVPModifier::new(self, modifier)
        }

        // fn tree_modifier<P>(self, modifier: P) -> P
        //     where P: TreeModifier<E, Self::UpContext, Self::DownContext>
        // {
        //     self
        // }
    }
}
pub use into_view_provider::*;

mod view_provider;
pub use view_provider::*;

pub mod layout;
pub mod modifers;
pub mod util;

// #[cfg(debug_assertions)]
#[allow(unused_variables)]
pub mod dev_views;
pub mod color_view;
pub mod signal_view;
pub mod portal;

// vstack, hstack, zstack, hflex, vflex
// scrollview
// text, textfield, monotonicview
// button, link, spacer, radiobutton, checkbox
// vsplit, hsplit
// #if, #match (via view signal)
// router view/mux/match
// file opener
// image
// shape/path
// codecompletionthing that's like a new window
// fonts

// modifiers
// .hover
// .click
// when(
// opacity
// background
// border
// corner radius

// positional
// padding, offset
// box
// layer
// min_frame, frame, max_frame (and alignment)
// vmap, hmap, zmap
// flex_grow, flex_shrink, (and related)
// all done in a monadic fashion?
