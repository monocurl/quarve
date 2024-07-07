use std::ffi::{c_char, c_void, CStr, CString};
use crate::core::{WindowNativeCallback};
use crate::event::{Event, EventModifiers, EventPayload, Key, KeyEvent, MouseEvent};
use crate::util::geo::{Point, ScreenUnit};

pub type WindowHandle = usize;

#[repr(C)]
struct FatPointer(usize, usize);

#[repr(C)]
struct BufferEvent {
    is_mouse: bool,
    is_scroll: bool,
    is_up: bool,
    is_down: bool,
    is_left_button: bool,
    is_right_button: bool,
    modifiers: u8,
    cursor_x: ScreenUnit,
    cursor_y: ScreenUnit,
    // scroll or mouse delta
    delta_x: ScreenUnit,
    delta_y: ScreenUnit,
    key_characters: *const u8,
    native_event: *mut c_void,
}

impl From<&dyn WindowNativeCallback> for FatPointer {
    fn from(value: &dyn WindowNativeCallback) -> Self {
        unsafe {
            std::mem::transmute(value)
        }
    }
}

impl FatPointer {
    fn into_window(self) -> &'static dyn WindowNativeCallback {
        unsafe {
            std::mem::transmute(self)
        }
    }
}

impl From<BufferEvent> for Event {
    fn from(value: BufferEvent) -> Self {
        let payload = if value.is_mouse {
            let mouse = if value.is_scroll {
                MouseEvent::Scroll(value.delta_x, value.delta_y)
            } else if value.is_left_button {
                if value.is_down {
                    MouseEvent::LeftDown
                }
                else if value.is_up {
                    MouseEvent::LeftUp
                }
                else {
                    MouseEvent::LeftDrag(value.delta_x, value.delta_y)
                }
            } else if value.is_right_button {
                if value.is_down {
                    MouseEvent::RightDown
                }
                else if value.is_up {
                    MouseEvent::RightUp
                }
                else {
                    MouseEvent::RightDrag(value.delta_x, value.delta_y)
                }
            } else {
                MouseEvent::Move(value.delta_x, value.delta_y)
            };

            EventPayload::Mouse(mouse, Point::new(value.cursor_x, value.cursor_y))
        }
        else {
            let cstr = unsafe { CStr::from_ptr(value.key_characters as *const c_char) };
            let characters = CString::from(cstr).into_string().unwrap();
            let key = Key::new(characters);
            let key = if value.is_down {
                KeyEvent::Press(key)
            } else if value.is_up {
                KeyEvent::Release(key)
            } else {
                KeyEvent::Repeat(key)
            };

            EventPayload::Key(key)
        };

        Event {
            payload,
            modifiers: EventModifiers {
                modifiers: value.modifiers
            },
            native_event: value.native_event,
        }
    }
}

/* back -> front call backs */
mod callbacks {
    use crate::core::{APP, Slock, slock_force_main_owner, slock_main_owner};
    use crate::native::{BufferEvent, FatPointer};
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
    extern "C" fn front_window_layout(handle: FatPointer, w: f64, h: f64) {
        let s = slock_main_owner();

        handle.into_window().layout_full(w, h, s.marker())
    }

    #[no_mangle]
    extern "C" fn front_window_dispatch_event(handle: FatPointer, event: BufferEvent) {
        let s = slock_main_owner();

        handle.into_window()
            .dispatch_native_event(event.into(), s.marker());
    }

