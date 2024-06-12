mod inner_view;
pub use inner_view::*;

mod view;
pub use view::*;

mod into_view_provider {
    use crate::core::{Environment, MSlock};
    use crate::view::ViewProvider;

    // it may seem like we will have to wait a while for
    // TAIT but in the meantime it's not so bad
    // since 99% of the time intoviewprovider is only called
    // from intoviewprovider methods, which means capturing
    // rules arent that bad. Otherwise, it's fine to elide
    // the capture rules anyways since ViewProvider references static data
    // (does require unsafe still though)
    pub trait IntoViewProvider<E: Environment>: Sized {
        type UpContext;
        type DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock)
            -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
    }
}
pub use into_view_provider::*;

mod into_up_context {
    // can't implement Into for our own context
    pub trait IntoUpContext<T>: 'static
        where T: 'static
    {
        fn into_up_context(self) -> T;
    }

    impl<T> IntoUpContext<T> for T
        where T: 'static
    {
        fn into_up_context(self) -> T {
            self
        }
    }
}
pub use into_up_context::*;

pub mod view_provider;
pub use view_provider::*;

pub mod layout;
pub mod modifers;
pub mod macros;
pub mod util;

#[cfg(debug_assertions)]
#[allow(unused_variables)]
pub mod dev_views;
mod signal_view;

// vstack, hstack, zstack, hflex, vflex
// scrollview
// text, textfield, monotonicview
// button, link, spacer, radiobutton, checkbox
// vsplit, hsplit
// router view/mux/match
// file opener
// image
// shape/path
// sheet, popover, codecompletionthing that's like a new window

// fonts

// modifiers
// opacity
// background
// border
// corner radius
// vmap, hmap, zmap
// min_frame, frame, max_frame (and alignment)
// flex_grow, flex_shrink, (and related)
// all done in a monadic fashion?
