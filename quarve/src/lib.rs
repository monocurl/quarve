pub mod state;
pub mod event;
pub mod channel;
pub mod view;
pub mod util;
pub mod core;
mod native;

// pub mod prelude {
//     pub use crate::core::*;
//     pub use crate::state::*;
// }

#[cfg(target_os = "macos")]
#[link(name = "Cocoa", kind = "framework")]
extern { }
