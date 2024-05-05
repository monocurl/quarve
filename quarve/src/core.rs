use std::cell::{OnceCell, RefCell};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::sync::mpsc::{Receiver, sync_channel, SyncSender};
use std::thread;
use std::time::Duration;
use crate::native;
use crate::native::{exit_window, register_window, WindowHandle};

use crate::state::{ActionFilter, FixedSignal, JoinedSignal, Signal, Stateful, Binding};

const ANIMATION_THREAD_TICK: Duration = Duration::from_nanos(1_000_000_000 / 60);

static GLOBAL_STATE_LOCK: Mutex<()> = Mutex::new(());
static TIMER_WORKER: OnceLock<SyncSender<Box<dyn FnMut() -> bool + Send>>> = OnceLock::new();

thread_local! {
    pub(crate) static APP: OnceCell<Box<dyn ApplicationBase>> = OnceCell::new();
}

pub trait ChannelProvider: 'static { }

mod sealed {
    pub trait ThreadMarkerBase {

    }
}
pub trait ThreadMarker: sealed::ThreadMarkerBase {}
pub struct AnyThreadMarker;
impl sealed::ThreadMarkerBase for AnyThreadMarker {}
impl ThreadMarker for AnyThreadMarker {}
pub struct MainThreadMarker;
impl sealed::ThreadMarkerBase for MainThreadMarker {}
impl ThreadMarker for MainThreadMarker {}

pub struct Slock<M: ThreadMarker=AnyThreadMarker> {
    _guard: MutexGuard<'static, ()>,
    unsync_unsend: PhantomData<*const ()>,
    thread_marker: PhantomData<M>
}

impl<M: ThreadMarker> AsRef<Slock> for Slock<M> {
    fn as_ref(&self) -> &Slock {
        // safety: if a slock comes from a specific thread
        // it certainly came from any thread.
        // The marker itself is only a marker and is not stored
        // so that the data layouts are exactly the same
        unsafe {
            std::mem::transmute(self)
        }
    }
}

impl<M: ThreadMarker> Slock<M> {
    pub fn fixed<T: Send + 'static>(&self, val: T) -> impl Signal<T> {
        FixedSignal::new(val)
    }

    pub fn map<S, T, U, F>(&self, signal: &S, map: F) -> impl Signal<U>
        where S: Signal<T>,
              T: Send + 'static,
              U: Send + 'static,
              F: Send + 'static + Fn(&T) -> U
    {
        signal.map(map, self.as_ref())
    }

    pub fn join<T, U>(&self, t: &impl Signal<T>, u: &impl Signal<U>)
                      -> impl Signal<(T, U)>
        where T: Send + Clone + 'static,
              U: Send + Clone + 'static
    {
        JoinedSignal::from(t, u, |t, u| (t.clone(), u.clone()), self.as_ref())
    }

    pub fn join_map<T, U, V, F>(&self, t: &impl Signal<T>, u: &impl Signal<U>, map: F)
                                -> impl Signal<V>
        where T: Send + Clone + 'static,
              U: Send + Clone + 'static,
              V: Send + 'static,
              F: Send + Clone + 'static + Fn(&T, &U) -> V
    {
        JoinedSignal::from(t, u, map, self.as_ref())
    }

    pub fn apply<S, F>(&self, action: S::Action, to: &impl Binding<S, F>)
        where S: Stateful, F: ActionFilter<S>
    {
        to.apply(action, self.as_ref());
    }

    pub fn read<'a, T>(&'a self, from: &'a impl Signal<T>)
        -> impl Deref<Target=T> + 'a where T: Send + 'static {
        from.borrow(self.as_ref())
    }
}

