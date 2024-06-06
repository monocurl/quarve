use std::cell::OnceCell;
use std::sync::{OnceLock};
use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};

static TIMER_WORKER: OnceLock<SyncSender<(Box<dyn for<'a> FnMut(Duration, Slock<'a>) -> bool + Send>, Instant)>> = OnceLock::new();

thread_local! {
    pub(crate) static APP: OnceCell<Application> = OnceCell::new();
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
            if hang > Duration::from_millis(500) {
                println!("quarve: state locked attained for {} milliseconds. \
                    This may cause visible stalls; \
                    please try to release the state lock as soon as the transaction is complete.",
                         hang.as_millis());
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
                subscribers.retain_mut(|(f, start) | f(start_time.duration_since(*start), s.marker()));
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
                // FIXME this is sleeping too long
                // we may want to look at https://crates.io/crates/spin_sleep
                // at some point
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
    use std::sync::Arc;
    use crate::core::{MSlock, slock_main_owner};
    use crate::core::life_cycle::setup_timing_thread;
    use crate::core::window::{Window, WindowBase, WindowProvider};
    use crate::native;
    use crate::state::slock_cell::SlockCell;

    pub trait Environment: 'static {
        fn root_environment() -> Self;
    }

    pub trait ApplicationProvider: 'static {
        fn will_spawn(&self, app: &Application, s: MSlock<'_>);
    }

    pub struct Application {
        provider: Box<dyn ApplicationProvider>,
        pub(super) windows: RefCell<Vec<Arc<SlockCell<dyn WindowBase>>>>
    }

    impl Application {
        pub(crate) fn new(provider: impl ApplicationProvider) -> Self {
            Application {
                provider: Box::new(provider),
                windows: RefCell::new(Vec::new())
            }
        }

        pub(crate) fn run(&self) {
            setup_timing_thread();

            /* run app */
            native::global::main_loop();
        }

        pub(crate) fn will_spawn(&self) {
            let slock = slock_main_owner();

            self.provider.will_spawn(self, slock.marker());
        }

        pub fn spawn_window<W>(&self, provider: W, s: MSlock<'_>) where W: WindowProvider {
            self.windows.borrow_mut().push(Window::new(provider, s));
        }

        #[cold]
        pub fn exit(&self, _s: MSlock<'_>) {
            native::global::exit();
        }
    }
}
pub use application::*;

mod window {
    use std::cell::{Cell, RefCell};
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;
    use std::ops::{Deref, DerefMut};
    use std::sync::{Arc, Weak};
    use crate::core::{APP, Environment, MSlock, run_main_async, run_main_maybe_sync, Slock};
    use crate::native;
    use crate::native::{WindowHandle};
    use crate::state::{Signal};
    use crate::state::slock_cell::SlockCell;
    use crate::util::geo::{AlignedFrame, Alignment, Size};
    use crate::view::{InnerViewBase, View, ViewProvider};

    pub trait WindowProvider: 'static {
        type Env: Environment;

