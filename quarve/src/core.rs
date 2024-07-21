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
    pub(crate) struct DebugInfo {

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
                    try to release the state lock as soon as the transaction is complete.",
                         hang.as_millis());
            }
        }
    }

    #[cfg(not(debug_assertions))]
    impl DebugInfo {
        pub fn new() -> Self {
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

mod environment {
    use crate::view::menu::MenuChannel;

    pub trait Environment: 'static {
        type Const: 'static;
        type Variable: 'static;

        fn root_environment() -> Self;

        fn const_env(&self) -> &Self::Const;
        fn variable_env(&self) -> &Self::Variable;
        fn variable_env_mut(&mut self) -> &mut Self::Variable;
    }

    pub struct StandardChannels {
        pub undo_menu: MenuChannel,
        pub redo_menu: MenuChannel,
        pub cut_menu: MenuChannel,
        pub copy_menu: MenuChannel,
        pub paste_menu: MenuChannel,
        pub delete_menu: MenuChannel,
        pub select_all_menu: MenuChannel
    }

    pub struct StandardConstEnv {
        pub channels: StandardChannels
    }

    impl StandardConstEnv {
        pub fn new() -> Self {
            Self {
                channels: StandardChannels {
                    undo_menu: MenuChannel::new(),
                    redo_menu: MenuChannel::new(),
                    cut_menu: MenuChannel::new(),
                    copy_menu: MenuChannel::new(),
                    paste_menu: MenuChannel::new(),
                    delete_menu: MenuChannel::new(),
                    select_all_menu: MenuChannel::new(),
                }
            }
        }
    }

    impl AsRef<StandardConstEnv> for StandardConstEnv {
        fn as_ref(&self) -> &StandardConstEnv {
            self
        }
    }
}
pub use environment::*;

mod application {
    use std::cell::RefCell;
    use std::sync::Arc;
    use crate::core::{MSlock, slock_main_owner};
    use crate::core::life_cycle::setup_timing_thread;
    use crate::core::window::{Window, WindowNativeCallback, WindowProvider};
    use crate::native;
    use crate::state::slock_cell::{MainSlockCell};


    pub trait ApplicationProvider: 'static {
        fn will_spawn(&self, app: &Application, s: MSlock);
    }

    pub struct Application {
        provider: Box<dyn ApplicationProvider>,
        pub(super) windows: RefCell<Vec<Arc<MainSlockCell<dyn WindowNativeCallback>>>>
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

        pub fn spawn_window<W>(&self, provider: W, s: MSlock) where W: WindowProvider {
            self.windows.borrow_mut().push(Window::new(provider, s));
        }

        #[cold]
        pub fn exit(&self, _s: MSlock) {
            native::global::exit();
        }
    }
}
pub use application::*;

mod window {
    use std::cell::{Cell, RefCell};
    use std::collections::BinaryHeap;
    use std::ops::{Deref, DerefMut};
    use std::sync::{Arc, Weak};
    use crate::core::{APP, Environment, MSlock, run_main_async, run_main_maybe_sync, Slock};
    use crate::core::window::invalidated_entry::InvalidatedEntry;
    use crate::event::{Event, EventPayload, EventResult};
    use crate::{native, util};
    use crate::native::{WindowHandle};
    use crate::native::window::{window_exit, window_set_menu};
    use crate::state::{ActualDiffSignal, Binding, Filterless, Signal, Store};
    use crate::state::SetAction::Set;
    use crate::state::slock_cell::{MainSlockCell};
    use crate::util::geo::{Point, Rect, Size};
    use crate::view::{InnerViewBase};
    use crate::view::{ViewProvider};
    use crate::view::menu::{WindowMenu};

    mod invalidated_entry {
        use std::cmp::Ordering;
        use std::sync::Weak;
        use crate::core::Environment;
        use crate::state::slock_cell::MainSlockCell;
        use crate::view::InnerViewBase;

