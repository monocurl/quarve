use std::any::Any;
use std::cell::Cell;
use std::ffi::c_void;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::core::{Environment, MSlock, Slock, WindowViewCallback};
use crate::event::{Event, EventResult};
use crate::native;
use crate::native::backend::AUTO_CLIPS_CHILDREN;
use crate::native::view::{view_add_child_at, view_clear_children, view_remove_child, view_set_frame};
use crate::state::Buffer;
use crate::state::slock_cell::MainSlockCell;
use crate::util::geo;
use crate::util::geo::{Point, Rect, ScreenUnit, Size};
use crate::util::rust_util::PhantomUnsendUnsync;
use crate::view::{EnvRef, View, WeakInvalidator};
use crate::view::util::SizeContainer;
use crate::view::view_provider::ViewProvider;

pub(crate) trait InnerViewBase<E> where E: Environment {
    /* native methods */

    // must be called after show was called at least once
    // (otherwise will likely be null)
    fn native_view(&self) -> *mut c_void;

    /* tree methods */
    fn superview(&self) -> Option<Arc<MainSlockCell<dyn InnerViewBase<E>>>>;
    fn set_superview(&mut self, superview: Option<Weak<MainSlockCell<dyn InnerViewBase<E>>>>);
    fn graph(&self) -> &Graph<E>;
    fn mut_graph(&mut self) -> &mut Graph<E>;

    fn depth(&self) -> u32;

    // true if handled
    fn handle_mouse_event(&self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, event: &mut Event, prev_position: Point, focused: bool, s: MSlock) -> bool;
    // does not recurse
    fn handle_key_event(&mut self, event: &mut Event, s: MSlock) -> EventResult;

    fn unfocused(&self, rel_depth: u32, s: MSlock);
    fn focused(&self, rel_depth: u32, s: MSlock);

    /* layout methods */
    fn needs_layout_up(&self) -> bool;
    fn needs_layout_down(&self) -> bool;

    // true if we need to go to the parent and lay that up
    fn layout_up(&mut self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, env: &mut E, s: MSlock) -> bool;

    // fails if the current view requires context
    // in such a case, we must go to the parent and retry
    // frame should be the new frame for layout
    // or null if we are to use the last frame
    // this should only be done if we know for sure
    // the last frame is valid
    fn try_layout_down(&mut self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, env: &mut E, frame: Option<Rect>, s: MSlock) -> Result<(), ()>;
    fn translate(&mut self, by: Point, s: MSlock);
    // dispatches calculated frame to native
    fn finalize_view_frame(&self, s: MSlock);

    fn scroll_offset(&self, s: MSlock) -> Point;
    fn view_rect_in_window(&self, s: MSlock) -> Rect;
    fn view_rect(&self, s: MSlock) -> Rect;
    fn used_rect(&self, s: MSlock) -> Rect;
    fn suggested_rect(&self, _s: MSlock) -> Rect;
    fn bounding_rect(&self, _s: MSlock) -> Rect;

    fn intrinsic_size(&mut self, s: MSlock) -> Size;
    fn xsquished_size(&mut self, s: MSlock) -> Size;
    fn xstretched_size(&mut self, s: MSlock) -> Size;
    fn ysquished_size(&mut self, s: MSlock) -> Size;
    fn ystretched_size(&mut self, s: MSlock) -> Size;
    fn sizes(&mut self, s: MSlock) -> SizeContainer;

    /* mounting and unmounting */
    fn show(
        &mut self,
        this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>,
        window: &Weak<MainSlockCell<dyn WindowViewCallback<E>>>,
        e: &mut E,
        depth: u32,
        s: MSlock
    );

    fn hide(&mut self, s: MSlock);


    // this is done whenever a node has layout context
    // and thus cannot be layed out trivially so that the
    // parent must have its layout down flag set to true
    // even though it doesn't need a layout up
    fn set_needs_layout_down(&self);
    fn set_needs_layout_up(&self);
    fn invalidate(&self, this: Weak<MainSlockCell<dyn InnerViewBase<E>>>, s: Slock);