        fn title(&self, s: MSlock<'_>) -> impl Signal<String>;

        fn style(&self, s: MSlock<'_>);

        fn tree(&self, env: &Self::Env, s: MSlock<'_>)
            -> View<Self::Env, impl ViewProvider<Self::Env, LayoutContext=()>>;

        fn can_close(&self, _s: MSlock<'_>) -> bool {
            true
        }
    }

    pub(crate) trait WindowBase {
        /* delegate methods */
        fn can_close(&self, s: MSlock<'_>) -> bool;

        fn get_handle(&self) -> WindowHandle;

        fn layout(&self, s: MSlock);
    }

    pub(crate) trait WindowEnvironmentBase<E>: WindowBase where E: Environment {
        // this method is guaranteed to only touch
        // Send parts of self
        // handle is because of some async operations
        fn invalidate_view(&self, handle: Weak<SlockCell<dyn WindowEnvironmentBase<E>>>, view: Weak<SlockCell<dyn InnerViewBase<E>>>, depth: u32, s: Slock);
    }

    struct InvalidatedEntry<E> where E: Environment {
        view: Weak<SlockCell<dyn InnerViewBase<E>>>,
        // use negative depth to flip ordering
        depth: i32
    }

    impl<E> PartialEq<Self> for InvalidatedEntry<E> where E: Environment {
        fn eq(&self, other: &Self) -> bool {
            self.depth == other.depth && std::ptr::addr_eq(self.view.as_ptr(), other.view.as_ptr())
        }
    }

    impl<E> Eq for InvalidatedEntry<E> where E: Environment { }

    impl<E> PartialOrd for InvalidatedEntry<E> where E: Environment {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl<E> Ord for InvalidatedEntry<E> where E: Environment {
        fn cmp(&self, other: &Self) -> Ordering {
            if self.depth != other.depth {
                self.depth.cmp(&other.depth)
            }
            else {
                self.view.as_ptr().cmp(&other.view.as_ptr())
            }
        }
    }

    pub struct Window<P> where P: WindowProvider {
        provider: P,

        // to prevent reentry
        // it is common to take out the environment
        // and put it back in
        environment: Cell<Option<Box<P::Env>>>,
        // avoid having to borrow window mutably
        // when invalidating
        invalidated_views: RefCell<BinaryHeap<InvalidatedEntry<P::Env>>>,

        /* native */
        handle: WindowHandle,
        content_view: Arc<SlockCell<dyn InnerViewBase<P::Env>>>
    }

    impl<P> Window<P> where P: WindowProvider {
        // order things are done is a bit awkward
        // but need to coordinate between many things
        pub(super) fn new(provider: P, s: MSlock<'_>) -> Arc<SlockCell<dyn WindowBase>> {
            let root_env = P::Env::root_environment();

            let handle = native::window::window_init(s);
            let content_view = provider.tree(&root_env, s).0;

            let window = Window {
                provider,
                environment: Cell::new(Some(Box::new(root_env))),
                invalidated_views: RefCell::new(BinaryHeap::new()),
                handle,
                content_view
            };

            let b = Arc::new(SlockCell::new_main(window, s));
            // create initial tree and other tasks
            Window::init(&b, s);

            // set handle of backing
            {
                let borrow = b.borrow_main(s);
                native::window::window_set_handle(handle, borrow.deref() as &dyn WindowBase, s);
            }

            b
        }

        fn set_dimensions_of_window(&self, content_borrow: &mut dyn InnerViewBase<P::Env>, s: MSlock) {
            let xsquish = content_borrow.xsquished_size(s);
            let ysquish = content_borrow.ysquished_size(s);
            let min = Size::new(xsquish.w.max(ysquish.w), xsquish.h.max(ysquish.h));
            let xstretch = content_borrow.xstretched_size(s);
            let ystretch = content_borrow.ystretched_size(s);
            let max = Size::new(xstretch.w.min(ystretch.w), xstretch.h.max(ystretch.h));

            native::window::window_set_min_size(self.handle, min.w as f64, min.h as f64, s);
            native::window::window_set_max_size(self.handle, max.w as f64, max.h as f64, s);
        }

        fn init(this: &Arc<SlockCell<Self>>, s: MSlock) {
            let borrow = this.borrow_main(s);

            /* apply style */
            Self::apply_window_style(s, borrow.deref());

            /* apply window title  */
            Self::title_listener(&this, borrow.deref(), s);

            /* mount content view */
            Self::mount_content_view(&this, s, borrow.deref());
        }

        // logic is a bit tricky for initial mounting
        // due to reentry
        fn mount_content_view(this: &Arc<SlockCell<Window<P>>>, s: MSlock, borrow: &Window<P>) {
            let handle = borrow.handle;
            let weak_content = Arc::downgrade(&borrow.content_view);
            let weak_window = Arc::downgrade(this)
                as Weak<SlockCell<dyn WindowEnvironmentBase<P::Env>>>;

            let mut stolen_env = borrow.environment.take().unwrap();
            let content_copy = borrow.content_view.clone();

            let mut content_borrow = content_copy.borrow_mut_main(s);
            content_borrow.show(weak_content, &weak_window, stolen_env.deref_mut(), 0u32, s);

            let intrinsic = content_borrow.intrinsic_size(s);
            content_borrow.try_layout_down(
                stolen_env.deref_mut(),
                Some(AlignedFrame::new_from_size(intrinsic, Alignment::Center)),
                s
            ).unwrap();

            // set window size
            native::window::window_set_size(borrow.handle, intrinsic.w as f64, intrinsic.h as f64, s);
            borrow.set_dimensions_of_window(content_borrow.deref_mut(), s);

            // give back env
            borrow.environment.set(Some(stolen_env));

            debug_assert!(!content_borrow.backing().is_null());

            native::window::window_set_root(handle, content_borrow.backing(), s);
        }

        fn apply_window_style(s: MSlock, borrow: &Window<P>) {
            let _style = borrow.provider.style(s);
        }

        fn title_listener(this: &Arc<SlockCell<Window<P>>>, borrow: &Window<P>, s: MSlock) {
            let title = borrow.provider.title(s);
            let weak = Arc::downgrade(&this);
            title.listen(move |val, _s| {
                let Some(this) = weak.upgrade() else {
                    return false;
                };

                let title_copy = val.to_owned();
                run_main_async(move |s| {
                    let borrow = this.borrow_main(s);
                    native::window::window_set_title(borrow.handle, &title_copy, s);
                });

                true
            }, s);

            let current = title.borrow(s).to_owned();
            native::window::window_set_title(borrow.handle, &current, s);
        }

        // env is currently right above from
        // and has to be moved right above to
        fn walk_env(
            env: &mut P::Env,
            from: Arc<SlockCell<dyn InnerViewBase<P::Env>>>,
            to: Arc<SlockCell<dyn InnerViewBase<P::Env>>>,
            s: MSlock
        ) {
            // equalize level
            let mut targ_stack = vec![];

            let mut curr_depth = from.borrow_main(s).depth();
            let mut targ_depth = to.borrow_main(s).depth();

            let mut curr = from;
            let mut targ = to;
            while curr_depth > targ_depth {
                let next = curr.borrow_main(s).superview().unwrap();
                curr = next;
                curr.borrow_main(s)
                    .pop_environment(env, s);
                curr_depth -= 1;
            }

            while !std::ptr::addr_eq(curr.as_ptr(), targ.as_ptr()) {
                if curr_depth == targ_depth {
                    // need to advance curr as well
                    let next = curr.borrow_main(s).superview().unwrap();
                    curr = next;
                    curr.borrow_main(s)
                        .pop_environment(env, s);
                    curr_depth -= 1;
                }

                // remember that we want to be one above the target
                // so we push to stack only afterwards
                let next = targ.borrow_main(s).superview().unwrap();
                targ = next;
                targ_stack.push(targ.clone());
                targ_depth -= 1;
            }

            // walk towards the target
            for node in targ_stack.into_iter().rev() {
                node.borrow_main(s)
                    .push_environment(env, s);
            }
        }

        #[cold]
        pub fn exit(&self, s: MSlock<'_>) {
            /* remove from application window list */
            APP.with(|app| {
                app.get().unwrap()
                    .windows
                    .borrow_mut()
                    .retain(|window| window.borrow_main(s).get_handle() != self.handle);
            });

            native::window::window_exit(self.handle, s);
        }
    }

    impl<P> WindowBase for Window<P> where P: WindowProvider {
        fn can_close(&self, s: MSlock<'_>) -> bool {
            let can_close = self.provider.can_close(s);

            if can_close {
                APP.with(|app| {
                    app.get().unwrap()
                        .windows
                        .borrow_mut()
                        .retain(|window| window.borrow_main(s).get_handle() != self.handle);
                });
            }

            can_close
        }

        fn get_handle(&self) -> WindowHandle {
            self.handle
        }

        fn layout(&self, s: MSlock) {
            // layout down queue
            // layout up queue is stored in self
            // and may change by invalidations
            let mut layout_down: BinaryHeap<InvalidatedEntry<P::Env>> = BinaryHeap::new();
            let mut env = self.environment.take().unwrap();
            // the environment is right above this node
            let mut env_spot = self.content_view.clone();

            loop {
                /* decide what to do next */
                let has_up = if !self.invalidated_views.borrow().is_empty() {
                    true
                }
                else if !layout_down.is_empty() {
                    false
                }
                else {
                    break;
                };

                if has_up {
                    let curr = self.invalidated_views
                        .borrow_mut().pop().unwrap();

                    let Some(view) = curr.view.upgrade() else {
                        continue;
                    };

                    let borrow = view.borrow_main(s);

                    /* make sure it doesn't have a newer entry */
                    if borrow.depth() as i32 != curr.depth || !borrow.needs_layout_up() {
                        continue;
                    }

                    // move environment to target
                    Self::walk_env(env.deref_mut(), env_spot.clone(), view.clone(), s);

                    drop(borrow);
                    let mut borrow = view.borrow_mut_main(s);

                    let superview = borrow.superview();
                    if borrow.layout_up(env.deref_mut(), s) && superview.is_some() {
                        // we have to schedule parent
                        self.invalidated_views.borrow_mut()
                            .push(InvalidatedEntry {
                                view: Arc::downgrade(&superview.unwrap()),
                                depth: curr.depth - 1
                            });
                    }
                    else {
                        // schedule down layout of self
                        layout_down.push(InvalidatedEntry {
                            view: Arc::downgrade(&view),
                            // invert ordering
                            depth: -curr.depth
                        });
                    }
                }
                else {
                    let curr = layout_down.pop().unwrap();

                    /* ensure that view is still valid */
                    let Some(view) = curr.view.upgrade() else {
                        continue;
                    };

                    let borrow = view.borrow_main(s);
                    /* make sure it doesn't have a newer entry */
                    if borrow.depth() as i32 != -curr.depth || !borrow.needs_layout_down() {
                        continue;
                    }

                    // move environment to (right above) target
                    Self::walk_env(env.deref_mut(), env_spot.clone(), view.clone(), s);

                    drop(borrow);
                    let mut borrow = view.borrow_mut_main(s);

                    // try to layout down
                    // if fail must mean we need to schedule a new layout of the parent
                    // as this node requires context
                    if let Err(_) = borrow.try_layout_down(env.deref_mut(), None, s) {
                        // superview must exist since otherwise layout
                        // wouldn't have failed
                        let superview = borrow.superview().unwrap();
                        superview.borrow_mut_main(s).set_needs_layout_down();

                        layout_down.push(InvalidatedEntry {
                            view: Arc::downgrade(&superview),
                            // note the negative ops for reverse ordering of depth
                            depth: curr.depth + 1
                        });
                    }

                    drop(borrow);
                    // move the env pointer to the current view
                    env_spot = view;
                }
            }

            // put back env to root
            Self::walk_env(env.deref_mut(), env_spot, self.content_view.clone(), s);
            self.environment.set(Some(env));

            // maybe init dimensions of view
            self.set_dimensions_of_window(self.content_view.borrow_mut_main(s).deref_mut(), s);
        }
    }

    impl<P> WindowEnvironmentBase<P::Env> for Window<P> where P: WindowProvider {
        fn invalidate_view(&self, handle: Weak<SlockCell<dyn WindowEnvironmentBase<P::Env>>>, view: Weak<SlockCell<dyn InnerViewBase<P::Env>>>, depth: u32, s: Slock) {
            // note that we're only touching send parts of self
            let mut borrow = self.invalidated_views.borrow_mut();
            if borrow.is_empty() {
                // schedule a layout
                let native_handle = self.handle;
                run_main_maybe_sync(move |m| {
                    // avoid using handle after free
                    if handle.upgrade().is_some() {
                        native::window::window_set_needs_layout(native_handle, m);
                    }
                }, s);
            }

            borrow.push(InvalidatedEntry {
                view,
                depth: depth as i32
            });
        }
    }

    impl<P> Drop for Window<P> where P: WindowProvider {
        fn drop(&mut self) {
            native::window::window_free(self.handle);
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
    use crate::state::{ActionFilter, Binding, FixedSignal, IntoAction, JoinedSignal, CapacitatedSignal, Signal, Stateful};
    use crate::state::capacitor::IncreasingCapacitor;
    use crate::util::markers::{AnyThreadMarker, MainThreadMarker, ThreadMarker};
    use crate::util::rust_util::PhantomUnsendUnsync;

    static GLOBAL_STATE_LOCK: Mutex<()> = Mutex::new(());
    #[cfg(debug_assertions)]
    static LOCKED_THREAD: Mutex<Option<thread::ThreadId>> = Mutex::new(None);

    pub struct SlockOwner<M=AnyThreadMarker> where M: ThreadMarker {
        _guard: MutexGuard<'static, ()>,
        pub(crate) debug_info: DebugInfo,
        unsend_unsync: PhantomUnsendUnsync,
        thread_marker: PhantomData<M>,
    }

    #[cfg(debug_assertions)]
    #[derive(Copy, Clone)]
    pub struct Slock<'a, M=AnyThreadMarker> where M: ThreadMarker {
        pub(crate) owner: &'a SlockOwner<M>,
        // unnecessary? but to emphasize
        unsend_unsync: PhantomUnsendUnsync,
    }

    #[cfg(not(debug_assertions))]
    #[derive(Copy, Clone)]
    pub struct Slock<'a, M=AnyThreadMarker> where M: ThreadMarker {
        owner: PhantomData<&'a SlockOwner<M>>,
        unsend_unsync: PhantomUnsendUnsync,
    }

    pub type MSlock<'a> = Slock<'a, MainThreadMarker>;

    #[inline]
    fn global_guard() -> MutexGuard<'static, ()> {

        #[cfg(debug_assertions)]
        {
            let lock = GLOBAL_STATE_LOCK.try_lock();
            let ret = if let Ok(lock) = lock {
                lock
            } else {
                if *LOCKED_THREAD.lock().unwrap() == Some(thread::current().id()) {
                    panic!("Attempted to acquire state lock when the current thread already has the state lock. \
                        Instead of acquiring the slock multiple times, pass around the Slock marker. \
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
    /// but important concept in quarve. It acts as a global
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
    #[inline]
    pub fn slock_owner() -> SlockOwner {
        SlockOwner {
            _guard: global_guard(),
            debug_info: DebugInfo::new(),
            unsend_unsync: PhantomData,
            thread_marker: PhantomData
        }
    }

    #[inline]
    pub fn slock_main_owner() -> SlockOwner<MainThreadMarker> {
        if !native::global::is_main() {
            panic!("Cannot call slock_main")
        }

        SlockOwner {
            _guard: global_guard(),
            debug_info: DebugInfo::new(),
            unsend_unsync: PhantomData,
            thread_marker: PhantomData,
        }
    }

    impl<M: ThreadMarker> SlockOwner<M> {
        // note that the global state lock is kept for entire
        // lifetime of slockowner; calling marker does not acquire the state lock
        // and dropping the marker does not relenquish it
        #[inline]
        pub fn marker(&self) -> Slock<'_, M> {
            #[cfg(debug_assertions)]
            {
                Slock {
                    owner: &self,
                    unsend_unsync: PhantomData,
                }
            }

            #[cfg(not(debug_assertions))]
            {
                Slock {
                    owner: PhantomData,
                    unsend_unsync: PhantomData,
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    impl<M: ThreadMarker> Drop for SlockOwner<M> {
        fn drop(&mut self) {
            *LOCKED_THREAD.lock().unwrap() = None;
        }
    }

    impl<'a, M: ThreadMarker> Slock<'a, M> {
        pub fn fixed_signal<T: Send + 'static>(self, val: T) -> FixedSignal<T> {
            FixedSignal::new(val)
        }

        pub fn clock_signal(self) -> CapacitatedSignal<IncreasingCapacitor> {
            let constant = FixedSignal::new(0.0);
            CapacitatedSignal::from(&constant, IncreasingCapacitor, self)
        }

        pub fn timed_worker<F>(self, f: F)
            where F: for<'b> FnMut(Duration, Slock<'b>) -> bool + Send + 'static
        {
            timed_worker(f)
        }

        pub fn map<S, T, U, F>(self, signal: &S, map: F) -> S::MappedOutput<U>
            where S: Signal<T>,
                  T: Send + 'static,
                  U: Send + 'static,
                  F: Send + 'static + Fn(&T) -> U
        {
            signal.map(map, self.as_general_slock())
        }

        pub fn join<T, U>(self, t: &impl Signal<T>, u: &impl Signal<U>)
                          -> JoinedSignal<T, U, (T, U)>
            where T: Send + Clone + 'static,
                  U: Send + Clone + 'static
        {
            JoinedSignal::from(t, u, |t, u| (t.clone(), u.clone()), self.as_general_slock())
        }

        pub fn join_map<T, U, V, F>(self, t: &impl Signal<T>, u: &impl Signal<U>, map: F)
                                    -> JoinedSignal<T, U, V>
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
            if !native::global::is_main() {
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
    #[cold]
    pub fn launch(provider: impl ApplicationProvider) {
        if let Err(_) = APP.with(|m| m.set(Application::new(provider))) {
            panic!("Cannot launch an app multiple times");
        }

        APP.with(|m| m.get().unwrap().run());
    }

    /// If the current thread is main, it executes
    /// the function directly. Otherwise,
    /// the behavior is identical to run_main_async
    pub fn run_main_maybe_sync<F>(f: F, s: Slock) where F: for<'a> FnOnce(MSlock<'a>) + Send + 'static {
        if let Some(main) = s.try_as_main_slock() {
            f(main);
        }
        else {
            native::global::run_main(f)
        }
    }

    /// Asynchronously runs a task on the main thread
    /// This method can be called from any thread (including) the main one
    pub fn run_main_async<F>(f: F) where F: for<'a> FnOnce(MSlock<'a>) + Send + 'static {
        native::global::run_main(f)
    }

    /// Must be called only after initial application launch was called
    pub fn with_app(f: impl FnOnce(&Application), _s: MSlock<'_>) {
        APP.with(|app| {
            f(app.get().expect("With app should only be called after the application has fully launched"))
        })
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