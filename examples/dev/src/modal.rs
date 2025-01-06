use quarve::view::modal::{MessageBox, MessageBoxButton, OpenFilePicker};

pub fn message_box_ex() {
    MessageBox::new(Some("Confirm Deletion"), None)
        .button(MessageBoxButton::Cancel)
        .button(MessageBoxButton::Delete)
        .run(|b, s| {
           println!("Pressed {:?}", b);
        });
}

pub fn open_file_ex() {
    OpenFilePicker::new()
        .content_types("png")
        .run(|path, s| {
            println!("Selected {:?}", path);
        })
}