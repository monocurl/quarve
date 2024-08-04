// convenience utilities

mod color {
    use crate::prelude::color;
    use crate::view::util::Color;

    pub fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::rgb(r, g, b)
    }

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
        Color::rgba(r, g, b, a)
    }

    pub fn hex(rgb: i32) -> Color {
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

    pub fn hstack<E>() -> HeteroIVP<E, impl HeteroIVPNode<E, (), ()>, HStack> where E: Environment
    {
        HStack::hetero()
    }

    pub fn zstack<E>() -> HeteroIVP<E, impl HeteroIVPNode<E, (), ()>, ZStack>
        where E: Environment
    {
        ZStack::hetero()
    }
}
pub use layout::*;