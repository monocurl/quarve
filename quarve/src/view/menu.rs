use std::ffi::c_void;
use crate::core::{MSlock};

// FIXME can use into/conversion semantics to make this more efficient
pub unsafe trait MenuItem: 'static {
    fn backing(&mut self, s: MSlock) -> *mut c_void;
}

mod window_menu {
    use std::ffi::c_void;
    use crate::core::{MSlock, StandardConstEnv};
    use crate::event::EventModifiers;
    use crate::native::menu::{menu_init, menu_push};
    use crate::view::menu::{Menu, MenuButton, MenuItem, MenuReceiver, MenuSeparator};

    pub struct WindowMenu {
        backing: *mut c_void,
        submenus: Vec<MenuButton>
    }

    impl WindowMenu {
        pub fn new() -> Self {
            WindowMenu {
                backing: 0 as *mut c_void,
                submenus: Vec::new(),
            }
        }

        pub fn standard(env: &StandardConstEnv, file: Menu, mut edit: Menu, view: Menu, help: Menu, s: MSlock) -> Self {
            edit = edit
                .push(MenuReceiver::new(&env.channels.undo_menu, "Undo", "z", EventModifiers::new().set_command(), s))
                .push(MenuReceiver::new(&env.channels.redo_menu, "Redo", "Z", EventModifiers::new().set_shift().set_command(), s))
                .push(MenuSeparator::new())
                .push(MenuReceiver::new(&env.channels.cut_menu, "Cut", "x", EventModifiers::new().set_command(), s))
                .push(MenuReceiver::new(&env.channels.copy_menu, "Copy", "c", EventModifiers::new().set_command(), s))
                .push(MenuReceiver::new(&env.channels.paste_menu, "Paste", "v", EventModifiers::new().set_command(), s))
                .push(MenuSeparator::new())
                .push(MenuReceiver::new(&env.channels.select_all_menu, "Select All", "a", EventModifiers::new().set_command(), s));

            WindowMenu::new()
                .push(file)
                .push(edit)
                .push(view)
                .push(help)
        }

        pub fn push(mut self, menu: Menu) -> Self {
            self.submenus.push(
                MenuButton::new(menu.name.clone(), "", EventModifiers::new(), |_| {})
                    .submenu(menu)
            );
            self
        }

        pub(crate) fn backing(&mut self, s: MSlock) -> *mut c_void {
            let ours = menu_init("".to_owned(), s);
            for sm in &mut self.submenus {
                menu_push(ours, sm.backing(s), s);
            }
            self.backing = ours;
            self.backing
        }
    }
}
pub use window_menu::*;

mod menu {
    use std::ffi::c_void;
    use crate::core::MSlock;
    use crate::native::menu::{menu_free, menu_init, menu_push};
    use crate::view::menu::MenuItem;

    pub struct Menu {
        pub(super) name: String,
        items: Vec<Box<dyn MenuItem>>,
        backing: *mut c_void
    }

    impl Menu {
        pub fn new(name: impl Into<String>) -> Self {
            let name = name.into();

            Menu {
                name,
                items: Vec::new(),
                backing: 0 as *mut c_void
            }
        }

        pub fn push(mut self, item: impl MenuItem + 'static) -> Self {
            self.items.push(Box::new(item));
            self
        }

        pub(crate) fn backing(&mut self, s: MSlock) -> *mut c_void {
            let ours = menu_init(self.name.clone(), s);
            for sm in &mut self.items {
                menu_push(ours, sm.backing(s), s);
            }

            ours
        }
    }

    impl Drop for Menu {
        fn drop(&mut self) {
            menu_free(self.backing);
        }
    }
}
pub use menu::*;

mod menu_button_backing {
    use std::ffi::c_void;
    use crate::native::global::is_main;
    use crate::native::menu::button_free;

    pub(super) struct MenuButtonBacking {
        // for send compliance
        pub backing: *mut c_void
    }

    impl Drop for MenuButtonBacking {
        fn drop(&mut self) {
            debug_assert!(is_main());

            if !self.backing.is_null() {
                button_free(self.backing)
            }
        }
    }
}

