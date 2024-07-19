pub mod state;
pub mod event;
pub mod channel;
pub mod view;
pub mod util;
pub mod core;
pub mod resource;
pub mod prelude;

/* private */
mod native;

#[cfg(target_os = "macos")]
#[link(name = "Cocoa", kind = "framework")]
#[link(name = "UniformTypeIdentifiers", kind = "framework")]
extern { }
