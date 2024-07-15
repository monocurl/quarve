use std::ffi::c_void;
use crate::core::{MSlock};
use crate::event::EventModifiers;
use crate::native::global::is_main;
use crate::native::menu::{button_free, button_init, button_set_action, menu_free, menu_init, menu_push};

pub struct Menu {
    items: Vec<Box<dyn MenuItem>>,
    pub(crate) backing: *mut c_void
}

struct MenuButtonBacking {
    backing: *mut c_void
}

pub struct MenuButton {
    submenu: Option<Menu>,
    backing: MenuButtonBacking
}

pub struct MenuChannel {
    receiver: Option<MenuReceiver>
}

pub struct MenuReceiver {
    backing: MenuButtonBacking,
    currently_set: bool,
}

pub unsafe trait MenuItem: 'static {
    fn backing(&self) -> *mut c_void;
}

impl Menu {
    // pub fn standard_root(
    //
    // ) -> Menu {
    //
    // }

    pub fn new(name: impl Into<String>, s: MSlock) -> Self {
        let name = name.into();

        Menu {
            items: Vec::new(),
            backing: menu_init(name.as_ptr(), s)
        }
    }

    pub fn push(mut self, item: impl MenuItem + 'static, s: MSlock) -> Self {
        menu_push(self.backing, item.backing(), s);
        self.items.push(Box::new(item));
        self
    }
}

impl Drop for Menu {
    fn drop(&mut self) {
        menu_free(self.backing);
    }
}

impl MenuButton {
    pub fn new(named: impl Into<String>, keys: impl Into<String>, modifier: EventModifiers, action: impl FnMut(MSlock) + 'static, s: MSlock) -> Self {
        let ret = MenuButton {
            submenu: None,
            backing: MenuButtonBacking {
                backing: button_init(named.into(), keys.into(), modifier.modifiers, s)
            },
        };
        button_set_action(ret.backing.backing, action, s);
        ret
    }

    pub fn submenu(mut self, menu: Menu) -> Self {
        self.submenu = Some(menu);
        self
    }
}

unsafe impl MenuItem for MenuButton {
    fn backing(&self) -> *mut c_void {
        self.backing.backing
    }
}

impl MenuChannel {
    pub fn set(&mut self, action: Box<impl FnMut(MSlock)>, title: Option<String>, s: MSlock) {

    }

    pub fn unset(&mut self) {

    }
}

impl Drop for MenuButtonBacking {
    fn drop(&mut self) {
        debug_assert!(is_main());

        if !self.backing.is_null() {
            button_free(self.backing)
        }
    }
}

pub trait MenuSender<E> {
    fn menu_send(self, ) -> i32;
}
