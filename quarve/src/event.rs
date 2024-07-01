use std::ffi::c_void;
use crate::util::geo::{Point, ScreenUnit};

#[derive(Copy, Clone)]
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

#[derive(Copy, Clone)]
pub enum Key {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Zero, One, Two, Three, Four, Five, Six, Seven, Eight, Nine,
    Backquote, Tilde, ExclamationMark, At, Hash, Dollar, Percent, Caret, Ampersand, Star,
    LeftParenthesis, RightParenthesis, LeftSquareBracket, RightSquareBracket,
    LeftFlowerBracket, RightFlowerBracket, Backslash, Pipe,
    Minus, Underscore, Equals, Plus,
    Period, Comma, LessThan, GreaterThan, Slash, QuestionMark,
    SemiColon, Colon, SingleQuote, DoubleQuote,

    Tab, Esc, Delete, Backspace, Enter, Shift,
    Function, AltOption, Control, Command
}

#[derive(Copy, Clone)]
pub enum KeyEvent {
    Press(Key),
    Repeat(Key),
    Release(Key),
}

#[derive(Copy, Clone)]
pub enum EventPayload {
    Mouse(MouseEvent),
    Key(KeyEvent)
}

const COMMAND: u8 = 1 << 0;
const CONTROL: u8 = 1 << 1;
const SHIFT: u8 = 1 << 2;
const FN: u8 = 1 << 3;
const ALT_OPTION: u8 = 1 << 4;

#[derive(Copy, Clone)]
pub struct EventModifiers {
    pub(crate) modifiers: u8
}

impl EventModifiers {
    pub fn command(self) -> bool {
        self.modifiers & COMMAND != 0
    }

    pub fn control(self) -> bool {
        self.modifiers & CONTROL != 0
    }

    pub fn alt_or_option(self) -> bool {
        self.modifiers & ALT_OPTION != 0
    }

    pub fn shift(self) -> bool {
        self.modifiers & SHIFT != 0
    }

    pub fn function(self) -> bool {
        self.modifiers & FN != 0
    }
}

#[derive(Copy, Clone)]
pub struct Event {
    pub payload: EventPayload,
    pub modifiers: EventModifiers,
    pub cursor: Point,
    pub(crate) native_event: *mut c_void
}

impl Event {
    pub fn is_mouse(&self) -> bool {
        matches!(self.payload, EventPayload::Mouse(_))
    }
}

#[derive(Copy, Clone)]
pub enum EventResult {
    NotHandled,
    Handled,
    FocusAcquire,
    FocusRelease
}