    /* environment */
    fn push_environment(&mut self, env: &mut E, s: MSlock);
    fn pop_environment(&mut self, env: &mut E, s: MSlock);
}

// contains a backing and
pub(crate) struct InnerView<E, P> where E: Environment,
                                        P: ViewProvider<E> {
    // parent, subviews, depth, backing, etc
    graph: Graph<E>,

    needs_layout_up: Cell<bool>,
    needs_layout_down: Cell<bool>,
    // wonder if there is better solution
    performing_up: Arc<AtomicBool>,

    /* cached layout results */
    last_suggested: Rect,
    last_exclusion: Rect,
    last_view_frame: Rect,
    // union of this view frame and view frame of all others
    last_bounding_rect: Rect,

    /* provider */
    provider: P
}

impl<E, P> InnerView<E, P> where E: Environment, P: ViewProvider<E> {
    pub(super) fn into_backing_and_provider(self) -> (NativeView, P) {
        (self.graph.native_view, self.provider)
    }

    #[inline]
    pub(super) fn is_trivial_context(&self) -> bool {
        std::any::TypeId::of::<P>() == std::any::TypeId::of::<()>()
    }

    pub(super) fn provider(&mut self) -> &mut P {
        &mut self.provider
    }

    // returns position of rect
    // in superview coordinate system
    // expects environment to be below this view
    pub(super) fn layout_down_with_context(
        &mut self,
        this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>,
        suggested: Rect,
        env: &mut E,
        context: &P::DownContext,
        s: MSlock
    ) -> Rect {
        // all writes to dirty flag are done with a state lock
        // we may set the dirty flag to false now that we are performing a layout
        let mut actually_needs_layout = self.needs_layout_down.get();
        // if context isn't trivial, there may be updates
        // that were not taken into account
        actually_needs_layout = actually_needs_layout
            || !self.is_trivial_context();
        // if frame is different from last frame (except for just translation)
        // maybe different overall frame
        actually_needs_layout = actually_needs_layout
            || suggested.size() != self.last_suggested.size();

        let (view_frame, exclusion) = if actually_needs_layout {
            let subtree = Subtree {
                graph: &mut self.graph,
                owner: this,
            };
            let (actual_frame, exclusion) = self.provider.layout_down(&subtree, suggested.size(), context, &mut EnvRef(env), s);

            /* children were mounted with respect to the origin, but the used may translate
               that somewhat, so we must negate this account
             */
            if actual_frame.x != 0.0 || actual_frame.y != 0.0 {
                let inverse_children_transform = Point::new(-actual_frame.x, -actual_frame.y);
                for child in self.graph().subviews() {
                    child.borrow_mut_main(s)
                        .translate(inverse_children_transform, s);
                }
            }

            (actual_frame.translate(suggested.origin()), exclusion.translate(suggested.origin()))
        }
        else {
            let at = suggested.origin();
            let delta = Point::new(at.x - self.last_suggested.x, at.y - self.last_suggested.y);

            (self.last_view_frame.translate(delta), self.last_exclusion.translate(delta))
        };

        self.last_suggested = suggested;
        self.last_exclusion = exclusion;
        self.last_view_frame = view_frame;
        self.last_bounding_rect = view_frame;
        if !self.graph.native_view.clips_subviews {
            for sub in &self.graph.subviews {
                let in_our_frame = sub.borrow_main(s).bounding_rect(s);
                self.last_bounding_rect = self.last_bounding_rect
                    .union(in_our_frame.translate(view_frame.origin()))
            }
        }

        self.needs_layout_down.set(false);

        let expand_self = AUTO_CLIPS_CHILDREN && !self.graph.native_view.clips_subviews;
        let additional_translation = self.last_view_frame.origin() - self.last_bounding_rect.origin();

        // we cannot finalize our frame until parent has finished translate calls
        // but we can finalize subview frames
        self.graph.subviews
            .iter()
            .for_each(|sv| {
                let mut sv = sv.borrow_mut_main(s);
                if expand_self {
                    // translate according to inflated self
                    // if necessary
                    sv.translate(additional_translation, s);
                }
                sv.finalize_view_frame(s)
            });

        exclusion
    }

