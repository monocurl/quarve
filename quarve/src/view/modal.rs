mod file_picker {
    use std::path::PathBuf;
    use crate::core::{MSlock, slock_main_owner};
    use crate::native::file_picker::{open_panel_free, open_panel_init, open_panel_run, save_panel_free, save_panel_init, save_panel_run};
    use crate::native::global::run_main_slock_owner;

    pub struct SaveFilePicker<'a> {
        file_type: Option<&'a str>
    }

    impl<'a> SaveFilePicker<'a> {
        pub fn new() -> Self {
            Self {
                file_type: None
            }
        }

        pub fn content_types(mut self, types: &'a str) -> Self {
            self.file_type = Some(types);
            self
        }

        pub fn run(self, callback: impl FnOnce(Option<PathBuf>, MSlock) + Send + 'static) {
            let file_type = self.file_type.map(|s| s.to_string());
            run_main_slock_owner(move |so| {
                let modal = save_panel_init(file_type, so.marker());
                drop(so);
                let res = save_panel_run(modal);
                let so = slock_main_owner();
                save_panel_free(modal, so.marker());
                callback(res, so.marker());
            })
        }
    }

    pub struct OpenFilePicker<'a> {
        file_type: Option<&'a str>
    }

    impl<'a> OpenFilePicker<'a> {
        pub fn new() -> Self {
            Self {
                file_type: None
            }
        }

        pub fn content_types(mut self, types: &'a str) -> Self {
            self.file_type = Some(types);
            self
        }

        pub fn run(self, callback: impl FnOnce(Option<PathBuf>, MSlock) + Send + 'static) {
            let file_type = self.file_type.map(|s| s.to_string());
            run_main_slock_owner(move |so| {
                let modal = open_panel_init(file_type, so.marker());
                drop(so);
                let res = open_panel_run(modal);
                let so = slock_main_owner();
                open_panel_free(modal, so.marker());
                callback(res, so.marker());
            })
        }
    }
}
pub use file_picker::*;

mod message_box {
    use crate::core::{MSlock, slock_main_owner};
    use crate::native::global::run_main_slock_owner;
    use crate::native::view::message_box::{init_message_box, message_box_add, message_box_run};

    #[derive(Copy, Clone, Debug)]
    #[repr(u8)]
    pub enum MessageBoxButton {
        Ok = 1,
        Cancel = 2,
        Delete = 3
    }

    pub struct MessageBox<'a, 'b> {
        title: Option<&'a str>,
        message: Option<&'b str>,
        buttons: Vec<MessageBoxButton>
    }

    impl<'a, 'b> MessageBox<'a, 'b> {
        /// NOTE: at this time you may receive warnings about
        /// acquiring the state lock for too long. We are looking into better options
        pub fn new(title: Option<&'a str>, message: Option<&'b str>) -> Self {
            Self {
                title,
                message,
                buttons: Vec::new()
            }
        }

        pub fn button(mut self, button: MessageBoxButton) -> Self {
            self.buttons.push(button);
            self
        }

        pub fn run(self, callback: impl FnOnce(MessageBoxButton, MSlock) + Send + 'static) {
            let buttons = if self.buttons.is_empty() {
                vec![MessageBoxButton::Ok]
            }
            else {
                self.buttons
            };

            // more than 3 rarely makes sense anyway
            assert!(buttons.len() <= 3);

            let title = self.title.map(|t| t.to_string());
            let message = self.message.map(|t| t.to_string());
            run_main_slock_owner(move |s| {
                let mb = init_message_box(title, message, s.marker());
                for button in &buttons {
                    message_box_add(mb, *button as u8, s.marker());
                }

                // don't hold it during the actual message box (which may take time)
                drop(s);
                let res = message_box_run(mb);
                // reacquire state lock for the callback
                let s = slock_main_owner();
                callback(buttons[res as usize], s.marker());
            });
        }
    }
}
pub use message_box::*;