mod menu_button {
    use std::cell::Cell;
    use std::ffi::c_void;
    use crate::core::MSlock;
    use crate::event::EventModifiers;
    use crate::native::menu::{button_init, button_set_action, button_set_submenu};
    use crate::view::menu::{Menu, MenuItem};
    use crate::view::menu::menu_button_backing::MenuButtonBacking;

    pub struct MenuButton {
        name: String,
        key: String,
        modifier: EventModifiers,
        action: Cell<Option<Box<dyn FnMut(MSlock)>>>,
        submenu: Option<Menu>,
        backing: MenuButtonBacking
    }

    impl MenuButton {
        pub fn new(named: impl Into<String>, keys: impl Into<String>, modifier: EventModifiers, action: impl FnMut(MSlock) + 'static) -> Self {
            let ret = MenuButton {
                name: named.into(),
                key: keys.into(),
                modifier,
                action: Cell::new(Some(Box::new(action))),
                submenu: None,
                backing: MenuButtonBacking {
                    backing: 0 as *mut c_void
                },
            };
            ret
        }

        pub fn submenu(mut self, menu: Menu) -> Self {
            self.submenu = Some(menu);
            self
        }
    }

    unsafe impl MenuItem for MenuButton {
        fn backing(&mut self, s: MSlock) -> *mut c_void {
            if self.backing.backing.is_null() {
                self.backing.backing = button_init(self.name.clone(), self.key.clone(), self.modifier.modifiers, s);
                button_set_action(self.backing.backing, self.action.take().unwrap(), s);
                if let Some(ref mut sm) = self.submenu {
                    button_set_submenu(self.backing.backing, sm.backing(s), s);
                }
            }
            self.backing.backing
        }
    }
}
pub use menu_button::*;

mod menu_separator {
    use std::ffi::c_void;
    use crate::core::MSlock;
    use crate::native::menu::{separator_free, separator_init};
    use crate::view::menu::MenuItem;

    pub struct MenuSeparator {
        backing: *mut c_void
    }

    impl MenuSeparator {
        pub fn new() -> MenuSeparator {
            MenuSeparator {
                backing: 0 as *mut c_void,
            }
        }
    }

    unsafe impl MenuItem for MenuSeparator {
        fn backing(&mut self, s: MSlock) -> *mut c_void {
            self.backing = separator_init(s);
            self.backing
        }
    }

    impl Drop for MenuSeparator {
        fn drop(&mut self) {
            if !self.backing.is_null() {
                separator_free(self.backing);
            }
        }
    }
}
pub use menu_separator::*;

mod menu_receiver {
    use std::ffi::c_void;
    use std::sync::Arc;
    use crate::core::MSlock;
    use crate::event::EventModifiers;
    use crate::native::menu::{button_init, button_set_enabled};
    use crate::state::slock_cell::{MainSlockCell};
    use crate::view::menu::menu_button_backing::MenuButtonBacking;
    use crate::view::menu::{MenuChannel, MenuItem};

    pub(super) struct MenuReceiverInner {
        pub backing: MenuButtonBacking,
        pub default_name: String,
        pub key: String,
        pub modifiers: EventModifiers,
        pub currently_set: bool
    }

    pub struct MenuReceiver {
        inner: Arc<MainSlockCell<MenuReceiverInner>>,
    }

    impl MenuReceiver {
        pub fn new(channel: &MenuChannel, name: impl Into<String>, keys: impl Into<String>, modifiers: EventModifiers, s: MSlock) -> Self {
            let inner = Arc::new(MainSlockCell::new_main(MenuReceiverInner {
                backing: MenuButtonBacking { backing: 0 as *mut c_void },
                default_name: name.into(),
                key: keys.into(),
                modifiers,
                currently_set: false,
            }, s));
            assert!(channel.receiver.borrow(s).receiver.is_none(), "Channel already in used");
            channel.receiver.borrow_mut(s).receiver = Some(Arc::downgrade(&inner));

            MenuReceiver {
                inner,
            }
        }
    }