    pub(super) fn take_backing(
        &mut self,
        this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>,
        source: (NativeView, P),
        env: &mut EnvRef<E>,
        s: MSlock
    ) {
        if !self.native_view().is_null() {
            panic!("May not take backing from alt view when this backing has already been inited")
        }
        else if !source.0.backing.is_null() {
            // since our backing was not inited
            // we are guaranteed to have had zero children
            // and that we cannot have been shown already
            // therefore, show must be called on this view sometime in the future
            self.provider.push_environment(env.0.variable_env_mut(), s);

            let invalidator = WeakInvalidator {
                performing_up: Arc::clone(&self.performing_up),
                view: Arc::downgrade(this)
            };
            let mut subtree = Subtree {
                graph: &mut self.graph,
                owner: this
            };
            self.graph.native_view =
                self.provider.init_backing(
                invalidator,
                &mut subtree,
                Some(source),
                env,
                s
            );

            self.provider.pop_environment(env.0.variable_env_mut(), s);
        }
        // else: nothing to copy from so this is no op
    }
}

impl<E, P> InnerViewBase<E> for InnerView<E, P> where E: Environment, P: ViewProvider<E> {

    // unowned
    fn native_view(&self) -> *mut c_void {
        self.graph.native_view.backing
    }

    fn superview(&self) -> Option<Arc<MainSlockCell<dyn InnerViewBase<E>>>> {
        self.graph.superview.as_ref().and_then(|s| s.upgrade())
    }

    fn set_superview(&mut self, superview: Option<Weak<MainSlockCell<dyn InnerViewBase<E>>>>) {
        if self.graph.superview.is_some() && superview.is_some() {
            panic!("Attempt to add view to superview when the subview is already mounted to a view. \
                        Remove the view from the other view before proceeding");
        }

        self.graph.superview = superview;
    }

    fn graph(&self) -> &Graph<E> {
        &self.graph
    }

    fn mut_graph(&mut self) -> &mut Graph<E> {
        &mut self.graph
    }

    fn depth(&self) -> u32 {
        self.graph.depth
    }

    fn handle_mouse_event(&self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, event: &mut Event, prev_position: Point, focused: bool, s: MSlock) -> bool {
        let position = event.cursor();

        // last_bounding_rect is in parent coordinates
        // position is in our coordinates
        let vf_offset = self.last_view_frame.origin();
        let prev_contains = self.last_bounding_rect.contains(prev_position.translate(vf_offset));
        let curr_contains = self.last_bounding_rect.contains(position.translate(vf_offset));
        if !curr_contains && !prev_contains && !focused{
            return false;
        }

        // note that window handles sending out
        // key events, not us
        match self.provider.handle_event(event, s) {
            EventResult::Handled => return true,
            EventResult::FocusAcquire => {
                if let Some(window) = self.graph.window.as_ref().and_then(|w| w.upgrade()) {
                    window.borrow_main(s)
                        .request_focus(Arc::downgrade(this))
                }
                return true;
            },
            EventResult::NotHandled => (),
            EventResult::FocusRelease => {
                if let Some(window) = self.graph.window.as_ref().and_then(|w| w.upgrade()) {
                    window.borrow_main(s)
                        .unrequest_focus(Arc::downgrade(this))
                }
            }
        }

        let nv_delta = self.scroll_offset(s);

        self.graph.subviews.iter().rev()
            .any(|sv| {
                let borrow = sv.borrow_main(s);
                let delta = nv_delta - borrow.view_rect(s).origin();
                let prev = prev_position.translate(delta);

                event.set_cursor(position.translate(delta));

                borrow.handle_mouse_event(sv, event, prev, false, s)
            })
    }

