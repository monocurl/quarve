mod file_picker {
    use std::path::PathBuf;

    pub struct SavePicker<'a> {
        file_type: &'a str
    }

    impl<'a> SavePicker<'a> {
        pub fn new(content_types: &'a str) -> Self {
            Self {
                file_type: content_types,
            }
        }

        pub fn run(self) -> Option<PathBuf> {

            None
        }
    }

    pub struct OpenPicker<'a> {
        file_type: &'a str
    }

    impl<'a> OpenPicker<'a> {
        // Separated by pipe
        pub fn new(content_types: &'a str) -> Self {
            Self {
                file_type: content_types,
            }
        }

        pub fn run(self) -> Option<PathBuf> {
            None
        }
    }
}
pub use file_picker::*;

mod message_box {
    pub enum ButtonType {
        Ok = 1,
        Cancel = 2
    }

    pub struct MessageBox<'a, 'b> {
        title: Option<&'a str>,
        message: Option<&'b str>,
        buttons: Vec<ButtonType>
    }

    impl<'a, 'b> MessageBox<'a, 'b> {
        pub fn new(title: Option<&'a str>, message: Option<&'b str>) -> Self {
            Self {
                title,
                message,
                buttons: Vec::new()
            }
        }

        pub fn button(mut self, button: ButtonType) -> Self {
            self.buttons.push(button);
            self
        }

        pub fn run(self) -> ButtonType {
            let buttons = if self.buttons.is_empty() {
                Vec::new()
            }
            else {
                self.buttons
            };

            todo!()
        }
    }
}