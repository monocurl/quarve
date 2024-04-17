use std::any::Any;
use std::cell::{OnceCell, Ref, RefCell};
use std::iter::Once;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Mutex, MutexGuard};
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

use crate::state::{ActionFilter, FixedSignal, JoinedSignal, Signal, State, Stateful};

static GLOBAL_STATE_LOCK: Mutex<()> = Mutex::new(());

pub trait ChannelProvider: Send + 'static {

}

pub struct Slock {
    guard: MutexGuard<'static, ()>,
    unsync_unsend: PhantomData<*const i32>
}

impl Slock {
    // pub fn channels(&self) -> Ref<C> {
    //     self.channels.borrow()
    // }
    //
    // pub fn channels_mut(&self) {
    //
    // }

    pub fn fixed<T: Send + 'static>(&self, val: T) -> impl Signal<T> {
        FixedSignal::new(val)
    }

    pub fn map<S, T, U, F>(&self, signal: &S, map: F) -> impl Signal<U>
        where S: Signal<T>,
              T: Send + 'static,
              U: Send + 'static,
              F: Send + 'static + Fn(&T) -> U
    {
        signal.map(map, self)
    }

    pub fn join<T, U>(&self, t: &impl Signal<T>, u: &impl Signal<U>)
        -> impl Signal<(T, U)>
        where T: Send + Clone + 'static,
              U: Send + Clone + 'static
    {
        JoinedSignal::from(t, u, |t, u| (t.clone(), u.clone()), self)
    }

    pub fn join_map<T, U, V, F>(&self, t: &impl Signal<T>, u: &impl Signal<U>, map: F)
                      -> impl Signal<V>
        where T: Send + Clone + 'static,
              U: Send + Clone + 'static,
              V: Send + 'static,
              F: Send + Clone + 'static + Fn(&T, &U) -> V
    {
        JoinedSignal::from(t, u, map, self)
    }

    pub fn apply<S, F>(&self, action: S::Action, to: &State<S, F>)
        where S: Stateful, F: ActionFilter<S>
    {
        to.apply(action, self);
    }

    pub fn read<'a, T: Send + 'static>(&'a self, from: &'a impl Signal<T>) -> impl Deref<Target=T> + 'a {
        from.borrow(self)
    }
}

struct QuarveChannels {
    // sheets channel (?)
    // popover channel (?)
    // animation worker channel
}

/// Returns an owned copy of the context lock
/// Whichever thread owns the context object is allowed to do writes/reads to the state
/// tree
pub fn slock() -> Slock {
    Slock {
        guard: GLOBAL_STATE_LOCK.lock().expect("Unable to lock context"),
        // channels: PhantomData,
        unsync_unsend: PhantomData
    }
}

fn animation_worker(receiver: Receiver<i32>) {
    loop {
        match receiver.try_recv() {
            Ok(handle) => {

            }
            Err(_) => break
        }

        // if no subscribers, wait until a subscriber comes
    }
}

struct Application<C: ChannelProvider> {
    animation_thread: JoinHandle<()>,
    channel_marker: PhantomData<&'static C>,
}

impl<C: ChannelProvider> Application<C> {
    fn new(channels: C) -> Self {
        /* register global context */

        Application {
            animation_thread: std::thread::spawn(|| {
                // animation_worker()
            }),
            channel_marker: PhantomData
        }
    }

    fn run(&self) {

    }
}

pub trait WindowProvider {
    type WindowChannels: ChannelProvider;
    fn channels() -> Self::WindowChannels;

    fn style(&self);
    fn menu_bar(&self);

    fn tree(&self);
}

pub trait ApplicationProvider {
    type ApplicationChannels: ChannelProvider;
    fn channels() -> Self::ApplicationChannels;
}

pub fn launch<C: ChannelProvider>(channels: C) {
    Application::new(channels).run();
}
