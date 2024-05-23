use std::marker::PhantomData;
use crate::core::{ChannelProvider, Slock};
use crate::util::markers::MainThreadMarker;

pub struct NativeHandle<B> {
    backing: B
}

pub(crate) trait LayoutUpDown {
    fn layout_up(&self) -> bool;
    fn layout_down(&self);

    fn parent(&self) -> Option<&dyn LayoutUpDown>;
}

// contains a backing and
struct InnerView<P> where P: ViewProvider {
    // parent
    up: Option<&'static dyn LayoutUpDown>,
    depth: u16,
    provider: PhantomData<P>
}

pub struct Frame {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32
}

pub enum FrameAlignment {
    TopLeading,
    Top,
    TopTrailing,
    Leading,
    Center,
    Trailing,
    BotLeading,
    Bot,
    BotTrailing
}

pub struct View<P> where P: ViewProvider {
    inner: Box<InnerView<P>>
}

//     pub fn from_provider(provider: impl ViewProvider) -> Self {
//         View {
//
//         }
//     }

pub unsafe trait ViewProvider: Sized + 'static
{
    type ApplicationChannels: ChannelProvider;

    fn intrinsic_size();
    fn xsquished_size();
    fn ysquished_size();
    fn xstretched_size();
    fn ystretched_size();

    type Backing;
    /// Populate a new backing
    /// The structure of the old backing is undefined
    /// All we know is that it is of the same type as our backing
    fn form_backing(view: &mut View<Self>, possible_supplier: Option<Self::Backing>, s: &Slock<MainThreadMarker>) -> Self::Backing;

    /// The children have properly calculated their
    /// minimum, intrinsic, and maximum sizes
    /// We must now calculate ours
    /// If any changes to the bounds happened,
    /// this method should return true to indicate that
    /// the parent must recalculate as well
    /// This method is always called before layout down
    /// and is generally the place to relay state changes to backings
    fn layout_up(view: &mut View<Self>, channels: &Self::ApplicationChannels, s: &Slock<MainThreadMarker>) -> bool;

    /// The children have properly calculated their
    /// minimum, intrinsic, and maximum sizes
    /// (and so have we)
    /// Now, we must position them according to the given frame
    fn layout_down(view: &mut View<Self>, in_frame: Frame, with_alignment: FrameAlignment, channels: &Self::ApplicationChannels, s: &Slock<MainThreadMarker>);
}

pub trait LayoutProvider {
    // fn monotonicity(&self, x: X<i32>);

    fn layout(&self);

    // ad-hoc method
    // fn into_view(self) -> View<impl ViewProvider>;
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
// corner radius
// vmap, hmap, zmap
// min_frame, frame, max_frame (and alignment)
// flex_grow, flex_shrink, (and related)
// all done in a monadic fashion?
