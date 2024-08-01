use std::ffi::{c_char, c_void, CStr, CString};
use crate::core::{WindowNativeCallback};
use crate::event::{Event, EventModifiers, EventPayload, Key, KeyEvent, MouseEvent};
use crate::util::geo::{Point, ScreenUnit};

// FIXME, name of functions are inconsistent

pub type WindowHandle = usize;

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


#[repr(C)]
struct FatPointer(usize, usize);

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
    use crate::core::{APP, MSlock, slock_force_main_owner, slock_main_owner, SlockOwner};
    use crate::native::{BufferEvent, FatPointer};
    use crate::util::geo::ScreenUnit;
    use crate::util::marker::MainThreadMarker;

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

        p.into_window()
            .set_fullscreen(fs, s.marker());
    }

    #[no_mangle]
    extern "C" fn front_execute_fn_once(bx: FatPointer) {
        /* ownership taken */
        let b: Box<dyn FnOnce(SlockOwner<MainThreadMarker>) + Send> = unsafe {
            std::mem::transmute(bx)
        };

        /* main thread only */
        let s = slock_main_owner();
        b(s);
    }

    #[no_mangle]
    extern "C" fn front_execute_fn_mut(bx: FatPointer) {
        let mut b: Box<dyn FnMut(MSlock)> = unsafe {
            std::mem::transmute(bx)
        };
        let s = slock_main_owner();
        b(s.marker());
    }

    #[no_mangle]
    extern "C" fn front_free_fn_mut(bx: FatPointer) {
        let _b: Box<dyn FnMut(MSlock)> = unsafe {
            std::mem::transmute(bx)
        };
    }

    #[no_mangle]
    extern "C" fn front_set_screen_unit_binding(bx: FatPointer, value: f64) {
        let s = unsafe {
            slock_force_main_owner()
        };
        let b: Box<dyn Fn(ScreenUnit, MSlock)> = unsafe {
            std::mem::transmute(bx)
        };

        b(value, s.marker());

        std::mem::forget(b);
    }

    #[no_mangle]
    extern "C" fn front_free_screen_unit_binding(bx: FatPointer) {
        let _b: Box<dyn Fn(ScreenUnit, MSlock)> = unsafe {
            std::mem::transmute(bx)
        };
    }

    #[no_mangle]
    extern "C" fn front_set_opt_string_binding(bx: FatPointer, value: *const u8) {
        let s = unsafe {
            slock_force_main_owner()
        };
        let b: Box<dyn Fn(*const u8, MSlock)> = unsafe {
            std::mem::transmute(bx)
        };

        b(value, s.marker());

        std::mem::forget(b);
    }

    #[no_mangle]
    extern "C" fn front_free_opt_string_binding(bx: FatPointer) {
        let _b: Box<dyn Fn(*const u8, MSlock)> = unsafe {
            std::mem::transmute(bx)
        };
    }

    #[no_mangle]
    extern "C" fn front_set_token_binding(bx: FatPointer, has_value: u8, value: i32) {
        let s = unsafe {
            slock_force_main_owner()
        };
        let b: Box<dyn Fn(bool, i32, MSlock)> = unsafe {
            std::mem::transmute(bx)
        };

        b(has_value != 0, value, s.marker());

        std::mem::forget(b);
    }

    #[no_mangle]
    extern "C" fn front_free_token_binding(bx: FatPointer) {
        let _b: Box<dyn Fn(bool, i32, MSlock)> = unsafe {
            std::mem::transmute(bx)
        };
    }

    #[no_mangle]
    extern "C" fn front_set_bool_binding(bx: FatPointer, value: u8) {
        let s = unsafe {
            slock_force_main_owner()
        };
        let b: Box<dyn Fn(u8, MSlock)> = unsafe {
            std::mem::transmute(bx)
        };

        b(value, s.marker());

        std::mem::forget(b);
    }

    #[no_mangle]
    extern "C" fn front_free_bool_binding(bx: FatPointer) {
        let _b: Box<dyn Fn(u8, MSlock)> = unsafe {
            std::mem::transmute(bx)
        };
    }
}

/* crate endpoints */
// FIXME use libc types at some point
pub mod global {
    use std::cell::Cell;
    use crate::core::{Slock, SlockOwner};
    use crate::native::FatPointer;
    use crate::util::marker::MainThreadMarker;

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

    #[inline]
    pub fn is_main() -> bool {
        MAIN.get()
    }

