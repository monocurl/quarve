use std::ffi::c_void;
use crate::core::{Environment, MSlock};
use crate::resource::Resource;
use crate::util::geo::{Rect, Size};
use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ViewProvider};

// image view
pub struct ImageView {
    location: Resource
}

impl<E> IntoViewProvider<E> for ImageView where E: Environment {
    type UpContext = ();
    type DownContext = ();

    fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        ImageViewVP {
            location: self.location,
            backing: 0 as *mut c_void,
            intrinsic: Size::new(0.0, 0.0)
        }
    }
}

struct ImageViewVP {
    location: Resource,
    backing: *mut c_void,
    intrinsic: Size
}

impl ImageView {
    pub fn new(location: Resource) -> Self {
        ImageView {
            location
        }
    }
}

impl<E> ViewProvider<E> for ImageViewVP where E: Environment {
    type UpContext = ();
    type DownContext = ();

    fn intrinsic_size(&mut self, _s: MSlock) -> Size {
        self.intrinsic
    }

    fn xsquished_size(&mut self, _s: MSlock) -> Size {
        self.intrinsic
    }

    fn xstretched_size(&mut self, _s: MSlock) -> Size {
        self.intrinsic
    }

    fn ysquished_size(&mut self, _s: MSlock) -> Size {
        self.intrinsic
    }

    fn ystretched_size(&mut self, _s: MSlock) -> Size {
        self.intrinsic
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        ()
    }

    fn init_backing(&mut self, _invalidator: Invalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        if let Some((nv, src)) = backing_source {
            if src.location == self.location {
                return nv;
            }
        }

        // let nv =

        todo!()
    }

    fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
        todo!()
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        todo!()
    }
}
