use crate::{ChannelProvider, Slock};

struct Rect {
    origin: Point,
    size: Size,
}

struct Point {
    x: f32,
    y: f32,
}

struct Size {
    w: f32,
    h: f32
}

struct Backing {

}

trait View: 'static {
    type Backing;
    fn backing_exchange(&mut self, ex: Option<Self::Backing>) -> Option<Self::Backing>;
    fn backing(&self) -> Option<Self::Backing>;

    fn layout_up(&mut self) -> bool;
    fn layout_down(&mut self, with_frame: i32);
}

// trait Template<W: ChannelProvider, A: ChannelProvider>: IntoView {
//     fn template(&self, s: &Slock, winc: &W, appc: &A) -> impl IntoView;
// }

trait InnerView: View {

}

struct ScrollView {

}

// vstack, hstack, zstack, flex
// scrollview
// text, textfield, textview
// button, link, spacer, radiobutton, checkbox
// vsplit, hsplit
// router view/mux/match
// image
// shape/path
// sheet, popover

// fonts
struct Font {

}

// modifiers
// opacity
// background
// border
// vmap, hmap, zmap
// min_frame, frame, max_frame (and alignment)
// flex_grow, flex_shrink, (and related)
// all done in a monadic fashion?
