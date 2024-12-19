// convenience utilities

mod color {
    use crate::prelude::color;
    use crate::view::util::Color;

    pub const CLEAR: Color = Color::clear();

    pub const WHITE: Color = Color::white();
    pub const LIGHT_GRAY: Color = Color::rgb(196, 196, 196);
    pub const GRAY: Color = Color::rgb(128, 128, 128);
    pub const DARK_GRAY: Color = Color::rgb(64, 64, 64);
    pub const BLACK: Color = Color::black();

    // manim
    pub const RED: Color = hex(0xFC6255);
    pub const ORANGE: Color = hex(0xFF7F50);
    pub const YELLOW: Color = hex(0xFFFF00);
    pub const GREEN: Color = hex(0x83C167);
    pub const BLUE: Color = hex(0x58C4DD);
    pub const PURPLE: Color = hex(0x9A72AC);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::rgb(r, g, b)
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color::rgba(r, g, b, a)
    }

    pub const fn hex(rgb: i32) -> Color  {
        color::rgb(
            ((rgb >> 16) & 0xFF) as u8,
            ((rgb >> 8) & 0xFF) as u8,
            ((rgb >> 0) & 0xFF) as u8,
        )
    }
}
pub use color::*;

mod layout {
    use crate::core::Environment;
    use crate::view::layout::{HeteroIVP, HeteroIVPNode, HStack, VecLayoutProvider, VStack, ZStack};

    /// Alias for `VStack::hetero()`
    pub fn vstack<E>() -> HeteroIVP<E, impl HeteroIVPNode<E, (), ()>, VStack>
        where E: Environment
    {
        VStack::hetero()
    }

    /// Alias for `HStack::hetero()`
    pub fn hstack<E>() -> HeteroIVP<E, impl HeteroIVPNode<E, (), ()>, HStack> where E: Environment
    {
        HStack::hetero()
    }

    /// Alias for `ZStack::hetero()`
    pub fn zstack<E>() -> HeteroIVP<E, impl HeteroIVPNode<E, (), ()>, ZStack>
        where E: Environment
    {
        ZStack::hetero()
    }
}
pub use layout::*;

mod view {
    pub use crate::view::conditional::*;
    pub use crate::view::{ViewProvider, IntoViewProvider};
    pub use crate::view::menu::{WindowMenu, Menu, MenuButton};

    use crate::core::{Environment, MSlock, StandardVarEnv};
    use crate::resource::Resource;
    use crate::state::{Binding, Filterless, Signal};
    use crate::view::control::{Button};
    use crate::view::image_view::ImageView;
    use crate::view::scroll::ScrollView;
    use crate::view::text::{Text, TextField};

    pub fn text(label: impl Into<String>) -> Text<impl Signal<Target=String>>
    {
        Text::new(label)
    }

    pub fn text_field<B>(content: B) -> TextField<B>
        where B: Binding<Filterless<String>> + Clone
    {
        TextField::new(content)
    }

    pub fn button<E>(text: impl Into<String>, action: impl Fn(MSlock) + 'static)
        -> impl IntoViewProvider<E, DownContext=(), UpContext=()>
        where E: Environment, E::Variable: AsRef<StandardVarEnv>
    {
        Button::new(text, action)
    }

    pub fn image(named: impl Into<Resource>) -> ImageView
    {
        ImageView::new(named)
    }

    pub fn hscroll<E, I>(content: I) -> ScrollView<E, I>
        where E: Environment, I: IntoViewProvider<E>
    {
        ScrollView::horizontal(content)
    }

    pub fn vscroll<E, I>(content: I) -> ScrollView<E, I>
        where E: Environment, I: IntoViewProvider<E>
    {
        ScrollView::vertical(content)
    }
}
pub use view::*;

mod modifiers {
    pub use crate::view::modifers::*;
    pub const F: Frame = Frame::new();
}
pub use modifiers::*;

mod global {
    pub use crate::core::{
        Slock, MSlock,
        Environment, StandardConstEnv, StandardVarEnv,
        WindowProvider, ApplicationProvider,
    };
    pub use crate::resource::{Resource};
    pub use crate::util::geo::*;
}
pub use global::*;

mod state {
    pub use crate::state::{
        Signal, Binding, Bindable,
        Store,
        FixedSignal
    };
}
pub use state::*;