    fn handle_key_event(&mut self, event: &mut Event, s: MSlock) -> EventResult {
        self.provider.handle_event(event, s)
    }

    fn unfocused(&self, rel_depth: u32, s: MSlock) {
        self.provider.unfocused(rel_depth, s)
    }

    fn focused(&self, rel_depth: u32, s: MSlock) {
        self.provider.focused(rel_depth, s)
    }

    fn needs_layout_up(&self) -> bool {
        self.needs_layout_up.get()
    }

    fn needs_layout_down(&self) -> bool {
        self.needs_layout_down.get()
    }

    fn layout_up(&mut self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, env: &mut E, s: MSlock) -> bool {
        debug_assert!(self.needs_layout_up());

        let mut handle = EnvRef(env);
        let mut subtree = Subtree {
            graph: &mut self.graph,
            owner: this,
        };

        self.performing_up.store(true, Ordering::SeqCst);
        let ret = self.provider.layout_up(&mut subtree, &mut handle, s);
        self.performing_up.store(false, Ordering::SeqCst);

        self.needs_layout_up.set(false);
        self.needs_layout_down.set(true);

        ret
    }

    fn try_layout_down(&mut self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, env: &mut E, frame: Option<Rect>, s: MSlock) -> Result<(), ()> {
        let context = ();
        let context_ref: &dyn Any = &context;

        if let Some(r) = context_ref.downcast_ref::<P::DownContext>() {
            self.layout_down_with_context(this, frame.unwrap_or(self.last_suggested), env, r, s);

            Ok(())
        }
        else {
            Err(())
        }
    }

    // basically for correction when parent frame rect is not actually grand parent suggested rect
    fn translate(&mut self, by: Point, _s: MSlock) {
        debug_assert!(!self.needs_layout_down.get());
        self.last_suggested = self.last_suggested.translate(by);
        self.last_exclusion = self.last_exclusion.translate(by);
        self.last_view_frame = self.last_view_frame.translate(by);
        self.last_bounding_rect = self.last_bounding_rect.translate(by);
    }

    fn finalize_view_frame(&self, s: MSlock) {
        debug_assert!(!self.needs_layout_down.get());
        // look at subviews
        let frame = if AUTO_CLIPS_CHILDREN && !self.graph.native_view.clips_subviews {
            self.last_bounding_rect
        }
        else {
            self.last_view_frame
        };

        self.provider.finalize_frame(self.last_view_frame, s);

        view_set_frame(self.native_view(), frame, s)
    }

    fn scroll_offset(&self, s: MSlock) -> Point {
        if let Some(borrow) = self.graph.native_view.state
            .as_ref()
            .map(|b| b.borrow(s)) {
            Point::new(borrow.offset_x, borrow.offset_y)
        }
        else {
            Point::new(0.0, 0.0)
        }
    }

    fn view_rect_in_window(&self, s: MSlock) -> Rect {
        let mut view = self.last_view_frame;
        let mut curr = self.graph.superview.as_ref().and_then(|c| c.upgrade());
        while let Some(at) = curr {
            let this = at.borrow_main(s);
            view = view.translate(this.view_rect(s).origin() - this.scroll_offset(s));
            curr = this.superview();
        }

        view
    }

    fn view_rect(&self, _s: MSlock) -> Rect {
        // this one it's okay to call even if it needs a layout down
        // for event purposes
        self.last_view_frame
    }

    fn used_rect(&self, _s: MSlock) -> Rect {
        debug_assert!(!self.needs_layout_down.get());
        self.last_exclusion
    }

    fn suggested_rect(&self, _s: MSlock) -> Rect {
        debug_assert!(!self.needs_layout_down.get());
        self.last_suggested
    }

    fn bounding_rect(&self, _s: MSlock) -> Rect {
        debug_assert!(!self.needs_layout_down.get());
        self.last_bounding_rect
    }

