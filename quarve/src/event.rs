pub enum Event {
    MouseEvent,

}

pub enum EventResult {
    FocusAcquire,
    NotHandled(Event),
    FocusRelease
}