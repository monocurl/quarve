use std::ffi::c_void;
use std::ops::{Deref};
use crate::core::{APP, ApplicationProvider, Slock, slock_main_owner, WindowBase, WindowProvider};
use crate::util::markers::MainThreadMarker;

pub type WindowHandle = usize;

#[repr(C)]
struct FatPointer(*const c_void, *const c_void);

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

/* front -> back callbacks */
#[cfg(target_os = "macos")]
extern "C" {
    fn back_main_loop();

    fn back_is_main() -> bool;
    fn back_run_main(bx: FatPointer);
    fn back_register_window(handle: FatPointer) -> *mut c_void;
    fn back_exit_window(window: *mut c_void);

    fn back_terminate();
}

/* back -> front call backs */
#[no_mangle]
extern "C" fn front_will_spawn() {
    APP.with(|m| m.get().unwrap().will_spawn());
}

#[no_mangle]
extern "C" fn front_window_should_close(handle: FatPointer) -> bool {
    let s = slock_main_owner();

    handle.into_window().can_close(s.borrow())
}

#[no_mangle]
extern "C" fn front_execute_box(bx: FatPointer) {
    /* ownership taken */
    let b: Box<dyn for<'a> FnOnce(Slock<'a, MainThreadMarker>) + Send> = unsafe {
        std::mem::transmute(bx)
    };

    /* main thread only */
    let s = slock_main_owner();

    b(s.borrow());
}

/* crate endpoints */
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

pub fn run_main<F: for <'a> FnOnce(Slock<'a, MainThreadMarker>) + Send + 'static>(f: F) {
    let b: Box<dyn for <'a> FnOnce(Slock<'a, MainThreadMarker>)> = Box::new(f);
    let b = unsafe {
        std::mem::transmute(b)
    };

    unsafe {
        back_run_main(b);
    }
}

/// Makes window handle and spawns it
pub fn register_window<A, P>(dump: &Box<dyn WindowBase>)
    -> WindowHandle
    where A: ApplicationProvider,
          P: WindowProvider
{
    let raw = dump.deref();
    let b = unsafe {
        std::mem::transmute(raw)
    };
    let res = unsafe {
        back_register_window(b)
    };

    res as WindowHandle
}

pub fn exit_window(handle: WindowHandle) {
    unsafe {
        println!("Calling");
        back_exit_window(handle as *mut c_void);
    }
}

pub fn exit() {
    unsafe {
        back_terminate();
    }
}