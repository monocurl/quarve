pub mod state;
pub mod event;
pub mod core;
pub mod channel;
pub mod view;
pub mod util;

pub use core::*;

#[cfg(target_os = "macos")]
#[link(name = "Cocoa", kind = "framework")]
extern { }


extern "C" {
    fn main_loop();
}

pub fn run() {
    unsafe {
        main_loop();
    }
}
