use crate::core::{WindowBase};

pub type WindowHandle = usize;

#[repr(C)]
struct FatPointer(usize, usize);

impl From<&dyn WindowBase> for FatPointer {
    fn from(value: &dyn WindowBase) -> Self {
        unsafe {
            std::mem::transmute(value)
        }
    }
}

impl FatPointer {
    fn into_window(self) -> &'static dyn WindowBase {
        unsafe {
            std::mem::transmute(self)
        }
    }
}

/* back -> front call backs */
mod callbacks {
    use crate::core::{APP, Slock, slock_main_owner};
    use crate::native::FatPointer;
    use crate::util::markers::MainThreadMarker;

    #[no_mangle]
    extern "C" fn front_will_spawn() {
        APP.with(|m|
            m.get()
                .unwrap()
                .will_spawn()
        );
    }

    #[no_mangle]
    extern "C" fn front_window_should_close(handle: FatPointer) -> bool {
        let s = slock_main_owner();

        handle.into_window().can_close(s.marker())
    }

    #[no_mangle]
    extern "C" fn front_window_layout(handle: FatPointer) {
        let s = slock_main_owner();

        handle.into_window().layout(s.marker())
    }

    #[no_mangle]
    extern "C" fn front_execute_box(bx: FatPointer) {
        /* ownership taken */
        let b: Box<dyn for<'a> FnOnce(Slock<'a, MainThreadMarker>) + Send> = unsafe {
            std::mem::transmute(bx)
        };

        /* main thread only */
        let s = slock_main_owner();

        b(s.marker());
    }
}

/* crate endpoints */
pub mod global {
    use crate::core::Slock;
    use crate::native::FatPointer;
    use crate::util::markers::MainThreadMarker;

    extern "C" {
        fn back_main_loop();

        fn back_is_main() -> bool;
        fn back_run_main(bx: FatPointer);


        fn back_terminate();
    }

    pub fn main_loop() {
        unsafe {
            back_main_loop();
        }
    }

    pub fn is_main() -> bool {
        unsafe {
            back_is_main()
        }
    }

    pub fn run_main<F: for<'a> FnOnce(Slock<'a, MainThreadMarker>) + Send + 'static>(f: F) {
        let b: Box<dyn for<'a> FnOnce(Slock<'a, MainThreadMarker>)> = Box::new(f);
        let b = unsafe {
            std::mem::transmute(b)
        };

        unsafe {
            back_run_main(b);
        }
    }

    pub fn exit() {
        unsafe {
            back_terminate();
        }
    }
}

/// Makes window handle and spawns it
pub mod window {
    use std::ffi::{c_void, CString};
    use crate::core::{MSlock, WindowBase};
    use crate::native::{FatPointer, WindowHandle};

    extern "C" {
        fn back_window_init() -> *mut c_void;
        fn back_window_set_handle(window: *mut c_void, handle: FatPointer);
        fn back_window_set_title(window: *mut c_void, title: *const u8);
        fn back_window_set_needs_layout(window: *mut c_void);
        fn back_window_set_root(window: *mut c_void, root: *mut c_void);
        fn back_window_exit(window: *mut c_void);
        fn back_window_free(window: *mut c_void);
    }

    pub fn window_init(_s: MSlock) -> WindowHandle
    {
        unsafe {
            back_window_init() as WindowHandle
        }
    }

    pub fn window_set_handle(window: WindowHandle, handle: &dyn WindowBase, _s: MSlock) {
        unsafe {
            back_window_set_handle(window as *mut c_void, std::mem::transmute(handle));
        }
    }

    pub fn window_set_title(window: WindowHandle, title: &str, _s: MSlock) {
        unsafe {
            let cstring = CString::new(title).unwrap();
            let bytes = cstring.as_bytes().as_ptr();
            back_window_set_title(window as *mut c_void, bytes)
        }
    }

    pub fn window_set_root(window: WindowHandle, root: *mut c_void, _s: MSlock) {
        unsafe {
            back_window_set_root(window as *mut c_void, root);
        }
    }

    pub fn window_set_needs_layout(window: WindowHandle, _s: MSlock) {
        unsafe {
            back_window_set_needs_layout(window as *mut c_void);
        }
    }

    pub fn window_exit(handle: WindowHandle, _s: MSlock) {
        unsafe {
            back_window_exit(handle as *mut c_void);
        }
    }

    pub fn window_free(handle: WindowHandle) {
        unsafe {
            back_window_free(handle as *mut c_void);
        }
    }
}

// view methods
pub mod view {
    use std::ffi::{c_ulonglong, c_void};
    use crate::core::MSlock;
    use crate::util::geo::Rect;

    extern "C" {
        fn debug_back_view_init() -> *mut c_void;

        fn back_view_layout_init() -> *mut c_void;
        fn back_view_clear_children(view: *mut c_void);
        fn back_view_remove_child(view: *mut c_void, index: std::ffi::c_ulonglong);
        fn back_view_insert_child(view: *mut c_void, subview: *mut c_void, index: std::ffi::c_ulonglong);
        fn back_view_set_frame(view: *mut c_void, left: f64, top: f64, width: f64, height: f64);
        fn back_free_view(view: *mut c_void);
    }

    pub fn debug_view_init(_s: MSlock) -> *mut c_void {
        unsafe {
            debug_back_view_init()
        }
    }

    pub fn init_layout_view(_s: MSlock) -> *mut c_void {
        unsafe {
            back_view_layout_init()
        }
    }

    pub fn view_clear_children(view: *mut c_void, _s: MSlock) {
        unsafe {
            back_view_clear_children(view);
        }
    }

    pub fn view_remove_child(view: *mut c_void, at: usize, _s: MSlock) {
        unsafe {
            back_view_remove_child(view, at as c_ulonglong);
        }
    }

    pub fn view_add_child_at(view: *mut c_void, subview: *mut c_void, at: usize, _s: MSlock) {
        unsafe {
            back_view_insert_child(view, subview, at as c_ulonglong);
        }
    }

    pub fn view_set_frame(view: *mut c_void, frame: Rect, _s: MSlock) {
        unsafe {
            back_view_set_frame(view, frame.x as f64, frame.y as f64, frame.w as f64, frame.h as f64);
        }
    }

    pub fn free_view(view: *mut c_void) {
        // this should be the case
        // due to the fact that views can only be freed by their
        // parents (all other arcs are weak
        // and we cant have race conditions anyways
        // due to the slock)
        // nevertheless a safety check doesn't hurt
        debug_assert!(super::global::is_main());

        unsafe {
            back_free_view(view);
        }
    }
}