    fn intrinsic_size(&mut self, s: MSlock) -> Size {
        debug_assert!(!self.needs_layout_up() && self.depth() != u32::MAX,
                      "This method must only be called after the subview \
                      has been mounted to the parent. Also, this view cannot be in an invalidated/dirty state");
         self.provider.intrinsic_size(s)
    }

    fn xsquished_size(&mut self, s: MSlock) -> Size {
        debug_assert!(!self.needs_layout_up() && self.depth() != u32::MAX,
                      "This method must only be called after the subview \
                      has been mounted to the parent. Also, this view cannot be in an invalidated/dirty state");
        self.provider.xsquished_size(s)
    }

    fn xstretched_size(&mut self, s: MSlock) -> Size {
        debug_assert!(!self.needs_layout_up() && self.depth() != u32::MAX,
                      "This method must only be called after the subview \
                      has been mounted to the parent. Also, this view cannot be in an invalidated/dirty state");
        self.provider.xstretched_size(s)
    }

    fn ysquished_size(&mut self, s: MSlock) -> Size {
        debug_assert!(!self.needs_layout_up() && self.depth() != u32::MAX,
                      "This method must only be called after the subview \
                      has been mounted to the parent. Also, this view cannot be in an invalidated/dirty state");
        self.provider.ysquished_size(s)
    }

    fn ystretched_size(&mut self, s: MSlock) -> Size {
        debug_assert!(!self.needs_layout_up() && self.depth() != u32::MAX,
                      "This method must only be called after the subview \
                      has been mounted to the parent. Also, this view cannot be in an invalidated/dirty state");
        self.provider.ystretched_size(s)
    }

    fn sizes(&mut self, s: MSlock) -> SizeContainer {
        SizeContainer::new(
            self.intrinsic_size(s),
            self.xsquished_size(s),
            self.xstretched_size(s),
            self.ysquished_size(s),
            self.ystretched_size(s)
        )
    }

    fn show(
        &mut self,
        this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>,
        window: &Weak<MainSlockCell<dyn WindowViewCallback<E>>>,
        e: &mut E,
        depth: u32,
        s: MSlock
    ) {
        /* save attributes */
        let new_window = Some(window.clone());
        if self.graph.window.is_some() && !std::ptr::addr_eq(self.graph.window.as_ref().unwrap().as_ptr(), window.as_ptr()) {
            panic!("Cannot add view to different window than the original one it was mounted on!")
        }

        // the window will never be reset so this is the most effective
        // way of checking first mount. checking if native view is null
        // is not a good check since it could have been initialized via
        // a take backing call
        let first_mount = self.graph.window.is_none();
        self.graph.window = new_window;

        /* push environment */
        self.push_environment(e, s);

        /* init backing if necessary */
        if self.native_view().is_null() {
            let invalidator = WeakInvalidator {
                performing_up: Arc::clone(&self.performing_up),
                view: Arc::downgrade(this)
            };
            let mut handle = EnvRef(e);
            let mut subtree = Subtree {
                graph: &mut self.graph,
                owner: this,
            };
            self.graph.native_view = self.provider.init_backing(invalidator, &mut subtree, None, &mut handle, s);
        }

        // it is important to do this after the init_backing call
        // since otherwise there may be multiple show calls for the children
        self.graph.depth = depth;

        /* main notifications to provider and subtree */
        self.provider.pre_show(s);
        for (i, subview) in self.graph.subviews.iter().enumerate() {
            let mut borrow = subview.borrow_mut_main(s);
            borrow.show(
                subview,
                window,
                e,
                depth + 1,
                s
            );

            /* add subview if this first time backing allocated */
            if first_mount {
                view_add_child_at(self.graph.native_view.backing, borrow.native_view(), i, s);
            }
            view_add_child_at(self.graph.native_view.backing, borrow.native_view(), i, s);
        }
        self.provider.post_show(s);

        if self.needs_layout_up.get() {
            self.invalidate(Arc::downgrade(this), s.to_general_slock());
        }

        /* pop environment */
        self.pop_environment(e, s);
    }

