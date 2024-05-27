use std::cell::OnceCell;
use std::sync::{OnceLock};
use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};
use crate::core::application::ApplicationBase;

static TIMER_WORKER: OnceLock<SyncSender<(Box<dyn for<'a> FnMut(Duration, Slock<'a>) -> bool + Send>, Instant)>> = OnceLock::new();

thread_local! {
    pub(crate) static APP: OnceCell<Box<dyn ApplicationBase>> = OnceCell::new();
}

mod debug_stats {
    use std::cell::RefCell;
    use std::time::{Duration, Instant};

    #[cfg(debug_assertions)]
    pub(crate) struct DebugInfo {
        // addresses of applied states
        pub applying_transaction: RefCell<Vec<usize>>,
        start_time: Instant
    }

    #[cfg(not(debug_assertions))]
    struct DebugInfo {

    }

    #[cfg(debug_assertions)]
    impl DebugInfo {
        pub fn new() -> Self {
            DebugInfo {
                applying_transaction: RefCell::new(Vec::new()),
                start_time: Instant::now()
            }
        }
    }
    #[cfg(debug_assertions)]
    impl Drop for DebugInfo {
        fn drop(&mut self) {
            let hang =  Instant::now().duration_since(self.start_time);
            if hang > Duration::from_millis(200) {
                println!("quarve: state locked attained for {} milliseconds. \
            This may cause visible stalls; please try to release the state lock as soon as the transaction is complete.", hang.as_millis());
            }
        }
    }

    #[cfg(not(debug_assertions))]
    impl DebugInfo {
        fn new() -> Self {
            DebugInfo {

            }
        }
    }
}

mod life_cycle {
    use std::sync::mpsc::{Receiver, sync_channel};
    use std::thread;
    use std::time::{Duration, Instant};
    use crate::core::{slock_owner, Slock, TIMER_WORKER};

    const ANIMATION_THREAD_TICK: Duration = Duration::from_nanos(1_000_000_000 / 60);

    fn timer_worker(receiver: Receiver<(Box<dyn for <'a> FnMut(Duration, Slock<'a>) -> bool + Send>, Instant)>) {
        let mut subscribers: Vec<(Box<dyn for <'a> FnMut(Duration, Slock<'a>) -> bool + Send>, Instant)> = Vec::new();

        loop {
            let start_time = Instant::now();

            while let Ok(handle) = receiver.try_recv() {
                subscribers.push(handle);
            }

            if !subscribers.is_empty() {
                let s = slock_owner();
                subscribers.retain_mut(|(f, start) | f(start_time.duration_since(*start), s.borrow()));
            }

            if subscribers.is_empty() {
                // if no subscribers, wait until a subscriber comes
                match receiver.recv() {
                    Ok(handle) => {
                        subscribers.push(handle);
                    }
                    Err(_) => break
                }
            }

            let curr_time = Instant::now();
            let passed = curr_time.duration_since(start_time);
            if passed < ANIMATION_THREAD_TICK {
                thread::sleep(ANIMATION_THREAD_TICK - passed);
            }
        }
    }

    // may also be used in some testing code
    pub(crate) fn setup_timing_thread() {
        let (sender, receiver) = sync_channel(5);
        /* join handle not needed */
        let _ = thread::spawn(move || {
            timer_worker(receiver)
        });

        TIMER_WORKER.set(sender).expect("Application should only be run once");
    }
}
// life cycle methods only needed outside of this module
// when testing
#[cfg(test)]
pub(crate) use life_cycle::*;

mod application {
    use std::cell::RefCell;
    use crate::core::{MSlock, Slock, slock_main_owner};
    use crate::core::life_cycle::setup_timing_thread;
    use crate::core::window::{Window, WindowBase, WindowProvider};
    use crate::native;
    use crate::util::markers::MainThreadMarker;

    pub trait ChannelProvider: 'static { }

    pub trait ApplicationProvider: Sized + 'static {
        type ApplicationChannels: ChannelProvider;

