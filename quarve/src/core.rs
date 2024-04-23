use std::cell::{OnceCell};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::sync::mpsc::{Receiver, sync_channel, SyncSender};
use std::time::Duration;

use crate::state::{ActionFilter, FixedSignal, JoinedSignal, Signal, State, Stateful};

const ANIMATION_THREAD_TICK: Duration = Duration::from_nanos(1_000_000_000 / 60);

static GLOBAL_STATE_LOCK: Mutex<()> = Mutex::new(());
/* must only be referenced from the main thread */
static TIMER_WORKER: OnceLock<SyncSender<Box<dyn FnMut() -> bool + Send>>> = OnceLock::new();

pub trait ChannelProvider: Send + 'static {

}

pub struct Slock {
    guard: MutexGuard<'static, ()>,
    unsync_unsend: PhantomData<*const i32>
}

impl Slock {
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


fn timer_worker(receiver: Receiver<Box<dyn FnMut() -> bool + Send>>) {
    let mut subscribers: Vec<Box<dyn FnMut() -> bool>> = Vec::new();

    loop {
        while let Ok(handle) = receiver.try_recv() {
            subscribers.push(handle);
        }

        if !subscribers.is_empty() {
            subscribers.retain_mut(|f| f());
            std::thread::sleep(ANIMATION_THREAD_TICK);
        }
        else {
            // if no subscribers, wait until a subscriber comes
            match receiver.recv() {
                Ok(handle) => {
                    subscribers.push(handle);
                }
                Err(_) => break
            }
        }
    }
}

struct Window<A: ApplicationProvider, P: WindowProvider> {
    marker: PhantomData<&'static A>,
    provider: P,
    channels: P::WindowChannels
}

impl<A: ApplicationProvider, P: WindowProvider> Window<A, P> {
    fn new(provider: P) -> Self {
        let channels = provider.channels();
        Window {
            marker: PhantomData,
            provider,
            channels
        }
    }
}

struct Application<P: ApplicationProvider> {
    provider: P,
    channels: P::ApplicationChannels,
}


#[cfg(target_os = "macos")]
extern "C" {
    fn main_loop();
}

impl<P: ApplicationProvider> Application<P> {
    fn new(provider: P) -> Self {
        let channels = provider.channels();
        Application {
            provider,
            channels,
        }
    }

    #[cfg(target_os = "macos")]
    fn platform_run(&self) {
        unsafe {
            main_loop();
        }
    }

    // start the run loop
    fn run(&self) {
        let (sender, receiver) = sync_channel(5);
        /* join handle not needed */
        let _ = std::thread::spawn(move || {
            timer_worker(receiver)
        });

        TIMER_WORKER.set(sender).unwrap();

        self.platform_run();
    }
}

pub trait WindowProvider: 'static {
    type WindowChannels: ChannelProvider;
    fn channels(&self) -> Self::WindowChannels;

    fn title(&self);

    fn style(&self);

    fn menu_bar(&self);

    fn tree(&self);
}

pub trait ApplicationProvider: 'static {
    type ApplicationChannels: ChannelProvider;

    /// Will only be called on the main thread
    fn channels(&self) -> Self::ApplicationChannels;

    /// Will only be called on the main thread
    fn initial_window(&self, app_c: &Self::ApplicationChannels) -> impl WindowProvider;
}

fn timer_subscriber(func: Box<dyn FnMut() -> bool + Send>) {
    TIMER_WORKER.get()
        .expect("Cannot call quarve functions before launch!")
        .send(func)
        .unwrap()
}

pub fn launch(provider: impl ApplicationProvider) {
    // if let Some(_) = APP.replace(None) {
    //     panic!("You cannot launch an app multiple times");
    // }

    let app = Application::new(provider);
    app.run();
}

/// Returns an owned copy of the context lock
/// Whichever thread owns the context object is allowed to do writes/reads to the state
/// tree
pub fn slock() -> Slock {
    Slock {
        guard: GLOBAL_STATE_LOCK.lock().expect("Unable to lock context"),
        unsync_unsend: PhantomData
    }
}