    fn hide(&mut self, s: MSlock) {
        // keep window,
        // note that superview is responsible for removing itself
        // when appropriate
        self.graph.depth = u32::MAX;

        self.provider.pre_hide(s);
        for subview in &self.graph.subviews {
            subview.borrow_mut_main(s).hide(s);
        }
        self.provider.post_hide(s);
    }

    fn set_needs_layout_down(&self) {
        self.needs_layout_down.set(true);
    }

    fn set_needs_layout_up(&self) {
        self.needs_layout_up.set(true);
    }

    fn invalidate(&self, this: Weak<MainSlockCell<dyn InnerViewBase<E>>>, s: Slock) {
        self.needs_layout_up.set(true);
        self.needs_layout_down.set(true);

        // currently unmounted
        if self.graph.depth == u32::MAX {
            return;
        }

        if let Some(window) = self.graph.window.as_ref().and_then(|window| window.upgrade()) {
            // safety:
            // the only part of window we're touching
            // is send (guaranteed by protocol)
            unsafe {
                window.borrow_non_main_non_send(s)
                    .invalidate_view(Arc::downgrade(&window), this, self.graph.depth, s);
            }
        }
    }

    fn push_environment(&mut self, env: &mut E, s: MSlock) {
        self.provider.push_environment(env.variable_env_mut(), s);
    }

    fn pop_environment(&mut self, env: &mut E, s: MSlock) {
        self.provider.pop_environment(env.variable_env_mut(), s);
    }
}

pub struct NativeViewState {
    pub offset_x: ScreenUnit,
    pub offset_y: ScreenUnit
}

pub struct NativeView {
    backing: *mut c_void,
    clips_subviews: bool,
    /* 'live data', don't really like this but whatever */
    state: Option<Buffer<NativeViewState>>
}