        /// Guaranteed to only be called on the main thread
        fn channels(&self) -> Self::ApplicationChannels;

        fn will_spawn(&self, app: AppHandle<Self>, s: MSlock<'_>);
    }

    pub(crate) trait ApplicationBase {
        /* delegate methods (main thread only!) */
        fn run(&self);

        fn will_spawn(&self);
    }

    pub(super) struct Application<P: ApplicationProvider> {
        provider: P,
        channels: P::ApplicationChannels,
        pub(super) windows: RefCell<Vec<Box<dyn WindowBase>>>
    }

    pub struct AppHandle<P: ApplicationProvider> {
        handle: &'static Application<P>
    }

    impl<A: ApplicationProvider> ApplicationBase for Application<A> {
        fn run(&self) {
            setup_timing_thread();

            /* run app */
            native::main_loop();
        }

        fn will_spawn(&self) {
            let slock = slock_main_owner();

            self.provider.will_spawn(self.handle(), slock.borrow());
        }
    }

    impl<P: ApplicationProvider> Application<P> {
        pub(crate) fn new(provider: P) -> Self {
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
            let window = Window::new(self.handle, provider);
            self.handle.windows.borrow_mut().push(window);
        }

        pub fn exit(&self, _s: &Slock<MainThreadMarker>) {
            native::exit();
        }
    }
}
pub use application::*;

mod window {
    use std::marker::PhantomData;
    use crate::core::{MSlock, Slock};
    use crate::core::application::{Application, ApplicationProvider};
    use crate::native::{exit_window, register_window, WindowHandle};
    use crate::util::markers::MainThreadMarker;

