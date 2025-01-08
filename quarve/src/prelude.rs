// convenience utilities

pub use color::*;
pub use global::*;
pub use layout::*;
pub use modifiers::*;
pub use state::*;
pub use view::*;

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
    pub const GREEN: Color = hex(0x4CB964);
    pub const BLUE: Color = hex(0x007AFF);
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

mod layout {
    use crate::core::Environment;
    pub use crate::view::layout::*;

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

mod view {
    use crate::core::{Environment, MSlock, StandardVarEnv};
    use crate::resource::Resource;
    use crate::state::{Binding, Filterless, Signal};
    pub use crate::view::conditional::*;
    use crate::view::control::Button;
    pub use crate::view::functional_ivp::ivp_using;
    use crate::view::image_view::ImageView;
    pub use crate::view::menu::{Menu, MenuButton, WindowMenu};
    use crate::view::scroll::ScrollView;
    use crate::view::text::{Text, TextField};
    pub use crate::view::view_match::ViewMatchIVP;
    pub use crate::view::{IntoViewProvider, ViewProvider};

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

mod modifiers {
    use crate::prelude::ScreenUnit;
    use crate::state::FixedSignal;
    pub use crate::view::modifers::*;
    use crate::view::util::Color;

    pub const F: Frame = Frame::new();
    pub const L: Layer<
        FixedSignal<Color>,
        FixedSignal<ScreenUnit>,
        FixedSignal<Color>,
        FixedSignal<ScreenUnit>,
        FixedSignal<f32>,
    > = Layer::new();
}

mod global {
    pub use crate::core::{
        ApplicationProvider, Environment,
        MSlock, Slock, StandardConstEnv,
        StandardVarEnv, WindowProvider,
    };
    pub use crate::resource::Resource;
    pub use crate::util::geo::*;
}

mod state {
    pub use crate::state::{
        Bindable, Binding, FixedSignal,
        Signal, Store,
        StoreContainer
    };
}