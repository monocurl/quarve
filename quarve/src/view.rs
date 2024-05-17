use crate::core::{Slock};
use crate::util::markers::MainThreadMarker;

unsafe trait View: Sized + 'static {
    fn read_intrinsic_size();
    fn read_xsquished_size();
    fn read_ysquished_size();
    fn read_stretched_size();

    type Backing;
    /// Populate a new backing
    /// The structure of the old backing is undefined
    /// it could for example, be a textview that has garbage text in it
    fn backing_exchange(&mut self, ex: Option<Self::Backing>, s: &Slock<MainThreadMarker>);
    fn backing(&self) -> Self::Backing;

    /// The children have properly calculated their
    /// minimum, intrinsic, and maximum sizes
    /// We must now calculate ours
    /// If any changes to the bounds happened,
    /// this method should return true to indicate that
    /// the parent must recalculate as well
    /// This method is always called before layout down
    /// and is generally the place to relay state changes to backings
    fn layout_up(&mut self, s: &Slock<MainThreadMarker>) -> bool;

    /// The children have properly calculated their
    /// minimum, intrinsic, and maximum sizes
    /// (and so have we)
    /// Now, we must position them according to the given frame
    fn layout_down(&mut self, with_frame: i32, s: &Slock<MainThreadMarker>);
}

trait Layout {
    fn monotonicty();
}

// vstack, hstack, zstack, hflex, vflex
// scrollview
// text, textfield, textview
// button, link, spacer, radiobutton, checkbox
// vsplit, hsplit
// router view/mux/match
// image
// shape/path
// sheet, popover, codecompletionthing that's like a new window

// fonts

// modifiers
// opacity
// background
// border
// vmap, hmap, zmap
// min_frame, frame, max_frame (and alignment)
// flex_grow, flex_shrink, (and related)
// all done in a monadic fashion?