    unsafe impl MenuItem for MenuReceiver {
        fn backing(&mut self, s: MSlock) -> *mut c_void {
            let mut borrow = self.inner.borrow_mut_main(s);
            let backing = button_init(borrow.default_name.clone(), borrow.key.clone(), borrow.modifiers.modifiers, s);
            button_set_enabled(backing, 0, s);
            borrow.backing.backing = backing;
            backing
        }
    }
}
pub use menu_receiver::*;

mod menu_channel {
    use std::sync::{Arc, Weak};
    use crate::core::{MSlock};
    use crate::native::menu::{button_set_action, button_set_enabled, button_set_title};
    use crate::state::slock_cell::{MainSlockCell, SlockCell};
    use crate::view::menu::menu_receiver::MenuReceiverInner;

    pub(super) struct MenuChannelInner {
        pub receiver: Option<Weak<MainSlockCell<MenuReceiverInner>>>
    }

    pub struct MenuChannel {
        pub(super) receiver: Arc<SlockCell<MenuChannelInner>>
    }

    impl MenuChannel {
        pub fn new() -> MenuChannel {
            MenuChannel {
                receiver: Arc::new(SlockCell::new(MenuChannelInner {
                    receiver: None,
                }))
            }
        }

        pub(crate) fn clone(&self) -> MenuChannel {
            MenuChannel {
                receiver: self.receiver.clone()
            }
        }

        pub fn set(&self, action: Box<dyn FnMut(MSlock)>, title: Option<String>, s: MSlock) {
            let inner = self.receiver.borrow(s);

            {
                let upgraded = inner.receiver.as_ref().and_then(|i| i.upgrade())
                    .expect("Menu Channels should have a sole active receiver in place at all times. \
                              There are currently none mounted for this channel!");
                let mut borrow = upgraded.borrow_mut_main(s);
                // assert!(!borrow.currently_set, "MenuChannel already mounted!");
                borrow.currently_set = true;
                button_set_title(borrow.backing.backing, title.unwrap_or_else(|| borrow.default_name.clone()), s);
                button_set_enabled(borrow.backing.backing, 1, s);
                button_set_action(borrow.backing.backing, action, s);
            }
        }

        pub fn is_set(&self, s: MSlock) -> bool {
            let inner = self.receiver.borrow(s);
            inner.receiver
                .as_ref().and_then(|i| i.upgrade())
                .map(|a| a.borrow_mut_main(s).currently_set)
                .unwrap_or(false)
        }

        pub fn unset(&self, s: MSlock) {
            let inner = self.receiver.borrow(s);

            {
                let upgraded = inner.receiver.as_ref().and_then(|i| i.upgrade())
                    .expect("Menu Channels should have a sole active receiver in place at all times. \
                              There are currently none mounted for this channel!");
                let mut borrow = upgraded.borrow_mut_main(s);
                // assert!(borrow.currently_set, "MenuChannel not currently mounted!");
                borrow.currently_set = false;
                button_set_enabled(borrow.backing.backing, 0, s);
                button_set_title(borrow.backing.backing, borrow.default_name.clone(), s);
            }
        }
    }
}
pub use menu_channel::*;

mod menu_sender {
    use crate::core::{Environment, MSlock};
    use crate::view::{IntoViewProvider};
    use crate::view::menu::{MenuChannel};

    pub trait MenuSend<E>: IntoViewProvider<E> where E: Environment {
        // FIXME we can make this not have to be clone
        // by making the native menu return the action whenever it's unset
        fn menu_send(self, on: &MenuChannel, action: impl FnMut(MSlock) + Clone + 'static) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
    }

    impl<E, I> MenuSend<E> for I where E: Environment, I: IntoViewProvider<E> {
        fn menu_send(self, on: &MenuChannel, action: impl FnMut(MSlock) + Clone + 'static) -> impl IntoViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            let mut mc1 = on.clone();
            let mut mc2 = on.clone();
            self
                .pre_show(move |s| {
                    mc1.set(Box::new(action.clone()), None, s)
                })
                .pre_hide(move |s| {
                    mc2.unset(s)
                })
        }
    }
}
pub use menu_sender::*;