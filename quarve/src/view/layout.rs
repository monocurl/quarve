use crate::core::{Environment, MSlock};
use crate::native;
use crate::util::geo::{AlignedFrame, Rect, Size};
use crate::view::{Handle, Invalidator, NativeView, Subtree, ViewProvider};

// struct VStack, HStack, ZStack;
// struct HFlex, VFlex;
// struct VMap, HMap, ZMap, HFLexMap, VFlexMap;

pub trait LayoutProvider<E>: Sized + 'static where E: Environment {
    type LayoutContext: 'static;

    fn into_layout_view_provider(self) -> LayoutViewProvider<E, Self> {
        LayoutViewProvider(self)
    }

    fn intrinsic_size(&self, s: MSlock) -> Size;

    fn xsquished_size(&self, s: MSlock) -> Size {
        self.intrinsic_size(s)
    }

    fn ysquished_size(&self, s: MSlock) -> Size {
        self.intrinsic_size(s)
    }

    fn xstretched_size(&self, s: MSlock) -> Size {
        self.intrinsic_size(s)
    }

    fn ystretched_size(&self, s: MSlock) -> Size {
        self.intrinsic_size(s)
    }

    fn init(
        &mut self,
        invalidator: Invalidator<E>,
        subtree: &mut Subtree<E>,
        source_provider: Option<Self>,
        env: &mut Handle<E>,
        s: MSlock
    );

    fn layout_up(
        &mut self,
        subtree: &mut Subtree<E>,
        env: &mut Handle<E>,
        s: MSlock
    ) -> bool;

    fn layout_down(
        &mut self,
        subtree: &Subtree<E>,
        frame: AlignedFrame,
        layout_context: &Self::LayoutContext,
        env: &mut Handle<E>,
        s: MSlock
    ) -> Rect;
}

pub struct LayoutViewProvider<E, L>(L) where E: Environment, L: LayoutProvider<E>;
unsafe impl<E, L> ViewProvider<E> for LayoutViewProvider<E, L>
    where E: Environment, L: LayoutProvider<E> {
    type LayoutContext = L::LayoutContext;

    fn intrinsic_size(&self, s: MSlock) -> Size {
        self.0.intrinsic_size(s)
    }

    fn xsquished_size(&self, s: MSlock) -> Size {
        self.0.xsquished_size(s)
    }

    fn ysquished_size(&self, s: MSlock) -> Size {
        self.ysquished_size(s)
    }

    fn xstretched_size(&self, s: MSlock) -> Size {
        self.xstretched_size(s)
    }

    fn ystretched_size(&self, s: MSlock) -> Size {
        self.ystretched_size(s)
    }

    fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut Handle<E>, s: MSlock<'_>) -> NativeView {
        if let Some(source) = backing_source {
            self.0.init(invalidator, subtree, Some(source.1.0), env, s);

            source.0
        }
        else {
            self.0.init(invalidator, subtree, None, env, s);

            NativeView::new(native::view::init_layout_view(s))
        }
    }

    fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut Handle<E>, s: MSlock<'_>) -> bool {
        self.0.layout_up(subtree, env, s)
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: AlignedFrame, layout_context: &Self::LayoutContext, env: &mut Handle<E>, s: MSlock<'_>) -> Rect {
        self.0.layout_down(subtree, frame, layout_context, env, s)
    }
}

pub trait DynamicLayoutProvider {

}