    #[no_mangle]
    extern "C" fn front_window_will_fullscreen(p: FatPointer, fs: bool) {
        let s = unsafe {
            slock_force_main_owner()
        };

        println!("Fullscreen {:?}", fs);

        p.into_window()
            .set_fullscreen(fs, s.marker());
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
    use std::cell::Cell;
    use crate::core::Slock;
    use crate::native::FatPointer;
    use crate::util::markers::MainThreadMarker;

    extern "C" {
        fn back_main_loop();

        fn back_run_main(bx: FatPointer);

        fn back_terminate();
    }

    thread_local! {
        static MAIN: Cell<bool> = const { Cell::new(false) };
    }

    pub fn main_loop() {
        MAIN.set(true);

        unsafe {
            back_main_loop();
        }
    }

    pub fn is_main() -> bool {
        MAIN.get()
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

pub mod window {
    use std::ffi::{c_void, CString};
    use crate::core::{MSlock, WindowNativeCallback};
    use crate::native::{FatPointer, WindowHandle};

    extern "C" {
        fn back_window_init() -> *mut c_void;
        fn back_window_set_handle(window: *mut c_void, handle: FatPointer);
        fn back_window_set_title(window: *mut c_void, title: *const u8);
        fn back_window_set_needs_layout(window: *mut c_void);
        fn back_window_set_root(window: *mut c_void, root: *mut c_void);
        fn back_window_set_size(window: *mut c_void, w: f64, h: f64);
        fn back_window_set_min_size(window: *mut c_void, w: f64, h: f64);
        fn back_window_set_max_size(window: *mut c_void, w: f64, h: f64);
        fn back_window_set_fullscreen(window: *mut c_void, fs: bool);
        // Note that this should NOT call front_window_should_close even though it's performed by front
        fn back_window_exit(window: *mut c_void);
        fn back_window_free(window: *mut c_void);
    }

    pub fn window_init(_s: MSlock) -> WindowHandle
    {
        unsafe {
            back_window_init() as WindowHandle
        }
    }

    pub fn window_set_handle(window: WindowHandle, handle: &dyn WindowNativeCallback, _s: MSlock) {
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

    pub fn window_set_size(window: WindowHandle, w: f64, h: f64, _s: MSlock) {
        unsafe {
            back_window_set_size(window as *mut c_void, w, h);
        }
    }

    pub fn window_set_min_size(window: WindowHandle, w: f64, h: f64, _s: MSlock) {
        unsafe {
            back_window_set_min_size(window as *mut c_void, w, h);
        }
    }

    pub fn window_set_max_size(window: WindowHandle, w: f64, h: f64, _s: MSlock) {
        unsafe {
            back_window_set_max_size(window as *mut c_void, w, h);
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

    pub fn window_set_fullscreen(window: WindowHandle, fs: bool, _s: MSlock) {
        unsafe {
            back_window_set_fullscreen(window as *mut c_void, fs);
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
    use crate::util::geo::{Rect, Size};
    use crate::view::util::Color;

    extern "C" {
        fn back_view_layout_init() -> *mut c_void;
        fn back_view_clear_children(view: *mut c_void);
        fn back_view_remove_child(view: *mut c_void, index: c_ulonglong);
        fn back_view_insert_child(view: *mut c_void, subview: *mut c_void, index: c_ulonglong);
        fn back_view_set_frame(view: *mut c_void, left: f64, top: f64, width: f64, height: f64);
        fn back_free_view(view: *mut c_void);

        /* layer view methods */
        fn back_view_layer_init() -> *mut c_void;
        fn back_view_layer_update(view: *mut c_void, bg_color: Color, border_color: Color, corner_radius: f64, border_width: f64, opacity: f32) -> *mut c_void;

        /* image view methods */
        fn back_view_image_init(path: *const u8) -> *mut c_void;
        fn back_view_image_size(image: *mut c_void) -> Size;

        /* Cursor View */
        fn back_view_cursor_init(cursor_type: std::ffi::c_int) -> *mut c_void;
        fn back_view_cursor_update(view: *mut c_void, cursor_type: std::ffi::c_int);

        /* scroll view */
        fn back_view_scroll_init(is_vertical: bool) -> *mut c_void;
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
        // (and check is performed on mslock)
        debug_assert!(super::global::is_main());

        unsafe {
            back_free_view(view);
        }
    }

    pub fn init_layout_view(_s: MSlock) -> *mut c_void {
        unsafe {
            back_view_layout_init()
        }
    }

    pub mod layer {
        use std::ffi::c_void;
        use crate::core::MSlock;
        use crate::native::view::{back_view_layer_init, back_view_layer_update};
        use crate::view::util::Color;

        pub fn init_layer_view(_s: MSlock) -> *mut c_void {
            unsafe {
                back_view_layer_init()
            }
        }

        pub fn update_layout_view(
            view: *mut c_void,
            bg_color: Color,
            border_color: Color,
            corner_radius: f64,
            border_width: f64,
            opacity: f32,
            _s: MSlock
        ) {
            unsafe {
                back_view_layer_update(
                    view,
                    bg_color,
                    border_color,
                    corner_radius,
                    border_width,
                    opacity
                );
            }
        }
    }

    pub mod image {
        use std::ffi::c_void;
        use crate::core::MSlock;
        use crate::native::view::{back_view_image_init, back_view_image_size};
        use crate::util::geo::Size;

        pub fn init_image_view(path: &[u8], _s: MSlock) -> *mut c_void {
            unsafe {
                back_view_image_init(path.as_ptr())
            }
        }

        pub fn image_view_size(view: *mut c_void) -> Size {
            unsafe {
                back_view_image_size(view)
            }
        }
    }

    pub mod cursor {
        use std::ffi::c_void;
        use crate::core::MSlock;
        use crate::native::view::{back_view_cursor_init, back_view_cursor_update};
        use crate::view::modifers::Cursor;

        pub fn init_cursor_view(cursor: Cursor, _s: MSlock) -> *mut c_void {
            unsafe {
                back_view_cursor_init(cursor as i32 as std::ffi::c_int)
            }
        }

        pub fn update_cursor_view(view: *mut c_void, cursor: Cursor) {
            unsafe {
                back_view_cursor_update(view, cursor as i32 as std::ffi::c_int);
            }
        }
    }

    pub mod scroll {
        use std::ffi::c_void;
        use crate::core::MSlock;
        use crate::native::view::back_view_scroll_init;

        pub fn init_scroll_view(vertical: bool, _s: MSlock) -> *mut c_void {
            unsafe {
                back_view_scroll_init(vertical)
            }
        }

    }
}

pub mod path {
    #[cfg(not(debug_assertions))]
    use std::path::PathBuf;

    #[cfg(all(target_os = "macos", not(debug_assertions)))]
    pub fn production_resource_root() -> PathBuf {
        std::env::current_exe().unwrap()
            .parent().unwrap()
            .join("Resources/res")
    }
}