impl Default for NativeViewState {
    fn default() -> Self {
        NativeViewState {
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

impl NativeView {
    pub unsafe fn new(owned_view: *mut c_void, _s: MSlock) -> NativeView {
        NativeView {
            backing: owned_view,
            clips_subviews: false,
            state: None,
        }
    }

    pub fn layout_view(s: MSlock) -> NativeView {
        unsafe {
            NativeView::new(native::view::init_layout_view(s), s)
        }
    }

    pub fn layer_view(s: MSlock) -> NativeView {
        unsafe {
            NativeView::new(native::view::layer::init_layer_view(s), s)
        }
    }

    pub fn backing(&self) -> *mut c_void {
        self.backing
    }

    pub fn set_clips_subviews(&mut self) {
        self.clips_subviews = true;
    }

    pub fn set_state(&mut self, state: Buffer<NativeViewState>) {
        self.state = Some(state);
    }
}

impl Drop for NativeView {
    fn drop(&mut self) {
        if !self.backing.is_null() {
            assert!(native::global::is_main());
            native::view::free_view(self.backing)
        }
    }
}

pub(crate) struct Graph<E> where E: Environment {
    native_view: NativeView,

    superview: Option<Weak<MainSlockCell<dyn InnerViewBase<E>>>>,
    window: Option<Weak<MainSlockCell<dyn WindowViewCallback<E>>>>,
    // u32::MAX indicates detached view
    depth: u32,

    subviews: Vec<Arc<MainSlockCell<dyn InnerViewBase<E>>>>,
    unsend_unsync: PhantomUnsendUnsync
}

// FIXME move subtree methods to graph
// and then have subtree delegate to graph
impl<E> Graph<E> where E: Environment {
    pub(crate) fn clear_subviews(&mut self, s: MSlock) {
        if !self.native_view.backing.is_null() {
            view_clear_children(self.native_view.backing, s);
        }

        for subview in std::mem::take(&mut self.subviews) {
            let mut borrow = subview.borrow_mut_main(s);
            borrow.set_superview(None);
            borrow.hide(s);
        }
    }

    pub(super) fn subviews(&self) -> &Vec<Arc<MainSlockCell<dyn InnerViewBase<E>>>> {
        &self.subviews
    }
}

pub struct Subtree<'a, E> where E: Environment {
    graph: &'a mut Graph<E>,
    owner: &'a Arc<MainSlockCell<dyn InnerViewBase<E>>>,
}

impl<'a, E> Subtree<'a, E> where E: Environment {
    pub fn len(&self) -> usize {
        self.graph.subviews.len()
    }

    pub fn contains<P: ViewProvider<E>>(&self, view: &View<E, P>) -> bool {
        self.graph.subviews.iter()
            .find(|v| std::ptr::addr_eq(v.as_ptr(), view.0.as_ptr()))
            .is_some()
    }

    fn dfs(curr: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, s: MSlock) {
        // safety is same reason invalidate above is safe
        // (only touching send parts)
        let borrow = curr.borrow_main(s);
        borrow.invalidate(Arc::downgrade(curr), s.to_general_slock());

        for subview in borrow.graph().subviews() {
            Subtree::dfs(subview, s);
        }
    }

    // i dont think this is the best place, but easiest
    // necessary for focus/keylisteners
    pub(crate) fn window(&self) -> Option<Weak<MainSlockCell<dyn WindowViewCallback<E>>>> {
        self.graph.window.clone()
    }

    pub(crate) fn owner(&self) -> &Arc<MainSlockCell<dyn InnerViewBase<E>>> {
        self.owner
    }

    // FIXME ugly hack to avoid reentry whenever
    // a environment_modifier wants to invalidate entire subtree
    // as a result of layout_up (it cant just call invalidtor
    // since that would reborrow the calling view, make it mutable and problem solved?)
    pub(crate) fn invalidate_subtree(&mut self, env: &mut EnvRef<E>, s: MSlock) {
        for sv in &self.graph.subviews {
            Subtree::dfs(sv, s);
        }

        self.ensure_subtree_has_layout_up_done(env, s);
    }

    fn ensure_subtree_has_layout_up_done(&mut self, env: &mut EnvRef<E>, s: MSlock) {
        // in this case, we are unmounted
        // the subtree will have its layout up done afterwards
        // this situation is generally encountered when init_backing
        // is called before mounting (which can itself happen due to take_backing)
        if self.graph.depth == u32::MAX {
            return;
        }

        let (window, depth) =
            (self.graph.window
                .as_ref()
                .and_then(|w| w.upgrade())
                .unwrap(),
             self.graph.depth);

        window.borrow_main(s)
            .layout_up(env.0, Some(self.owner.clone()), depth as i32, s);
    }

    pub fn remove_subview_at(&mut self, index: usize, env: &mut EnvRef<E>, s: MSlock) {
        // remove from backing
        if !self.graph.native_view.backing.is_null() {
            view_remove_child(self.graph.native_view.backing, index, s);
        }

        let removed = self.graph.subviews.remove(index);
        let mut borrow = removed.borrow_mut_main(s);
        borrow.set_superview(None);
        if self.graph.depth != u32::MAX {
            borrow.hide(s);
        }

        self.ensure_subtree_has_layout_up_done(env, s);
    }

    pub fn remove_subview<P>(&mut self, subview: &View<E, P>, env: &mut EnvRef<E>, s: MSlock) where P: ViewProvider<E> {
        let comp = subview.0.clone() as Arc<MainSlockCell<dyn InnerViewBase<E>>>;
        let index = self.graph.subviews.iter()
            .position(|u| Arc::ptr_eq(u, &comp))
            .expect("Input view should be a child of the current view");

        self.remove_subview_at(index, env, s);
    }

    pub fn clear_subviews(&mut self, s: MSlock) {
        self.graph.clear_subviews(s)

        // note that we need not ensure subtree has layout_up done in this case
        // since we are clearing the entire thing
        // (it may still be possible a "lower" view needs layout up
        // in the event that a portal sender in this subtree was removed causing an
        // invalidation of a receiver in some other subtree
        // but we don't really care about that)
    }

    // FIXME remove duplicate logic at some point?
    // this method should only be called by portals
    pub(crate) fn insert_arc_even_if_mounted_on_another_view(&mut self, subview: Arc<MainSlockCell<dyn InnerViewBase<E>>>, index: usize, env: &mut EnvRef<E>, s: MSlock) {
        let mut borrow = subview.borrow_mut_main(s);

        // forcefully override checks for prior view
        borrow.set_superview(None);
        if borrow.depth() != u32::MAX {
            borrow.hide(s);
        }

        if self.graph.depth != u32::MAX {
            let weak = self.graph.window.as_ref().unwrap().clone();
            let subview_this = subview.clone();
            borrow.show(&subview_this, &weak, env.0, self.graph.depth + 1, s);
        }

        borrow.set_superview(Some(Arc::downgrade(self.owner)));
        self.graph.subviews.insert(index, subview.clone());

        // add to backing
        if !self.graph.native_view.backing.is_null() {
            view_add_child_at(self.graph.native_view.backing, borrow.native_view(), index, s);
        }

        drop(borrow);
        self.ensure_subtree_has_layout_up_done(env, s);
    }

    // note that cyclic is technically possible if you work hard enough
    // but this will often just result in a stall or other weird effects
    pub fn insert_subview<P>(&mut self, subview: &View<E, P>, index: usize, env: &mut EnvRef<E>, s: MSlock) where P: ViewProvider<E> {
        self.graph.subviews.insert(index, subview.0.clone());
        let mut borrow = subview.0.borrow_mut_main(s);
        borrow.set_superview(Some(Arc::downgrade(self.owner)));


        // 1. we are currently mounted
        if self.graph.depth != u32::MAX {
            let weak = self.graph.window.as_ref().unwrap().clone();
            let subview_this = subview.0.clone() as Arc<MainSlockCell<dyn InnerViewBase<E>>>;
            borrow.show(&subview_this, &weak, env.0, self.graph.depth + 1, s);
        }

        // add to backing
        if !self.graph.native_view.backing.is_null() {
            view_add_child_at(self.graph.native_view.backing, borrow.native_view(), index, s);
        }

        drop(borrow);
        self.ensure_subtree_has_layout_up_done(env, s);
    }

    pub fn push_subview<P>(&mut self, subview: &View<E, P>, env: &mut EnvRef<E>, s: MSlock) where P: ViewProvider<E> {
        self.insert_subview(subview, self.graph.subviews.len(), env, s);
    }

    /* positional operations */

    // precondition: all subviews explicitly had their layout_down method
    // called
    pub fn translate_post_layout_down(&self, by: Point, s: MSlock) {
        if by.x.abs() < geo::EPSILON && by.y.abs() < geo::EPSILON {
            return;
        }

        for view in &self.graph.subviews {
            view.borrow_mut_main(s)
                .translate(by, s);
        }
    }
}

impl<E, P> InnerView<E, P> where E: Environment, P: ViewProvider<E> {
    pub(super) fn new(provider: P, s: MSlock) -> Arc<MainSlockCell<Self>> {
        Arc::new(
            MainSlockCell::new_main(InnerView {
                // marker that it is not on a tree
                graph: Graph {
                    depth: u32::MAX,
                    window: None,
                    superview: None,
                    native_view: NativeView {
                        backing: 0 as *mut c_void,
                        clips_subviews: false,
                        state: None,
                    },
                    subviews: vec![],
                    unsend_unsync: Default::default(),
                },
                needs_layout_down: Cell::new(true),
                needs_layout_up: Cell::new(true),
                last_suggested: Rect::default(),
                last_exclusion: Rect::default(),
                last_view_frame: Rect::default(),
                last_bounding_rect: Rect::default(),
                provider,
                performing_up: Arc::new(false.into()),
            }, s)
        )
    }
}