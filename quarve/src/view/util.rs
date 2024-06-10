use std::ops::{Index, IndexMut};
use crate::util::geo::Size;

#[derive(Default, PartialEq, Copy, Clone)]
pub struct SizeContainer {
    sizes: [Size; 5]
}

impl SizeContainer {
    pub fn new(intrinsic: Size, xsquish: Size, xstretch: Size, ysquish: Size, ystretch: Size) -> Self{
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

    pub fn xsquished(&self) -> Size {
        self.sizes[1]
    }

    pub fn xstretched(&self) -> Size {
        self.sizes[2]
    }

    pub fn ysquished(&self) -> Size {
        self.sizes[3]
    }

    pub fn ystretched(&self) -> Size {
        self.sizes[4]
    }
}