    pub fn run_main_slock_owner(f: impl FnOnce(SlockOwner<MainThreadMarker>) + Send + 'static) {
        let b: Box<dyn FnOnce(SlockOwner<MainThreadMarker>) + Send> = Box::new(f);
        let b = unsafe {
            std::mem::transmute(b)
        };

        unsafe {
            back_run_main(b);
        }
    }

    pub fn run_main<F>(f: F) where F: for<'a> FnOnce(Slock<'a, MainThreadMarker>) + Send + 'static {
        run_main_slock_owner(move |s| f(s.marker()))
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
    use crate::view::menu::{WindowMenu};

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
        fn back_window_set_menu(window: *mut c_void, menu: *mut c_void);
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

    pub fn window_set_menu(window: WindowHandle, menu: &mut WindowMenu, s: MSlock) {
        unsafe {
            back_window_set_menu(window as *mut c_void, menu.backing(s));
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
    use std::ffi;
    use std::ffi::{c_ulonglong, c_void};
    use crate::core::MSlock;
    use crate::native::FatPointer;
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
        fn back_view_scroll_init(
            allow_vertical: bool,
            allow_horizontal: bool,
            binding_y: FatPointer,
            binding_x: FatPointer
        ) -> *mut c_void;

        fn back_view_scroll_set_x(backing: *mut c_void, value: f64);
        fn back_view_scroll_set_y(backing: *mut c_void, value: f64);

        /* button */
        fn back_view_button_init() -> *mut c_void;
        fn back_view_button_update(view: *mut c_void, clicked: bool);

        /* dropdown */
        fn back_view_dropdown_init(binding: FatPointer) -> *mut c_void;
        fn back_view_dropdown_add(_view: *mut c_void, option: *const u8);
        fn back_view_dropdown_clear(_view: *mut c_void);
        fn back_view_dropdown_select(_view: *mut c_void, selection: *const u8) -> u8;
        fn back_view_dropdown_size(_view: *mut c_void) -> Size;

        /* text */
        fn back_text_init() -> *mut c_void;
        fn back_text_update(
            view: *mut c_void,
            str: *const u8,
            max_lines: ffi::c_int,
            bold: u8,
            italic: u8,
            underline: u8,
            strikethrough: u8,
            back: Color,
            front: Color,
            font: *const u8,
            font_size: f64
        );
        fn back_text_size(view: *mut c_void, suggested: Size) -> Size;

        /* text field */
        fn back_text_field_init(text_binding: FatPointer, focused_binding: FatPointer, token: i32) -> *mut c_void;
        fn back_text_field_focus(view: *mut c_void);
        fn back_text_field_unfocus(view: *mut c_void);
        fn back_text_field_update(
            view: *mut c_void,
            str: *const u8,
            max_lines: ffi::c_int,
            bold: u8,
            italic: u8,
            underline: u8,
            strikethrough: u8,
            back: Color,
            front: Color,
            font: *const u8,
            font_size: f64
        );
        fn back_text_field_size(view: *mut c_void, suggested: Size) -> Size;

        /* message box */
        fn back_message_box_init(title: *const u8, message: *const u8) -> *mut c_void;
        fn back_message_box_add_button(mb: *mut c_void, button_type: u8);

        // returns index that was clicked
        fn back_message_box_run(mb: *mut c_void) -> ffi::c_int;
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

        pub fn update_layer_view(
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
        use std::ffi::{c_void, CString};
        use std::os::unix::ffi::OsStrExt;
        use std::path::Path;
        use crate::core::MSlock;
        use crate::native::view::{back_view_image_init, back_view_image_size};
        use crate::util::geo::Size;

        pub fn init_image_view(path: &Path, _s: MSlock) -> *mut c_void {
            unsafe {
                let bytes = CString::new(path.as_os_str().as_bytes()).unwrap();
                back_view_image_init(bytes.as_bytes().as_ptr())
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
        use crate::native::view::{back_view_scroll_init, back_view_scroll_set_x, back_view_scroll_set_y};
        use crate::state::{Binding, Filterless, SetAction};
        use crate::util::geo::ScreenUnit;

        pub fn init_scroll_view(
            vertical: bool,
            horizontal: bool,
            binding_y: impl Binding<Filterless<ScreenUnit>>,
            binding_x: impl Binding<Filterless<ScreenUnit>>,
            _s: MSlock
        ) -> *mut c_void {
            unsafe {
                let set_x = Box::new(move |val, s: MSlock|  {
                    binding_x.apply(SetAction::Set(val), s);
                }) as Box<dyn Fn(ScreenUnit, MSlock)>;
                let set_x = std::mem::transmute(set_x);

                let set_y= Box::new(move |val, s: MSlock|  {
                    binding_y.apply(SetAction::Set(val), s);
                }) as Box<dyn Fn(ScreenUnit, MSlock)>;
                let set_y= std::mem::transmute(set_y);

                back_view_scroll_init(vertical, horizontal, set_y, set_x)
            }
        }

        pub fn scroll_view_set_x(
            scroll: *mut c_void,
            value: f64,
            _s: MSlock
        )
        {
            unsafe {
                back_view_scroll_set_x(scroll, value)
            }
        }

        pub fn scroll_view_set_y(
            scroll: *mut c_void,
            value: f64,
            _s: MSlock
        )
        {
            unsafe {
                back_view_scroll_set_y(scroll, value)
            }
        }
    }

    pub mod button {
        use std::ffi::c_void;
        use crate::core::MSlock;
        use crate::native::view::{back_view_button_init, back_view_button_update};

        pub fn init_button_view(_s: MSlock) -> *mut c_void {
            unsafe {
                back_view_button_init()
            }
        }

        pub fn update_button_view(button: *mut c_void, clicked: bool, _s: MSlock) {
            unsafe {
                back_view_button_update(button, clicked)
            }
        }
    }

    pub mod dropdown {
        use std::ffi::{c_char, c_void, CStr, CString};
        use crate::core::MSlock;
        use crate::native::view::{back_view_dropdown_add, back_view_dropdown_clear, back_view_dropdown_init, back_view_dropdown_select, back_view_dropdown_size};
        use crate::state::{Binding, Filterless, SetAction};
        use crate::util::geo::Size;

        pub fn init_dropdown(binding: impl Binding<Filterless<Option<String>>>,  _s: MSlock) -> *mut c_void {
            unsafe {
                let action = Box::new(move |str: *const u8, s: MSlock| {
                    let str = if str.is_null() {
                        None
                    }
                    else {
                        let cstr = CStr::from_ptr(str as *const c_char);
                        Some(CString::from(cstr).into_string().unwrap())
                    };
                    binding.apply(SetAction::Set(str), s);
                }) as Box<dyn Fn(*const u8, MSlock)>;
                back_view_dropdown_init(std::mem::transmute(action))
            }
        }

        pub fn dropdown_clear(view: *mut c_void, _s: MSlock) {
            unsafe {
                back_view_dropdown_clear(view)
            }
        }

        pub fn dropdown_select(view: *mut c_void, option: Option<&str>, _s: MSlock) -> bool {
            unsafe {
                if let Some(s) = option {
                    let cstr = CString::new(s).unwrap();
                    back_view_dropdown_select(
                        view,
                        cstr.as_bytes().as_ptr()
                    ) != 0
                }
                else {
                    back_view_dropdown_select(
                        view,
                        0 as *const u8
                    ) != 0
                }
            }
        }

        pub fn dropdown_push(view: *mut c_void, option: String, _s: MSlock) {
            unsafe {
                let cstr = CString::new(option).unwrap();
                back_view_dropdown_add(view, cstr.as_bytes().as_ptr())
            }
        }

        pub fn dropdown_size(view: *mut c_void, _s: MSlock) -> Size {
            unsafe {
                back_view_dropdown_size(view)
            }
        }
    }

    pub mod text {
        use std::ffi;
        use std::ffi::{c_void, CString};
        use crate::core::{MSlock, StandardVarEnv};
        use crate::native::view::{back_text_init, back_text_size, back_text_update};
        use crate::util::geo::Size;

        pub fn text_init(_s: MSlock) -> *mut c_void {
            unsafe {
                back_text_init()
            }
        }

        pub fn text_update(view: *mut c_void, str: &str, max_lines: u32, env: &StandardVarEnv, _s: MSlock) {
            unsafe {
                let cstring = CString::new(str).unwrap();
                let cpath = env.text.font
                    .as_ref()
                    .map(|s| s.cstring());

                back_text_update(
                    view,
                    cstring.as_bytes().as_ptr(),
                    max_lines as ffi::c_int,
                    env.text.bold as u8,
                    env.text.italic as u8,
                    env.text.underline as u8,
                    env.text.strikethrough as u8,
                    env.text.backcolor,
                    env.text.color,
                    cpath.as_ref().map(|c| c.as_bytes().as_ptr()).unwrap_or(0 as *const u8),
                    env.text.size
                )
            }
        }

        pub fn text_size(view: *mut c_void, suggested: Size, _s: MSlock) -> Size {
            unsafe {
                back_text_size(view, suggested)
            }
        }
    }

    pub mod text_field {
        use std::ffi;
        use std::ffi::{c_char, c_void, CStr, CString};
        use crate::core::{MSlock, StandardVarEnv};
        use crate::native::view::{back_text_field_focus, back_text_field_init, back_text_field_size, back_text_field_unfocus, back_text_field_update};
        use crate::state::{Binding, Filterless, SetAction};
        use crate::util::geo::Size;

        pub fn text_field_init(content: impl Binding<Filterless<String>>, focused: impl Binding<Filterless<Option<i32>>>, token: i32, _s: MSlock) -> *mut c_void {
            unsafe {
                let set_text = Box::new(move |str: *const u8, s: MSlock|  {
                    let cstr = CStr::from_ptr(str as *const c_char);
                    let string = CString::from(cstr).into_string().unwrap();
                    if *content.borrow(s) != string {
                        content.apply(SetAction::Set(string), s);
                    }
                }) as Box<dyn Fn(*const u8, MSlock)>;
                let set_text = std::mem::transmute(set_text);

                let set_focused= Box::new(move |has_val, val, s: MSlock|  {
                    if has_val {
                        focused.apply(SetAction::Set(Some(val)), s);
                    }
                    else if *focused.borrow(s) == Some(val) {
                        // only free if it's active
                        focused.apply(SetAction::Set(None), s);
                    }
                }) as Box<dyn Fn(bool, i32, MSlock)>;
                let set_focused= std::mem::transmute(set_focused);

                back_text_field_init(set_text, set_focused, token)
            }
        }

        pub fn text_field_update(view: *mut c_void, str: &str, max_lines: u32, env: &StandardVarEnv, _s: MSlock) {
            unsafe {
                let cstring = CString::new(str).unwrap();
                let cpath = env.text.font
                    .as_ref()
                    .map(|s| s.cstring());

                back_text_field_update(
                    view,
                    cstring.as_bytes().as_ptr(),
                    max_lines as ffi::c_int,
                    env.text.bold as u8,
                    env.text.italic as u8,
                    env.text.underline as u8,
                    env.text.strikethrough as u8,
                    env.text.backcolor,
                    env.text.color,
                    cpath.as_ref().map(|c| c.as_bytes().as_ptr()).unwrap_or(0 as *const u8),
                    env.text.size
                )
            }
        }

        pub fn text_field_focus(view: *mut c_void, _s: MSlock) {
            unsafe {
                back_text_field_focus(view)
            }
        }

        pub fn text_field_unfocus(view: *mut c_void, _s: MSlock) {
            unsafe {
                back_text_field_unfocus(view)
            }
        }

        pub fn text_field_size(view: *mut c_void, suggested: Size, _s: MSlock) -> Size {
            unsafe {
                back_text_field_size(view, suggested)
            }
        }
    }

    pub mod message_box {
        use std::ffi::{c_void, CString};
        use crate::core::MSlock;
        use crate::native::view::{back_message_box_add_button, back_message_box_init, back_message_box_run};

        pub fn init_message_box(title: Option<String>, message: Option<String>, _s: MSlock) -> *mut c_void {
            unsafe {
                let title_cstr = title.map(|s| CString::new(s).unwrap());
                let message_cstr = message.map(|s| CString::new(s).unwrap());
                back_message_box_init(
                    title_cstr.as_ref().map(|c| c.as_bytes().as_ptr()).unwrap_or(0 as *const u8),
                    message_cstr.as_ref().map(|c| c.as_bytes().as_ptr()).unwrap_or(0 as *const u8)
                )
            }
        }

        pub fn message_box_add(mb: *mut c_void, button_type: u8, _s: MSlock) {
            unsafe {
                back_message_box_add_button(mb, button_type)
            }
        }

        // takes ownership
        pub fn message_box_run(mb: *mut c_void) -> i32 {
            unsafe {
                back_message_box_run(mb) as i32
            }
        }
    }
}

pub mod menu {
    use std::ffi::{c_void, CString};
    use crate::core::MSlock;
    use crate::native::FatPointer;

    extern "C" {
        fn back_menu_init(title: *const u8) -> *mut c_void;
        fn back_menu_add(menu: *mut c_void, item: *mut c_void);
        fn back_menu_free(menu: *mut c_void);

        // button
        fn back_menu_separator_init() -> *mut c_void;
        fn back_menu_separator_free(view: *mut c_void);
        fn back_menu_button_init(title: *const u8, key: *const u8, modifier: u8) -> *mut c_void;
        fn back_menu_button_set_title(button: *mut c_void, title: *const u8);
        fn back_menu_button_set_action(button: *mut c_void, action: FatPointer);
        fn back_menu_button_set_enabled(button: *mut c_void, enabled: u8);
        fn back_menu_button_set_submenu(button: *mut c_void, menu: *mut c_void);
        fn back_menu_button_free(button: *mut c_void);
    }


    pub fn menu_init(title: String, _s: MSlock) -> *mut c_void {
        unsafe {
            let title = CString::new(title).unwrap();
            back_menu_init(title.as_bytes().as_ptr())
        }
    }

    pub fn menu_push(menu: *mut c_void, button: *mut c_void, _s: MSlock) {
        unsafe {
            back_menu_add(menu, button);
        }
    }

    pub fn menu_free(menu: *mut c_void) {
        unsafe {
            back_menu_free(menu);
        }
    }

    pub fn separator_init(_s: MSlock) -> *mut c_void {
        unsafe {
            back_menu_separator_init()
        }
    }

    pub fn separator_free(menu: *mut c_void) {
        unsafe {
            back_menu_separator_free(menu);
        }
    }

    pub fn button_init(title: String, key: String, modifiers: u8, _s: MSlock) -> *mut c_void {
        unsafe {
            let title = CString::new(title).unwrap();
            let key = CString::new(key).unwrap();
            back_menu_button_init(title.as_bytes().as_ptr(), key.as_bytes().as_ptr(), modifiers)
        }
    }

    pub fn button_set_title(button: *mut c_void, title: String, _s: MSlock) {
        unsafe {
            let title = CString::new(title).unwrap();
            back_menu_button_set_title(button, title.as_bytes().as_ptr());
        }
    }

    pub fn button_set_action(button: *mut c_void, action: Box<dyn FnMut(MSlock)>, _s: MSlock) {
        unsafe {
            back_menu_button_set_action(button, std::mem::transmute(action));
        }
    }

    pub fn button_set_enabled(button: *mut c_void, enabled: u8, _s: MSlock) {
        unsafe {
            back_menu_button_set_enabled(button, enabled);
        }
    }

    pub fn button_set_submenu(button: *mut c_void, menu: *mut c_void, _s: MSlock) {
        unsafe {
            back_menu_button_set_submenu(button, menu);
        }
    }

    pub fn button_free(button: *mut c_void) {
        unsafe {
            back_menu_button_free(button)
        }
    }
}

pub mod file_picker {
    use std::ffi::{c_char, c_void, CStr, CString};
    use std::path::{PathBuf};
    use crate::core::MSlock;

    extern "C" {
        fn back_file_open_picker_init(allowed_mask: *const u8) -> *mut c_void;
        fn back_file_open_picker_run(op: *mut c_void) -> *const u8;
        fn back_file_open_picker_free(op: *mut c_void);

        fn back_file_save_picker_init(allowed_mask: *const u8) -> *mut c_void;
        fn back_file_save_picker_run(op: *mut c_void) -> *const u8;
        fn back_file_save_picker_free(op: *mut c_void);
    }

    pub fn open_panel_init(mask: Option<String>, _s: MSlock) -> *mut c_void {
        unsafe {
            let cstr = mask.map(|s| CString::new(s).unwrap());
            back_file_open_picker_init(cstr.as_ref().map(|c| c.as_bytes().as_ptr()).unwrap_or(0 as *const u8))
        }
    }

    pub fn open_panel_run(op: *mut c_void) -> Option<PathBuf> {
        unsafe {
            let res = back_file_open_picker_run(op);
            if res.is_null() {
                None
            }
            else {
                let c = CStr::from_ptr(res as *const c_char);
                let cstring = CString::from(c);
                Some(PathBuf::from(cstring.into_string().unwrap()))
            }
        }
    }

    pub fn open_panel_free(op: *mut c_void, _s: MSlock) {
        unsafe {
            back_file_open_picker_free(op);
        }
    }

    pub fn save_panel_init(mask: Option<String>, _s: MSlock) -> *mut c_void {
        unsafe {
            let cstr = mask.map(|s| CString::new(s).unwrap());
            back_file_save_picker_init(cstr.as_ref().map(|c| c.as_bytes().as_ptr()).unwrap_or(0 as *const u8))
        }
    }

    pub fn save_panel_run(sp: *mut c_void) -> Option<PathBuf> {
        unsafe {
            let res = back_file_save_picker_run(sp);
            if res.is_null() {
                None
            }
            else {
                let c = CStr::from_ptr(res as *const c_char);
                let cstring = CString::from(c);
                Some(PathBuf::from(cstring.into_string().unwrap()))
            }
        }
    }

    pub fn save_panel_free(sp: *mut c_void, _s: MSlock) {
        unsafe {
            back_file_save_picker_free(sp);
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