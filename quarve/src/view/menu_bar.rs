use crate::core::MSlock;

pub struct MenuChannel {

}

pub struct Menu {

}

pub struct MenuGroup {

}

impl Menu {
    pub fn new() -> Self {
        Menu {

        }
    }

    pub fn push() {

    }
}

// Unstable Interface
pub trait MenuItem {
    fn is_enabled(&self) -> bool;
    fn perform(&self);
}

pub struct MenuButton<F> where F: FnMut(MSlock) {
    name: String,
    action: F
}

pub struct MenuReceiver {

}



pub trait MenuSender {
    fn menu_send(i: i32) -> i32;
}