    pub trait WindowProvider: 'static {
        fn title(&self, s: MSlock<'_>);

        fn style(&self, s: MSlock<'_>);

        fn menu_bar(&self, s: MSlock<'_>);

        fn tree(&self, s: MSlock<'_>);

        fn can_close(&self, _s: MSlock<'_>) -> bool {
            true
        }
    }

    pub(crate) trait WindowBase {
        /* delegate methods */
        fn can_close(&self, s: MSlock<'_>) -> bool;

        fn set_handle(&mut self, handle: WindowHandle);

        fn get_handle(&self) -> WindowHandle;
    }

    pub struct Window<A: ApplicationProvider, P: WindowProvider> {
        app: &'static Application<A>,
        marker: PhantomData<A>,
        provider: P,
        handle: WindowHandle
    }

    impl<A: ApplicationProvider, P: WindowProvider> Window<A, P> {
        pub(super) fn new(app: &'static Application<A>, provider: P) -> Box<dyn WindowBase> {
            let window = Window {
                app,
                marker: PhantomData,
                provider,
                handle: 0
            };

            let mut b: Box<dyn WindowBase> = Box::new(window);
            /* show window */
            b.set_handle(register_window::<A, P>(&b));

            b
        }
    }

    impl<A: ApplicationProvider, P: WindowProvider> WindowBase for Window<A, P> {
        fn can_close(&self, s: MSlock<'_>) -> bool {
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
        pub fn exit(&self, _s: &Slock<MainThreadMarker>) {
            /* remove from application window list */
            self.app.windows
                .borrow_mut()
                .retain(|window| window.get_handle() == self.handle);

            // main thread guaranteed
            exit_window(self.handle)
        }
    }
}
pub use window::*;

mod slock {
    use std::marker::PhantomData;
    use std::ops::Deref;
    use std::sync::{Mutex, MutexGuard};
    use std::thread;
    use std::time::Duration;
    use crate::core::{timed_worker};
    use crate::core::debug_stats::DebugInfo;
    use crate::native;
    use crate::state::{ActionFilter, Binding, CapacitatedSignal, FixedSignal, IntoAction, JoinedSignal, Signal, Stateful};
    use crate::state::capacitor::IncreasingCapacitor;
    use crate::util::markers::{AnyThreadMarker, MainThreadMarker, ThreadMarker};

    static GLOBAL_STATE_LOCK: Mutex<()> = Mutex::new(());

    pub struct SlockOwner<M=AnyThreadMarker> where M: ThreadMarker {
        _guard: MutexGuard<'static, ()>,
        pub(crate) debug_info: DebugInfo,
        unsync_unsend: PhantomData<*const ()>,
        thread_marker: PhantomData<M>,
    }

    #[cfg(debug_assertions)]
    #[derive(Copy, Clone)]
    pub struct Slock<'a, M=AnyThreadMarker> where M: ThreadMarker {
        pub(crate) owner: &'a SlockOwner<M>,
        // unnecessary? but too emphasize
        unsync_unsend: PhantomData<*const ()>,
    }

    #[cfg(not(debug_assertions))]
    #[derive(Copy, Clone)]
    pub struct Slock<'a, M=AnyThreadMarker> where M: ThreadMarker {
        owner: PhantomData<&'a SlockOwner<M>>,
        unsync_unsend: PhantomData<*const ()>,
    }

    pub type MSlock<'a> = Slock<'a, MainThreadMarker>;


    fn global_guard() -> MutexGuard<'static, ()> {
        static LOCKED_THREAD: Mutex<Option<thread::ThreadId>> = Mutex::new(None);

        #[cfg(debug_assertions)]
        {
            let lock = GLOBAL_STATE_LOCK.try_lock();
            let ret = if let Ok(lock) = lock {
                lock
            } else {
                if *LOCKED_THREAD.lock().unwrap() == Some(thread::current().id()) {
                    panic!("Attempted to acquire state lock when the current thread already has the state lock. \
                        Instead of acquiring the slock multiple times, either call find_slock() or pass around the Slock by reference.                        \
                        In production, instead of a panic, this will result in a deadlock! \
                       "
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

    /// The State Lock (often abbreviated 'slock') is a simple
    /// but important concept in quarve. It essentially acts as a global
    /// mutex and whichever thread owns the slock is the only thread
    /// that is able to perform many operations on the state graph, views,
    /// and other core constructs.
    ///
    /// You should call this method to acquire ownership of the slock.
    /// For the lifetime of the returned object, the calling thread
    /// will be able to read and write to the state graph. Note that
    /// calling this method when the current thread already has ownership of the slock
    /// will result in a panic. On the other hand, if another thread currently
    /// has ownership, then this method will block until the other thread is finished. For this reason,
    /// you should acquire the slock only once you need it and drop the slock
    /// as soon as you are done with the current transaction.
    /// However, do not feel the need to drop it and reacquire after every micro-operation;
    /// this may cause the user to view the result of a partially applied transaction.
    pub fn slock_owner() -> SlockOwner {
        SlockOwner {
            _guard: global_guard(),
            debug_info: DebugInfo::new(),
            unsync_unsend: PhantomData,
            thread_marker: PhantomData
        }
    }

    pub fn slock_main_owner() -> SlockOwner<MainThreadMarker> {
        if !native::is_main() {
            panic!("Cannot call slock_main")
        }

        SlockOwner {
            _guard: global_guard(),
            debug_info: DebugInfo::new(),
            unsync_unsend: PhantomData,
            thread_marker: PhantomData,
        }
    }

    impl<M: ThreadMarker> SlockOwner<M> {
        // note that the global state lock is kept for entire
        // lifetime of slockowner; borrowing does not acquire the state lock
        pub fn borrow(&self) -> Slock<'_, M> {
            #[cfg(debug_assertions)]
            {
                Slock {
                    owner: &self,
                    unsync_unsend: PhantomData
                }
            }

            #[cfg(not(debug_assertions))]
            {
                Slock {
                    owner: PhantomData,
                    unsync_unsend: PhantomData
                }
            }
        }
    }

    impl<'a, M: ThreadMarker> Slock<'a, M> {
        pub fn fixed_signal<T: Send + 'static>(self, val: T) -> impl Signal<T> {
            FixedSignal::new(val)
        }

        pub fn clock_signal(self) -> impl Signal<f64> {
            let constant = FixedSignal::new(0.0);
            CapacitatedSignal::from(&constant, IncreasingCapacitor, self)
        }

        pub fn timed_worker<F>(self, f: F)
            where F: for<'b> FnMut(Duration, Slock<'b>) -> bool + Send + 'static
        {
            timed_worker(f)
        }

        pub fn map<S, T, U, F>(self, signal: &S, map: F) -> impl Signal<U>
            where S: Signal<T>,
                  T: Send + 'static,
                  U: Send + 'static,
                  F: Send + 'static + Fn(&T) -> U
        {
            signal.map(map, self.as_general_slock())
        }

        pub fn join<T, U>(self, t: &impl Signal<T>, u: &impl Signal<U>)
                          -> impl Signal<(T, U)>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static
        {
            JoinedSignal::from(t, u, |t, u| (t.clone(), u.clone()), self.as_general_slock())
        }

        pub fn join_map<T, U, V, F>(self, t: &impl Signal<T>, u: &impl Signal<U>, map: F)
                                    -> impl Signal<V>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static,
                  V: Send + 'static,
                  F: Send + Clone + 'static + Fn(&T, &U) -> V
        {
            JoinedSignal::from(t, u, map, self.as_general_slock())
        }

        pub fn apply<S, F>(self, action: impl IntoAction<S::Action, S>, to: &impl Binding<S, F>)
            where S: Stateful, F: ActionFilter<Target=S>
        {
            to.apply(action, self.as_general_slock());
        }

        pub fn read<T>(self, from: &'a impl Signal<T>)
                           -> impl Deref<Target=T> + 'a where T: Send + 'static {
            from.borrow(self.as_general_slock())
        }

        pub fn try_as_main_slock(self) -> Option<MSlock<'a>> {
            if !native::is_main() {
                None
            }
            else {
                // safety:
                // data layouts of us and owner are the same
                // and we confirmed we're on the main thread
                unsafe {
                    Some(std::mem::transmute(self))
                }
            }
        }

        pub fn as_main_slock(self) -> MSlock<'a> {
            self.try_as_main_slock().expect("This method should only be called on the main thread!")
        }

        /// Given a slock that may be say the main slock
        /// convert it into the general slock
        /// Some methods require the general slock
        pub fn as_general_slock(self) -> Slock<'a> {
            // safety: if a slock comes from a specific thread
            // it certainly came from any thread.
            // The data layouts of the reference field are exactly
            // the same
            // (the layout of slock are certainly the same)
            // (and the layout of slock owner are the same)
            unsafe {
                std::mem::transmute(self)
            }
        }
    }
}
pub use slock::*;

mod global {
    use std::time::{Duration, Instant};
    use crate::core::application::{Application, ApplicationProvider};
    use crate::native;
    use super::{APP, MSlock, Slock, TIMER_WORKER};

    pub fn timed_worker<F: for<'a> FnMut(Duration, Slock<'a>) -> bool + Send + 'static>(func: F) {
        TIMER_WORKER.get()
            .expect("Cannot call quarve functions before launch!")
            .send((Box::new(func), Instant::now()))
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
    /// This method can be called from any thread (including) the main one
    pub fn run_main<F>(f: F) where F: for<'a> FnOnce(MSlock<'a>) + Send + 'static {
        native::run_main(f)
    }
}
pub use global::*;

#[cfg(test)]
mod tests {
    use std::thread::sleep;
    use std::time::Duration;
    use crate::core::slock_owner;

    /* of course, should only panic in debug scenarios */
    #[test]
    #[should_panic]
    fn test_recursive_lock_causes_panic() {
        let _s = slock_owner();
        let _s2 = slock_owner();
    }

    /* no panic test */
    #[test]
    fn test_different_threads_slock_no_panic() {
        let s = slock_owner();
        let res = std::thread::spawn(|| {
            let _s = slock_owner();

            return 1;
        });

        sleep(Duration::from_millis(100));
        drop(s);

        assert_eq!(res.join().unwrap(), 1);
    }
}