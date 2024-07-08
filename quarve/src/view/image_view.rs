use std::ffi::c_void;
use std::path::Path;
use crate::core::{Environment, MSlock};
use crate::native;
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
    pub fn new(location: impl Into<Resource>) -> Self {
        ImageView {
            location: location.into()
        }
    }

    pub fn named(rel_path: &str) -> Self {
        ImageView {
            location: Resource::new(Path::new(rel_path))
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

    fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
        ()
    }

    fn init_backing(&mut self, _invalidator: Invalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        if let Some((nv, src)) = backing_source {
            if src.location == self.location {
                return nv;
            }
        }

        let nv = native::view::image::init_image_view(self.location.path().as_os_str().as_encoded_bytes(), s);
        if nv.is_null() {
            panic!("Unable to create image!")
        }

        self.backing = nv;
        self.intrinsic = native::view::image::image_view_size(nv);
        NativeView::new(nv, s)
    }

    fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) -> bool {
        false
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, _s: MSlock) -> (Rect, Rect) {
        let corresponding_height = frame.w * self.intrinsic.h / self.intrinsic.w;
        let corresponding_width= frame.h * self.intrinsic.w / self.intrinsic.h;
        let used = if corresponding_width <= frame.w {
            Size::new(corresponding_width, frame.h)
        } else {
            Size::new(frame.w, corresponding_height)
        };

        (used.full_rect(), used.full_rect())
    }
}
