use crate::core::{Environment, MSlock};
use crate::util::geo::Size;
use crate::view::{Handle, Invalidator, Subviews};

// struct VStack, HStack, ZStack;
// struct HFlex, VFlex;
// struct VMap, HMap, ZMap, HFLexMap, VFlexMap;

pub trait LayoutProvider<E>: Sized + 'static where E: Environment {
    // fn make_view(self, s: MSlock) -> View<E, Self> {
    //     todo!()
    // }

    type LayoutContext: 'static;

    fn intrinsic_size(&self) -> Size;

    fn xsquished_size(&self) -> Size {
        self.intrinsic_size()
    }

    fn ysquished_size(&self) -> Size {
        self.intrinsic_size()
    }

    fn xstretched_size(&self) -> Size {
        Size::new(1000.0, 400.0)
    }

    fn ystretched_size(&self) -> Size {
        Size::new(1000.0, 400.0)
    }


    fn init(&self, invalidator: Invalidator<E>, s: MSlock) {

    }

    fn layout_up(&self, subviews: &Subviews<E>, env: &mut Handle<E>, s: MSlock);

    fn layout_down(self, env: &mut Handle<E>, s: MSlock);
}

pub enum Monotonicity {
    None,
    Vertical,
    Horizontal
}
pub trait DynamicLayoutProvider {
    const MONOTONICITY_OPTIMIZATIONS: Monotonicity;

    // fn make_view(self, s: MSlock) {
    //
    // }
}