        pub(super) struct InvalidatedEntry<E> where E: Environment {
            pub(super) view: Weak<MainSlockCell<dyn InnerViewBase<E>>>,
            // use negative depth to flip ordering in some cases
            pub(super) depth: i32
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
                    self.view.as_ptr().cast::<()>()
                        .cmp(&other.view.as_ptr().cast::<()>())
                }
            }
        }
    }

    pub trait WindowProvider: 'static {
        type Env: Environment;

        fn title(&self, env: &<Self::Env as Environment>::Const, s: MSlock) -> impl Signal<Target=String>;

        fn size(&self, env: &<Self::Env as Environment>::Const, s: MSlock) -> (Size, Size, Size);

        fn root(&self, env: &<Self::Env as Environment>::Const, s: MSlock)
                -> impl ViewProvider<Self::Env, DownContext=()>;

        #[allow(unused_variables)]
        fn menu(&self, env: &<Self::Env as Environment>::Const, s: MSlock) -> WindowMenu;

        #[allow(unused_variables)]
        fn is_open(&self, env: &<Self::Env as Environment>::Const, s: MSlock) -> impl Binding<Filterless<bool>> {
            Store::new(true)
                .binding()
        }

        #[allow(unused_variables)]
        fn is_fullscreen(&self, env: &<Self::Env as Environment>::Const, s: MSlock) -> impl Binding<Filterless<bool>> {
            Store::new(false)
                .binding()
        }
    }

    pub(crate) trait WindowNativeCallback {
        /* delegate methods */
        fn can_close(&self, s: MSlock) -> bool;
        fn hide_root(&self, s: MSlock);

        fn handle(&self) -> WindowHandle;

        fn layout_full(&self, w: f64, h: f64, s: MSlock);

        fn dispatch_native_event(&self, event: Event, s: MSlock);
        fn set_fullscreen(&self, fs: bool, s: MSlock);
    }

    pub(crate) trait WindowViewCallback<E> where E: Environment {
        // depth = only consider nodes with strictly greater depth (use -1 for all)
        fn layout_up(&self, env: &mut E, right_below: Option<Arc<MainSlockCell<dyn InnerViewBase<E>>>>, depth: i32, s: MSlock);

        // this method is guaranteed to only touch
        // Send parts of self
        // handle is because of some async operations
        fn invalidate_view(&self, handle: Weak<MainSlockCell<dyn WindowViewCallback<E>>>, view: Weak<MainSlockCell<dyn InnerViewBase<E>>>, depth: u32, s: Slock);

        fn request_focus(&self, view: Weak<MainSlockCell<dyn InnerViewBase<E>>>);
        fn unrequest_focus(&self, view: Weak<MainSlockCell<dyn InnerViewBase<E>>>);

        fn request_default_focus(&self, view: Weak<MainSlockCell<dyn InnerViewBase<E>>>);
        fn unrequest_default_focus(&self, view: Weak<MainSlockCell<dyn InnerViewBase<E>>>);

        fn request_key_listener(&self, view: Weak<MainSlockCell<dyn InnerViewBase<E>>>);
        fn unrequest_key_listener(&self, view: Weak<MainSlockCell<dyn InnerViewBase<E>>>);
    }

    pub struct Window<P> where P: WindowProvider {
        provider: P,

        /* event state */
        last_cursor: Cell<Point>,
        focus: RefCell<Option<Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>>>,
        scheduled_focus: Cell<Option<Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>>>,
        default_focus: RefCell<Vec<Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>>>,
        key_listeners: RefCell<Vec<Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>>>,

        // to prevent reentry
        // it is common to take out the environment
        // and put it back in
        environment: Cell<Option<Box<P::Env>>>,
        // avoid having to borrow window mutably
        // when invalidating
        up_views: RefCell<BinaryHeap<InvalidatedEntry<P::Env>>>,
        down_views: RefCell<BinaryHeap<InvalidatedEntry<P::Env>>>,
        performing_layout_down: Cell<bool>,

        /* native */
        handle: WindowHandle,
        menu: WindowMenu,
        content_view: Arc<MainSlockCell<dyn InnerViewBase<P::Env>>>
    }

    impl<P> Window<P> where P: WindowProvider {
        // order things are done is a bit awkward
        // but need to coordinate between many things
        pub(super) fn new(provider: P, s: MSlock) -> Arc<MainSlockCell<dyn WindowNativeCallback>> {
            let root_env = P::Env::root_environment();

            let handle = native::window::window_init(s);
            let content_view = provider.root(root_env.const_env(), s)
                .into_view(s).0;

            let menu = provider.menu(root_env.const_env(), s);

            let window = Window {
                provider,
                last_cursor: Cell::new(Point::new(util::geo::UNBOUNDED, util::geo::UNBOUNDED)),
                focus: RefCell::new(None),
                scheduled_focus: Cell::new(None),
                default_focus: RefCell::new(Vec::new()),
                key_listeners: RefCell::new(Vec::new()),
                environment: Cell::new(Some(Box::new(root_env))),
                up_views: RefCell::new(BinaryHeap::new()),
                down_views: RefCell::new(BinaryHeap::new()),
                performing_layout_down: Cell::new(false),
                handle,
                menu,
                content_view
            };

            let b = Arc::new(MainSlockCell::new_main(window, s));

            // create initial tree and other tasks
            Window::init(&b, s);

            // set handle of backing as well as root view
            {
                let borrow = b.borrow_main(s);
                native::window::window_set_handle(handle, borrow.deref() as &dyn WindowNativeCallback, s);

                let content_borrow = borrow.content_view.borrow_main(s);
                debug_assert!(!content_borrow.native_view().is_null());
                native::window::window_set_root(handle, content_borrow.native_view(), s);
            }

            b
        }

        // exact order is pretty important
        // (view depends on menu being before)
        fn init(this: &Arc<MainSlockCell<Self>>, s: MSlock) {
            let borrow = this.borrow_main(s);

            /* apply style */
            Self::apply_window_style(borrow.deref(), s);

            /* apply window title  */
            Self::window_listeners(&this, borrow.deref(), s);

            /* menu bar */
            drop(borrow);
            let mut borrow = this.borrow_mut_main(s);
            Self::mount_menu(borrow.deref_mut(), s);

            /* mount content view */
            drop(borrow);
            let borrow = this.borrow_main(s);
            Self::mount_content_view(&this, s, borrow.deref());

        }

        // logic is a bit tricky for initial mounting
        // due to reentry
        fn mount_content_view(this: &Arc<MainSlockCell<Window<P>>>, s: MSlock, borrow: &Window<P>) {
            let weak_window = Arc::downgrade(this)
                as Weak<MainSlockCell<dyn WindowViewCallback<P::Env>>>;

            let mut stolen_env = borrow.environment.take().unwrap();
            let content_copy = borrow.content_view.clone();

            let mut content_borrow = content_copy.borrow_mut_main(s);
            content_borrow.show(&borrow.content_view, &weak_window, stolen_env.deref_mut(), 0u32, s);

            drop(content_borrow);
            borrow.layout_up(stolen_env.deref_mut(), None, -1, s);
            let mut content_borrow = content_copy.borrow_mut_main(s);

            // now we must finish the layout down
            let intrinsic = borrow.provider.size(stolen_env.const_env(), s).1;
            content_borrow.try_layout_down(
                &borrow.content_view,
                stolen_env.deref_mut(),
                Some(Rect::new(0.0, 0.0, intrinsic.w, intrinsic.h)),
                s
            ).unwrap();

            // give back env
            borrow.environment.set(Some(stolen_env));
        }

        fn apply_window_style(borrow: &Window<P>, s: MSlock) {
            let stolen_env = borrow.environment.take().unwrap();

            // set window size (note no recursive layout call can happen since handle not mounted yet)
            let sizes = borrow.provider.size(stolen_env.const_env(), s);
            native::window::window_set_min_size(borrow.handle, sizes.0.w, sizes.0.h, s);
            native::window::window_set_size(borrow.handle, sizes.1.w, sizes.1.h, s);
            native::window::window_set_max_size(borrow.handle, sizes.2.w, sizes.2.h, s);

            borrow.environment.set(Some(stolen_env));
        }

        fn window_listeners(this: &Arc<MainSlockCell<Window<P>>>, borrow: &Window<P>, s: MSlock) {
            let stolen_env = borrow.environment.take().unwrap();

            // title
            {
                let title = borrow.provider.title(stolen_env.const_env(), s);
                let weak = Arc::downgrade(&this);
                title.diff_listen(move |val, s| {
                    let Some(this) = weak.upgrade() else {
                        return false;
                    };

                    let title_copy = val.to_owned();
                    run_main_maybe_sync(move |s| {
                        let borrow = this.borrow_main(s);
                        native::window::window_set_title(borrow.handle, &title_copy, s);
                    }, s);

                    true
                }, s);

                let current = title.borrow(s).to_owned();
                native::window::window_set_title(borrow.handle, &current, s);
            }

            // open
            {
                let open = borrow.provider.is_open(stolen_env.const_env(), s);
                let handle = borrow.handle();
                open.diff_listen(move |a, _s| {
                    if !a {
                        // we do not run synchronous
                        // because theres possibility of multiple borrows
                        // (once we finally perform the hide)
                        run_main_async(move |s| {
                            APP.with(|app| {
                                let mut windows = app.get().unwrap().windows.borrow_mut();
                                if let Some(pos) = windows
                                    .iter()
                                    .position(|w| w.borrow_main(s).handle() == handle)
                                {
                                    {
                                        let window = windows[pos].borrow_main(s);
                                        window.hide_root(s);
                                    }
                                    windows.remove(pos);
                                }
                            });
                            window_exit(handle, s);
                        });
                    }
                    true
                }, s);
            }

            // fullscreen
            {
                let fs = borrow.provider.is_fullscreen(stolen_env.const_env(), s);
                let weak = Arc::downgrade(&this);
                fs.diff_listen(move |val, _s| {
                    let Some(this) = weak.upgrade() else {
                        return false;
                    };

                    // avoid reentry of layout by delayed call
                    let fs = *val;
                    run_main_async(move |s| {
                        let borrow = this.borrow_main(s);
                        native::window::window_set_fullscreen(borrow.handle, fs, s);
                    });

                    true
                }, s);

                if *fs.borrow(s) {
                    native::window::window_set_fullscreen(borrow.handle, true, s);
                }
            }

            borrow.environment.set(Some(stolen_env));
        }

        fn mount_menu(borrow: &mut Window<P>, s: MSlock) {
            let stolen_env = borrow.environment.take().unwrap();

            window_set_menu(borrow.handle, &mut borrow.menu, s);

            borrow.environment.set(Some(stolen_env));
        }

        // env is currently right below from
        // and has to be moved right below to
        // if it must cross the min_depth, it will return false
        // (and the env will be left at the "subtree root")
        fn walk_env(
            env: &mut P::Env,
            curr: &mut Option<Arc<MainSlockCell<dyn InnerViewBase<P::Env>>>>,
            to: Option<Arc<MainSlockCell<dyn InnerViewBase<P::Env>>>>,
            curr_depth: &mut i32,
            min_depth: i32,
            s: MSlock
        ) -> bool {
            // equalize level
            let mut targ_stack = vec![];

            let mut targ_depth = to.as_ref().map(|t| t.borrow_main(s).depth() as i32).unwrap_or(-1);

            let mut targ = to;
            while *curr_depth > targ_depth {
                if *curr_depth == min_depth {
                    // about to perform double borrow, abort
                    return false;
                }

                *curr = {
                    let mut borrow = curr.as_mut().unwrap().borrow_mut_main(s);
                    borrow.pop_environment(env, s);
                    borrow.superview()
                };
                *curr_depth -= 1;
            }

            // (while not equal)
            while !(
                (curr.is_none() && targ.is_none()) ||
                    (curr.is_some() && targ.is_some() &&
                        std::ptr::addr_eq(curr.as_ref().unwrap().as_ptr(), targ.as_ref().unwrap().as_ptr())
                    )
            )
            {
                if *curr_depth == targ_depth {
                    if *curr_depth == min_depth {
                        return false;
                    }

                    // need to advance curr as well
                    *curr = {
                        let mut borrow = curr.as_mut().unwrap().borrow_mut_main(s);
                        borrow.pop_environment(env, s);
                        borrow.superview()
                    };
                    *curr_depth -= 1;
                }

                targ = {
                    let targ_ref = targ.as_mut().unwrap();
                    targ_stack.push(targ_ref.clone());
                    let res = targ_ref.borrow_main(s).superview();
                    res
                };
                targ_depth -= 1;
            }

            // walk towards the target
            for node in targ_stack.into_iter().rev() {
                node.borrow_mut_main(s)
                    .push_environment(env, s);
            }

            true
        }

        fn clear_focus_request(&self, s: MSlock) {
            // if different, notify ancestors
            let scheduled = self.scheduled_focus.take();
            let curr = self.focus.borrow().as_ref().map(|o| o.as_ptr());
            if scheduled.as_ref().map(|s| s.as_ptr()) != curr {
                let mut depth = 0u32;
                let mut it = self.focus.borrow().as_ref().and_then(|a| a.upgrade());
                while let Some(curr) = &it {
                    it = {
                        let borrow = curr.borrow_main(s);
                        borrow.unfocused(depth, s);
                        borrow.superview()
                    };
                    depth += 1
                }

                depth = 0u32;
                it = scheduled.as_ref().and_then(|a| a.upgrade());
                while let Some(curr) = &it {
                    it = {
                        let borrow = curr.borrow_main(s);
                        borrow.focused(depth, s);
                        borrow.superview()
                    };
                    depth += 1
                }

                *self.focus.borrow_mut() = scheduled.clone();
                self.scheduled_focus.set(scheduled);
            }
            else {
                self.scheduled_focus.set(scheduled);
            }
        }
    }

    impl<P> WindowNativeCallback for Window<P> where P: WindowProvider {
        fn can_close(&self, s: MSlock) -> bool {
            // let can_close = self.provider.can_close(s);
            let can_close = true;
            if can_close {
                self.hide_root(s);

                APP.with(|app| {
                    app.get().unwrap()
                        .windows
                        .borrow_mut()
                        .retain(|w| w.borrow_main(s).handle() != self.handle);
                });
            }

            can_close
        }

        fn hide_root(&self, s: MSlock) {
            self.content_view.borrow_mut_main(s)
                .hide(s);
        }

        fn handle(&self) -> WindowHandle {
            self.handle
        }

        fn layout_full(&self, w: f64, h: f64, s: MSlock) {
            let mut env = self.environment.take().unwrap();
            self.layout_up(env.deref_mut(), None, -1, s);

            // handle layout down
            self.performing_layout_down.set(true);
            let mut env_spot = None;
            let mut env_depth: i32 = -1;

            // if no screen size change, it will exit early
            self.content_view.borrow_mut_main(s)
                .try_layout_down(&self.content_view, env.deref_mut(), Some(Rect::new(0.0, 0.0, w, h)), s)
                .unwrap();

            while let Some(curr) = self.down_views.borrow_mut().pop() {
                /* ensure that view is still valid */
                let Some(view) = curr.view.upgrade() else {
                    continue;
                };

                let borrow = view.borrow_main(s);
                /* make sure it doesn't have a newer entry */
                if borrow.depth() as i32 != -curr.depth || !borrow.needs_layout_down() {
                    continue;
                }

                drop(borrow);
                debug_assert!(Self::walk_env(env.deref_mut(), &mut env_spot, Some(view.clone()), &mut env_depth, -1, s));
                let mut borrow = view.borrow_mut_main(s);

                // try to layout down
                // if fail must mean we need to schedule a new layout of the parent
                // as this node requires context
                if let Err(_) = borrow.try_layout_down(&view, env.deref_mut(), None, s) {
                    // superview must exist since otherwise layout
                    // wouldn't have failed
                    let superview = borrow.superview().unwrap();
                    superview.borrow_mut_main(s).set_needs_layout_down();

                    self.down_views.borrow_mut()
                        .push(InvalidatedEntry {
                            view: Arc::downgrade(&superview),
                            // note the negative ops for reverse ordering of depth
                            depth: curr.depth + 1
                        });
                }
            }

            // it has not parent so we must finalize it
            self.content_view.borrow_main(s)
                .finalize_view_frame(s);

            Self::walk_env(env.deref_mut(), &mut env_spot, None, &mut env_depth, -1, s);
            self.environment.set(Some(env));
            self.performing_layout_down.set(false);

            self.clear_focus_request(s);
        }

        // FIXME, when weak fails to upgrade make the option None
        fn dispatch_native_event(&self, mut event: Event, s: MSlock) {
            match &mut event.payload {
                EventPayload::Mouse(_, at) => {
                    // match cursor
                    let cv = self.content_view.borrow_mut_main(s);
                    *at = at.translate(-cv.view_rect(s).origin());
                    let cursor = *at;

                    cv.handle_mouse_event(&self.content_view, &mut event, self.last_cursor.take(), s);

                    self.last_cursor.set(cursor);
                },
                EventPayload::Key(_) => 'key: {
                    // if focus also in key listeners, only do one at a time
                    let mut already_handled: Option<*const MainSlockCell<dyn InnerViewBase<P::Env>>> = None;

                    let mut handled_key_event = |target: Arc<MainSlockCell<dyn InnerViewBase<P::Env>>>| {
                        match target.borrow_mut_main(s)
                            .handle_key_event(&mut event, s) {
                            EventResult::NotHandled => false,
                            EventResult::Handled => true,
                            EventResult::FocusRelease => {
                                self.unrequest_focus(Arc::downgrade(&target));
                                false
                            },
                            EventResult::FocusAcquire => {
                                self.request_focus(Arc::downgrade(&target));
                                true
                            }
                        }
                    };

                    if let Some(focus) = self.focus.borrow().deref().as_ref().and_then(|f| f.upgrade()) {
                        // 1. focus
                        already_handled = Some(Arc::as_ptr(&focus));
                        if handled_key_event(focus) {
                            break 'key;
                        }
                    }
                    else if let Some(default_focus) = self.default_focus.borrow().deref()
                        .first().and_then(|f| f.upgrade()) {
                        // 2. autofocus
                        already_handled = Some(Arc::as_ptr(&default_focus));
                        if handled_key_event(default_focus) {
                            break 'key;
                        }
                    }

                    // 3. key listeners
                    for listener in self.key_listeners.borrow().iter() {
                        if let Some(listener) = listener.upgrade() {
                            // skip if was already focused or autofocused
                            if let Some(handled) = already_handled {
                                if std::ptr::addr_eq(Arc::as_ptr(&listener), handled) {
                                    continue
                                }
                            }

                            // you cannot acquire focus by being a key listener (at least for now)
                            listener.borrow_mut_main(s)
                                .handle_key_event(&mut event, s);
                        }
                    }
                }
            }

            self.clear_focus_request(s)
        }

        fn set_fullscreen(&self, fs: bool, s: MSlock) {
            let stolen_env = self.environment.take().unwrap();

            let binding = self.provider.is_fullscreen(stolen_env.const_env(), s);
            if *binding.borrow(s) != fs {
                binding.apply(Set(fs), s);
            }

            self.environment.set(Some(stolen_env));
        }
    }

    impl<P> WindowViewCallback<P::Env> for Window<P> where P: WindowProvider {
        fn layout_up(&self, env: &mut P::Env, right_below: Option<Arc<MainSlockCell<dyn InnerViewBase<P::Env>>>>, depth: i32, s: MSlock) {
            // the environment is right above this node
            let mut env_spot = right_below.clone();
            let mut env_depth = depth;

            // generally very rare (some portals outside of subtree)
            let mut unhandled = vec![];
            while !self.up_views.borrow().is_empty() {
                let curr = {
                    let mut borrow = self.up_views.borrow_mut();
                    // finished subtree
                    if borrow.peek().unwrap().depth <= depth {
                        break;
                    }

                    borrow.pop().unwrap()
                };

                let Some(view) = curr.view.upgrade() else {
                    continue;
                };

                let view_borrow = view.borrow_main(s);

                /* make sure it doesn't have a newer entry */
                if view_borrow.depth() as i32 != curr.depth || !view_borrow.needs_layout_up() {
                    continue;
                }

                // move environment to target
                drop(view_borrow);
                if !Self::walk_env(env, &mut env_spot.clone(), Some(view.clone()), &mut env_depth, depth, s) {
                    // must be out of scope, mark as unhandled
                    unhandled.push(curr);
                    continue;
                }

                let mut view_mut_borrow = view.borrow_mut_main(s);

                let superview = view_mut_borrow.superview();
                if view_mut_borrow.layout_up(&view, env, s) && superview.is_some() {
                    // we have to schedule parent (if it's in range)
                    let unwrapped = superview.unwrap();
                    if view_mut_borrow.depth() != (depth + 1) as u32 {
                        unwrapped.borrow_mut_main(s)
                            .set_needs_layout_up();
                    }
                    else {
                        // I believe we actually don't have to do anything
                        // as this can only happen during a layout_up call of `right_below`
                        // and thus the parent is already performing its layout up
                        // so there is no need to even add it to unhandled
                    }

                    self.up_views.borrow_mut()
                        .push(InvalidatedEntry {
                            view: Arc::downgrade(&unwrapped),
                            depth: curr.depth - 1
                        });
                }
                else {
                    // schedule down layout of self
                    self.down_views.borrow_mut()
                        .push(InvalidatedEntry {
                            view: Arc::downgrade(&view),
                            // invert ordering
                            depth: -curr.depth
                        });
                }
            }

            let mut up_views = self.up_views.borrow_mut();
            for not_done in unhandled {
                up_views.push(not_done);
            }

            // put back env to root
            // for complex borrowing reasons, we can't just do right_below
            // as the target (as then it will have to measure right_below's depth, multiple borrow)
            // instead we just tell it to try to walk all the way to the actual root
            // but make sure it never crosses the min_depth, and hence get the desired behavior
            // of moving it to right_below without any multiple borrows
            Self::walk_env(env, &mut env_spot, None, &mut env_depth, depth, s);
        }

        fn invalidate_view(&self, handle: Weak<MainSlockCell<dyn WindowViewCallback<P::Env>>>, view: Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>, depth: u32, s: Slock) {
            assert!(!self.performing_layout_down.get(), "Cannot call invalidator while performing layout down (try to move to layout_up instead)!");

            // note that we're only touching send parts of self
            let mut borrow = self.up_views.borrow_mut();
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

        fn request_focus(&self, view: Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>) {
            self.scheduled_focus.set(Some(view));
        }

        fn unrequest_focus(&self, view: Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>) {
            let comp = self.scheduled_focus.take();
            if comp.as_ref().map(|w| Weak::as_ptr(&w)) == Some(view.as_ptr()) {
                self.scheduled_focus.set(None)
            }
            else {
                self.scheduled_focus.set(comp);
            }
        }

        fn request_default_focus(&self, view: Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>) {
            let mut borrow = self.default_focus.borrow_mut();
            if !borrow.iter().any(|w| std::ptr::addr_eq(w.as_ptr(), view.as_ptr())) {
                borrow.push(view)
            }
        }

        fn unrequest_default_focus(&self, view: Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>) {
            self.default_focus.borrow_mut()
                .retain(|w| !std::ptr::addr_eq(w.as_ptr(), view.as_ptr()))
        }

        fn request_key_listener(&self, view: Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>) {
            let mut borrow = self.key_listeners.borrow_mut();
            if !borrow.iter().any(|w| std::ptr::addr_eq(w.as_ptr(), view.as_ptr())) {
                borrow.push(view)
            }
        }

        fn unrequest_key_listener(&self, view: Weak<MainSlockCell<dyn InnerViewBase<P::Env>>>) {
            self.key_listeners.borrow_mut()
                .retain(|w| !std::ptr::eq(w.as_ptr(), view.as_ptr()))
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
    use std::sync::{Mutex, MutexGuard};
    use std::thread;
    use crate::native;
    use crate::util::marker::{AnyThreadMarker, MainThreadMarker, ThreadMarker};
    use crate::util::rust_util::PhantomUnsendUnsync;
    use crate::core::debug_stats::DebugInfo;

    static GLOBAL_STATE_LOCK: Mutex<()> = Mutex::new(());
    static SLOCK_INIT_LISTENER: Mutex<Vec<Box<dyn FnMut(Slock) -> bool + Send>>> = Mutex::new(Vec::new());
    static SLOCK_DROP_LISTENER: Mutex<Vec<Box<dyn FnMut(Slock) -> bool + Send>>> = Mutex::new(Vec::new());
    #[cfg(debug_assertions)]
    static LOCKED_THREAD: Mutex<Option<thread::ThreadId>> = Mutex::new(None);

    #[allow(unused)]
    pub struct SlockOwner<M=AnyThreadMarker> where M: ThreadMarker {
        _guard: MutexGuard<'static, ()>,
        // if forced, then don't do regular dealloc
        is_nested: bool,
        pub(crate) debug_info: DebugInfo,
        unsend_unsync: PhantomUnsendUnsync,
        thread_marker: PhantomData<M>,
    }

    #[cfg(debug_assertions)]
    #[derive(Copy, Clone)]
    #[allow(unused)]
    pub struct Slock<'a, M=AnyThreadMarker> where M: ThreadMarker {
        pub(crate) owner: &'a SlockOwner<M>,
        // unnecessary? but to emphasize
        unsend_unsync: PhantomUnsendUnsync,
    }

    #[cfg(not(debug_assertions))]
    #[derive(Copy, Clone)]
    #[allow(unused)]
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

    pub fn slock_init_listener(f: impl FnMut(Slock) -> bool + Send + 'static) {
        SLOCK_INIT_LISTENER.lock().unwrap()
            .push(Box::new(f))
    }

    pub fn slock_drop_listener(f: impl FnMut(Slock) -> bool + Send + 'static) {
        SLOCK_DROP_LISTENER.lock().unwrap()
            .push(Box::new(f))
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
        let ret = SlockOwner {
            _guard: global_guard(),
            debug_info: DebugInfo::new(),
            unsend_unsync: PhantomData,
            thread_marker: PhantomData,
            is_nested: false,
        };
        SLOCK_INIT_LISTENER.lock().unwrap()
            .retain_mut(|f| f(ret.marker()));
        ret
    }

    #[inline]
    pub fn slock_main_owner() -> SlockOwner<MainThreadMarker> {
        if !native::global::is_main() {
            panic!("Cannot call slock_main_owner outside of main thread")
        }

        let ret = SlockOwner {
            _guard: global_guard(),
            is_nested: false,
            debug_info: DebugInfo::new(),
            unsend_unsync: PhantomData,
            thread_marker: PhantomData,
        };

        SLOCK_INIT_LISTENER.lock().unwrap()
            .retain_mut(|f| f(ret.marker().to_general_slock()));

        ret
    }

    // some ffi makes it awkward to pass slock arround
    // If you are sure the thread currently owns the slock
    // you can call this method
    pub unsafe fn slock_force_main_owner() -> SlockOwner<MainThreadMarker> {
        static FAKE_GLOBAL_STATE_LOCK: Mutex<()> = Mutex::new(());

        // even with these checks, it's still unsafe due to lifetimes
        if !native::global::is_main() {
            panic!("Cannot force slock owner")
        }

        let current = LOCKED_THREAD.lock().unwrap();
        if current.is_none() || *current != Some(thread::current().id()) {
            drop(current);
            return slock_main_owner();
        }

        SlockOwner {
            _guard: FAKE_GLOBAL_STATE_LOCK.lock().unwrap(),
            is_nested: true,
            debug_info: DebugInfo::new(),
            unsend_unsync: PhantomData,
            thread_marker: PhantomData,
        }
    }

    impl<M> SlockOwner<M> where M: ThreadMarker {
        // note that the global state lock is kept for entire
        // lifetime of slockowner; calling marker does not acquire the state lock
        // and dropping the marker does not relenquish it
        #[inline]
        pub fn marker(&self) -> Slock<M> {
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
    impl<M> Drop for SlockOwner<M> where M: ThreadMarker {
        fn drop(&mut self) {
            if !self.is_nested {
                *LOCKED_THREAD.lock().unwrap() = None;

                SLOCK_DROP_LISTENER.lock().unwrap()
                    .retain_mut(|f| f(self.marker().to_general_slock()));
            }
        }
    }

    impl<'a, M> Slock<'a, M> where M: ThreadMarker {
        pub fn try_to_main_slock(self) -> Option<MSlock<'a>> {
            if !native::global::is_main() {
                None
            }
            else {
                // safety:
                // data layouts of us and owner are the same
                // and we confirmed we're on the main thread
                unsafe {
                    Some(std::mem::transmute::<Slock<'a, M>, MSlock<'a>>(self))
                }
            }
        }

        /// Given a slock that may be say the main slock
        /// convert it into the general slock
        /// Some methods require the general slock
        pub fn to_general_slock(self) -> Slock<'a> {
            // safety: if a slock comes from a specific thread
            // it certainly came from any thread.
            // The data layouts of the reference field are exactly
            // the same
            // (the layout of slock are certainly the same)
            // (and the layout of slock owner are the same)
            unsafe {
                std::mem::transmute::<Self, Slock<'a>>(self)
            }
        }
    }
}
pub use slock::*;

mod global {
    use std::time::{Duration, Instant};
    use crate::core::application::{Application, ApplicationProvider};
    use crate::native;
    use crate::state::{CapacitatedSignal, FixedSignal};
    use crate::state::capacitor::IncreasingCapacitor;
    use crate::util::marker::ThreadMarker;
    use super::{APP, MSlock, Slock, TIMER_WORKER};

    pub fn timed_worker<F: for<'a> FnMut(Duration, Slock<'a>) -> bool + Send + 'static>(func: F) {
        TIMER_WORKER.get()
            .expect("Cannot call quarve functions before launch!")
            .send((Box::new(func), Instant::now()))
            .unwrap()
    }

    pub fn clock_signal(s: Slock<impl ThreadMarker>) -> CapacitatedSignal<IncreasingCapacitor> {
        let constant = FixedSignal::new(0.0);
        CapacitatedSignal::from(&constant, IncreasingCapacitor, s)
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
        if let Some(main) = s.try_to_main_slock() {
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
    pub fn with_app(f: impl FnOnce(&Application), _s: MSlock) {
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