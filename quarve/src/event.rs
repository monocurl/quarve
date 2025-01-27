use std::ffi::c_void;

use crate::util::geo::{Point, ScreenUnit};

#[derive(Copy, Clone, Debug)]
pub enum MouseEvent {
    Scroll(ScreenUnit, ScreenUnit),
    LeftDown,
    LeftDrag(ScreenUnit, ScreenUnit),
    LeftUp,
    RightDown,
    RightDrag(ScreenUnit, ScreenUnit),
    RightUp,
    Move(ScreenUnit, ScreenUnit),
}

#[derive(Clone, Debug)]
pub struct Key(String);

impl Key {
    pub fn new(characters: String) -> Key {
        Key(characters)
    }

    pub fn chars(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub enum KeyEvent {
    Press(Key),
    Repeat(Key),
    Release(Key),
}

#[derive(Clone, Debug)]
pub enum EventPayload {
    Mouse(MouseEvent, Point),
    Key(KeyEvent)
}

const COMMAND: u8 = 1 << 0;
const CONTROL: u8 = 1 << 1;
const SHIFT: u8 = 1 << 2;
const FN: u8 = 1 << 3;
const ALT_OPTION: u8 = 1 << 4;

#[derive(Copy, Clone, Debug, Default)]
pub struct EventModifiers {
    pub(crate) modifiers: u8
}

impl EventModifiers {
    pub fn new() -> Self {
        EventModifiers {
            modifiers: 0,
        }
    }

    pub fn set_command(mut self) -> Self {
        self.modifiers |= COMMAND;
        self
    }

    pub fn command(self) -> bool {
        self.modifiers & COMMAND != 0
    }

    pub fn set_control(mut self) -> Self {
        self.modifiers |= CONTROL;
        self
    }

    pub fn control(self) -> bool {
        self.modifiers & CONTROL != 0
    }

    pub fn set_alt_or_option(mut self) -> Self {
        self.modifiers |= ALT_OPTION;
        self
    }

    pub fn alt_or_option(self) -> bool {
        self.modifiers & ALT_OPTION != 0
    }

    pub fn set_shift(mut self) -> Self {
        self.modifiers |= SHIFT;
        self
    }

    pub fn shift(self) -> bool {
        self.modifiers & SHIFT != 0
    }

    pub fn set_function(mut self) -> Self {
        self.modifiers |= FN;
        self
    }

    pub fn function(self) -> bool {
        self.modifiers & FN != 0
    }
}

#[derive(Clone, Debug)]
pub struct Event {
    // on the initial mouse event
    // this will be true
    // on the secondary mouse event
    // this will be false, even if the target node
    // happens to be the focused node
    pub for_focused: bool,
    pub payload: EventPayload,
    pub modifiers: EventModifiers,
    pub(crate) _native_event: *mut c_void
}

impl Event {
    pub fn is_mouse(&self) -> bool {
        matches!(self.payload, EventPayload::Mouse(_, _))
    }

    pub fn chars(&self) -> Option<&str> {
        if let EventPayload::Key(ref ke)  = self.payload {
            Some(match ke {
                KeyEvent::Press(k) => k.chars(),
                KeyEvent::Repeat(k) => k.chars(),
                KeyEvent::Release(k) => k.chars(),
            })
        }
        else {
            None
        }
    }

    pub fn cursor(&self) -> Point {
        match self.payload {
            EventPayload::Mouse(_, at) => at,
            EventPayload::Key(_) => panic!("Must only be accessed on mouse events")
        }
    }


    pub fn set_cursor(&mut self, cursor: Point) {
        match self.payload {
            EventPayload::Mouse(_, ref mut at) => *at = cursor,
            EventPayload::Key(_) => panic!("Must only be accessed on mouse events")
        }
    }
}

#[derive(Copy, Clone)]
pub enum EventResult {
    NotHandled,
    Handled,
    FocusAcquire,
    FocusRelease
}