fn timer_worker(receiver: Receiver<Box<dyn FnMut() -> bool + Send>>) {
    let mut subscribers: Vec<Box<dyn FnMut() -> bool>> = Vec::new();

    loop {
        let start_time = std::time::SystemTime::now();

        while let Ok(handle) = receiver.try_recv() {
            subscribers.push(handle);
        }

        if !subscribers.is_empty() {
            subscribers.retain_mut(|f| f());
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

        let curr_time = std::time::SystemTime::now();
        let passed = curr_time.duration_since(start_time).unwrap();
        if passed < ANIMATION_THREAD_TICK {
            std::thread::sleep(ANIMATION_THREAD_TICK - passed);
        }
    }
}


pub(crate) trait WindowBase {
    /* delegate methods */
    fn can_close(&self, s: &Slock<MainThreadMarker>) -> bool;

    fn set_handle(&mut self, handle: WindowHandle);

    fn get_handle(&self) -> WindowHandle;
}

pub struct Window<A: ApplicationProvider, P: WindowProvider> {
    app: &'static Application<A>,
    marker: PhantomData<A>,
    provider: P,
    channels: P::WindowChannels,
    handle: WindowHandle
}

impl<A: ApplicationProvider, P: WindowProvider> WindowBase for Window<A, P> {
    fn can_close(&self, s: &Slock<MainThreadMarker>) -> bool {
        self.provider.can_close(s)
    }

    fn set_handle(&mut self, handle: WindowHandle) {
        self.handle = handle
    }

    fn get_handle(&self) -> WindowHandle {
        self.handle
    }
}

impl<A: ApplicationProvider, P: WindowProvider> Window<A, P> {
    pub fn app(&self) -> AppHandle<A> {
        AppHandle {
            handle: self.app
        }
    }

    pub fn exit(&self, _s: &Slock<MainThreadMarker>) {
        /* remove from application window list */

        // main thread guaranteed
        self.app.windows
            .borrow_mut()
            .retain(|window| window.get_handle() == self.handle);

        exit_window(self.handle)
    }
}

pub(crate) trait ApplicationBase {
    /* delegate methods (main thread only!) */
    fn run(&self);

    fn will_spawn(&self);
}

struct Application<P: ApplicationProvider> {
    provider: P,
    channels: P::ApplicationChannels,
    windows: RefCell<Vec<Box<dyn WindowBase>>>
}

pub struct AppHandle<P: ApplicationProvider> {
    handle: &'static Application<P>
}

impl<A: ApplicationProvider> ApplicationBase for Application<A> {
    fn run(&self) {
        let (sender, receiver) = sync_channel(5);
        /* join handle not needed */
        let _ = std::thread::spawn(move || {
            timer_worker(receiver)
        });

        TIMER_WORKER.set(sender).expect("Application should only be run once");

        /* run app */
        native::main_loop();
    }

    fn will_spawn(&self) {
        let slock = unsafe {
            slock_main()
        };

        self.provider.will_spawn(self.handle(), &slock);
    }
}

impl<P: ApplicationProvider> Application<P> {
    fn new(provider: P) -> Self {
        let channels = provider.channels();
        Application {
            provider,
            channels,
            windows: RefCell::new(Vec::new())
        }
    }

    #[inline]
    fn handle(&self) -> AppHandle<P> {
        // safety: the one and only application reference
        // is static
        AppHandle {
            handle: unsafe {
                std::mem::transmute(self)
            }
        }
    }
}

impl<P: ApplicationProvider> AppHandle<P> {
    pub fn spawn_window<W: WindowProvider>(&self, provider: W, _s: &Slock<MainThreadMarker>) {
        let channels = provider.channels();

        let window = Window {
            app: self.handle,
            marker: PhantomData::<P>,
            provider,
            channels,
            handle: 0
        };

        let mut b: Box<dyn WindowBase> = Box::new(window);
        /* show window */
        b.set_handle(register_window::<P, W>(&b));
        self.handle.windows.borrow_mut().push(b);
    }

    pub fn exit(&self, _s: &Slock<MainThreadMarker>) {
        native::exit();
    }
}

// safety: all methods require Slock<Main> which means
// that when accessed, nothing is actually on a separate thread
unsafe impl<P: ApplicationProvider> Send for AppHandle<P> { }
unsafe impl<P: ApplicationProvider> Sync for AppHandle<P> { }

pub trait WindowProvider: 'static {
    type WindowChannels: ChannelProvider;
    fn channels(&self) -> Self::WindowChannels;

    fn title(&self, s: &Slock<MainThreadMarker>);

    fn style(&self, s: &Slock<MainThreadMarker>);

    fn menu_bar(&self, s: &Slock<MainThreadMarker>);

    fn tree(&self, s: &Slock<MainThreadMarker>);

    fn can_close(&self, _s: &Slock<MainThreadMarker>) -> bool {
        true
    }
}

pub trait ApplicationProvider: Sized + 'static {
    type ApplicationChannels: ChannelProvider;

    /// Will only be called on the main thread
    fn channels(&self) -> Self::ApplicationChannels;

    fn will_spawn(&self, app: AppHandle<Self>, s: &Slock<MainThreadMarker>);
}

fn timer_subscriber(func: Box<dyn FnMut() -> bool + Send>) {
    TIMER_WORKER.get()
        .expect("Cannot call quarve functions before launch!")
        .send(func)
        .unwrap()
}


/// Must be called from the main thread in the main function
pub fn launch(provider: impl ApplicationProvider) {
    if let Err(_) = APP.with(|m| m.set(Box::new(Application::new(provider)))) {
        panic!("Cannot launch an app multiple times");
    }

    APP.with(|m| m.get().unwrap().run());
}


/// Asynchronously runs a task on the main thread
pub fn run_main<F: FnOnce(&Slock<MainThreadMarker>) + Send +'static>(f: F) {
    native::run_main(f)
}

fn global_guard() -> MutexGuard<'static, ()> {
    static LOCKED_THREAD: Mutex<Option<thread::ThreadId>> = Mutex::new(None);

    #[cfg(debug_assertions)]
    {
        let lock = GLOBAL_STATE_LOCK.try_lock();
        let ret = if let Ok(lock) = lock {
            lock
        }
        else {
            if *LOCKED_THREAD.lock().unwrap() == Some(thread::current().id()) {
                panic!("Attempted to acquire state lock when the current thread already has the state lock. \
                        In production, this will result in a deadlock! \
                        Instead of acquiring the slock multiple times, pass around the Slock by reference."
                )
            }
            GLOBAL_STATE_LOCK.lock().expect("Unable to lock context")
        };

        *LOCKED_THREAD.lock().unwrap() = Some(thread::current().id());

        ret
    }

    #[cfg(not(debug_assertions))]
    {
        GLOBAL_STATE_LOCK.lock().expect("Unable to lock context")
    }
}

/// Returns an owned copy of the context lock
/// Whichever thread owns the context object is allowed to do writes/reads to the state
/// tree
pub fn slock() -> Slock {
    Slock {
        _guard: global_guard(),
        unsync_unsend: PhantomData,
        thread_marker: PhantomData
    }
}

pub(crate) unsafe fn slock_main() -> Slock<MainThreadMarker> {
    Slock {
        _guard: global_guard(),
        unsync_unsend: PhantomData,
        thread_marker: PhantomData
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use crate::core::slock;

    /* of course, should only panic in debug scenarios */
    #[test]
    #[should_panic]
    fn recursive_lock() {
        let _s = slock();
        let _s2 = slock();
    }

    /* no panic test */
    #[test]
    fn no_panic_test() {
        let s = slock();
        let res = std::thread::spawn(|| {
            let _s = slock();

            return 1;
        });

        std::thread::sleep(Duration::from_millis(100));
        drop(s);

        assert_eq!(res.join().unwrap(), 1);
    }
}