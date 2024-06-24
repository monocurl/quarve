mod size_container {
    use std::ops::{Index, IndexMut};
    use crate::util::geo::Size;

    #[derive(Default, PartialEq, Copy, Clone)]
    pub struct SizeContainer {
        sizes: [Size; 5]
    }

    impl SizeContainer {
        pub fn new(intrinsic: Size, xsquish: Size, xstretch: Size, ysquish: Size, ystretch: Size) -> Self {
            SizeContainer {
                sizes: [intrinsic, xsquish, xstretch, ysquish, ystretch]
            }
        }
    }

    impl Index<usize> for SizeContainer {
        type Output = Size;

        fn index(&self, index: usize) -> &Self::Output {
            &self.sizes[index]
        }
    }

    impl IndexMut<usize> for SizeContainer {
        fn index_mut(&mut self, index: usize) -> &mut Self::Output {
            &mut self.sizes[index]
        }
    }

    impl SizeContainer {
        pub fn num_sizes() -> usize {
            5
        }

        pub fn intrinsic(&self) -> Size {
            self.sizes[0]
        }

        pub fn intrinsic_mut(&mut self) -> &mut Size {
            &mut self.sizes[0]
        }

        pub fn xsquished(&self) -> Size {
            self.sizes[1]
        }

        pub fn xsquished_mut(&mut self) -> &mut Size {
            &mut self.sizes[1]
        }

        pub fn xstretched(&self) -> Size {
            self.sizes[2]
        }

        pub fn xstretched_mut(&mut self) -> &mut Size {
            &mut self.sizes[2]
        }

        pub fn ysquished(&self) -> Size {
            self.sizes[3]
        }

        pub fn ysquished_mut(&mut self) -> &mut Size {
            &mut self.sizes[3]
        }

        pub fn ystretched(&self) -> Size {
            self.sizes[4]
        }

        pub fn ystretched_mut(&mut self) -> &mut Size {
            &mut self.sizes[4]
        }
    }
}
pub use size_container::*;

mod color {
    #[derive(Default, Copy, Clone, PartialEq, Eq)]
    #[repr(C)]
    pub struct Color {
        r: u8, g: u8, b: u8, a: u8
    }

    impl Color {
        pub fn new(r: u8, g: u8, b: u8) -> Color {
            Color {
                r, g, b, a: u8::MAX
            }
        }

        pub fn new_alpha(r: u8, g: u8, b: u8, a: u8) -> Color {
            Color {
                r, g, b, a
            }
        }

        pub fn transparent() -> Color {
            Color {
                r: 0, g: 0, b: 0, a: 0
            }
        }

        pub fn black() -> Color {
            Color {
                r: 0, g: 0, b: 0, a: u8::MAX
            }
        }

        pub fn white() -> Color {
            Color {
                r: u8::MAX, g: u8::MAX, b: u8::MAX, a: u8::MAX
            }
        }

        pub fn r(&self) -> u8 {
            self.r
        }

        pub fn g(&self) -> u8 {
            self.g
        }

        pub fn b(&self) -> u8 {
            self.b
        }

        pub fn a(&self) -> u8 {
            self.a
        }
    }

}
pub use color::*;

mod font {

}
pub use font::*;