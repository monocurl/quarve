pub enum Event {
    MouseEvent,
    KeyEvent
}

pub enum EventResult {
    FocusAcquire,
    NotHandled(Event),
    Handled,
    FocusRelease
}