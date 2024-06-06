mod inner_view;
pub use inner_view::*;

mod view;
pub use view::*;

mod into_view {
    use crate::core::{Environment, MSlock};
    use crate::view::{View, ViewProvider};

    pub trait IntoView<E: Environment> {
        fn into_view(self, env: &E, s: MSlock<'_>) -> View<E, impl ViewProvider<E>>;
    }
}
pub use into_view::*;

mod view_provider {
    use std::ffi::c_void;
    use crate::core::{Environment, MSlock};
    use crate::util::geo::{AlignedFrame, Rect, Size};
    use crate::view::inner_view::InnerView;
    use crate::view::{Handle, Invalidator, Subviews, View};

    pub unsafe trait ViewProvider<E>: Sized + 'static
        where E: Environment
    {
        /// Additional context to be used when performing layouts
        /// Typically, this is set to ()
        /// This may be useful when information from parent views
        /// must be sent down to child (or grandchild) views
        type LayoutContext: 'static;

        fn make_view(self, s: MSlock) -> View<E, Self> {
            View(InnerView::new(self, s))
        }

        fn intrinsic_size(&self, s: MSlock) -> Size;
        fn xsquished_size(&self, s: MSlock) -> Size;
        fn ysquished_size(&self, s: MSlock) -> Size;
        fn xstretched_size(&self, s: MSlock) -> Size;
        fn ystretched_size(&self, s: MSlock) -> Size;

        /// Allocate a backing and perform other initialization steps.
        /// This method will only be called once for a given view provider.
        ///
        /// * `replaced_backing` - The old backing that this view will be replacing.
        /// This will be None if we are not replacing an old view (i.e. a fresh allocation).
        /// It is guaranteed that the backing provided will be allocated from a view of the same type,
        /// specifically that which was provided the `replaced_provider`.
        /// The high level idea is that by providing the old backing,
        /// allocations may be avoided in a manner very similar to a recycler view.
        /// * `replaced_provider` - The provider that this view is replacing. None if if we are doing a
        /// fresh allocation and not replacing an old view.
        fn init_backing(
            &mut self,
            invalidator: Invalidator<E>,
            subviews: &mut Subviews<E>,
            replaced_backing: Option<*mut c_void>,
            replaced_provider: Option<Self>,
            env: &mut Handle<E>,
            s: MSlock<'_>
        ) -> *mut c_void;

        /// The children have properly calculated their
        /// minimum, intrinsic, and maximum sizes
        /// We must now calculate ours
        /// If any changes to the bounds happened,
        /// this method should return true to indicate that
        /// the parent must recalculate as well
        /// This method is always called before layout down
        /// and is generally the place to relay state changes to backings
        fn layout_up(
            &mut self,
            subviews: &mut Subviews<E>,
            env: &mut Handle<E>,
            s: MSlock<'_>
        ) -> bool;

        /// The children have properly calculated their
        /// minimum, intrinsic, and maximum sizes
        /// (and so have we)
        /// Now, we must position them according to the given frame
        /// Return value is used value within the frame
        fn layout_down(
            &mut self,
            frame: AlignedFrame,
            layout_context: &Self::LayoutContext,
            env: &mut Handle<E>,
            s: MSlock<'_>
        ) -> Rect;

        // callback methods
        fn pre_show(&self, _s: MSlock<'_>) {

        }

        fn post_show(&self, _s: MSlock<'_>) {

        }

        fn pre_hide(&self, _s: MSlock<'_>) {

        }

        fn post_hide(&self, _s: MSlock<'_>) {

        }

        // focus and unfocused state...
        fn focused(&self, _s: MSlock<'_>) {

        }

        fn unfocused(&self, _s: MSlock<'_>) {

        }

        fn push_environment(&self, _env: &mut E, _s: MSlock) {

        }

        fn pop_environment(&self, _env: &mut E, _s: MSlock) {

        }
    }
}
pub use view_provider::*;

pub mod layout;
pub mod modifers;

#[cfg(debug_assertions)]
pub mod dev_views;

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