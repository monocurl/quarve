use std::any::Any;
use std::ffi::c_void;
use std::sync::{Arc, Weak};
use crate::core::{Environment, MSlock, Slock, WindowEnvironmentBase};
use crate::native;
use crate::native::view::{view_add_child_at, view_clear_children, view_remove_child, view_set_frame};
use crate::state::slock_cell::{MainSlockCell};
use crate::util::geo::{AlignedFrame, Point, Rect, Size};
use crate::util::rust_util::PhantomUnsendUnsync;
use crate::view::{EnvRef, Invalidator, View};
use crate::view::util::SizeContainer;
use crate::view::view_provider::ViewProvider;

pub(crate) trait InnerViewBase<E> where E: Environment {
    /* native methods */

    // must be called after show was called at least once
    // (otherwise will likely be null)
    fn backing(&self) -> *mut c_void;

    /* tree methods */
    fn superview(&self) -> Option<Arc<MainSlockCell<dyn InnerViewBase<E>>>>;
    fn set_superview(&mut self, superview: Option<Weak<MainSlockCell<dyn InnerViewBase<E>>>>);
    fn graph(&mut self) -> &mut Graph<E>;

    fn depth(&self) -> u32;

    /* layout methods */
    fn needs_layout_up(&self) -> bool;
    fn needs_layout_down(&self) -> bool;

    // true if we need to go to the parent and lay that up
    fn layout_up(&mut self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, env: &mut E, s: MSlock<'_>) -> bool;

    // fails if the current view requires context
    // in such a case, we must go to the parent and retry
    // frame should be the new frame for layout
    // or null if we are to use the last frame
    // this should only be done if we know for sure
    // the last frame is valid
    fn try_layout_down(&mut self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, env: &mut E, frame: Option<AlignedFrame>, s: MSlock<'_>) -> Result<(), ()>;

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
        window: &Weak<MainSlockCell<dyn WindowEnvironmentBase<E>>>,
        e: &mut E,
        depth: u32,
        s: MSlock<'_>
    );

    fn hide(&mut self, s: MSlock<'_>);


    // this is done whenever a node has layout context
    // and thus cannot be layed out trivially so that the
    // parent must have its layout down flag set to true
    // even though it doesn't need a layout up
    fn set_needs_layout_down(&mut self);
    fn set_needs_layout_up(&mut self);
    fn invalidate(&mut self, this: Weak<MainSlockCell<dyn InnerViewBase<E>>>, s: Slock);

    /* environment */

    fn push_environment(&mut self, env: &mut E, s: MSlock);
    fn pop_environment(&mut self, env: &mut E, s: MSlock);
}

// contains a backing and
pub(crate) struct InnerView<E, P> where E: Environment,
                                        P: ViewProvider<E> {
    // parent, subviews, depth, backing, etc
    graph: Graph<E>,

    needs_layout_up: bool,
    needs_layout_down: bool,

    /* cached layout results */
    last_point: Point,
    last_frame: AlignedFrame,
    // rectangle within the last_frame that was used
    last_rect: Rect,

    /* provider */
    provider: P
}

impl<E, P> InnerView<E, P> where E: Environment, P: ViewProvider<E> {
    pub(super) fn into_backing_and_provider(self) -> (NativeView, P) {
        (self.graph.backing, self.provider)
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
    pub(super) fn layout_down_with_context(
        &mut self,
        this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>,
        frame: AlignedFrame,
        at: Point,
        env: &mut E,
        context: &P::DownContext,
        s: MSlock<'_>
    ) -> Rect {
        // all writes to dirty flag are done with a state lock
        // we may set the dirty flag to false now that we are performing a layout
        let mut actually_needs_layout = self.needs_layout_down;
        // if context isn't trivial, there may be updates
        // that were not taken into account
        actually_needs_layout = actually_needs_layout || !self.is_trivial_context();
        // if frame is different from last frame
        // maybe different overall frame
        actually_needs_layout = actually_needs_layout || frame != self.last_frame;
        actually_needs_layout = true;

        let untranslated = if actually_needs_layout {
            self.provider.push_environment(env.variable_env_mut(), s);

            let subtree = Subtree {
                graph: &mut self.graph,
                owner: this,
            };
            let ret = self.provider.layout_down(&subtree, frame, context, &mut EnvRef(env), s);
            self.provider.pop_environment(env.variable_env_mut(), s);

            ret
        }
        else {
            self.last_rect
        };

        self.last_point = at;
        self.last_rect = untranslated;
        self.last_frame = frame;

        self.needs_layout_down = false;

        let translated = untranslated.translate(at);
        view_set_frame(self.backing(), translated, s);

        translated
    }

    pub(super) fn take_backing(
        &mut self,
        this: &'_ Arc<MainSlockCell<dyn InnerViewBase<E>>>,
        source: (NativeView, P),
        env: &mut EnvRef<E>,
        s: MSlock
    ) {
        if !self.backing().is_null() {
            panic!("May not take backing from alt view when this backing has already been inited")
        }
        else if !source.0.0.is_null() {
            // since our backing was not inited
            // we are guaranteed to have had zero children
            // and that we cannot have been shown already
            // therefore, show must be called on this view sometime in the future
            self.provider.push_environment(env.0.variable_env_mut(), s);

            let invalidator = Invalidator {
                view: Arc::downgrade(this)
            };
            let mut subtree = Subtree {
                graph: &mut self.graph,
                owner: this
            };
            self.graph.backing = self.provider.init_backing(
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
    fn backing(&self) -> *mut c_void {
        self.graph.backing.0
    }

    fn superview(&self) -> Option<Arc<MainSlockCell<dyn InnerViewBase<E>>>> {
        self.graph.superview.as_ref().and_then(|s| s.upgrade())
    }

    fn set_superview(&mut self, superview: Option<Weak<MainSlockCell<dyn InnerViewBase<E>>>>) {
        if self.graph.superview.is_some() && superview.is_some() {
            panic!("Attempt to add view to superview when the subview is already mounted to a view. \
                        Please remove the view from the other view before proceeding");
        }

        self.graph.superview = superview;
    }

    fn graph(&mut self) -> &mut Graph<E> {
        &mut self.graph
    }

    fn depth(&self) -> u32 {
        self.graph.depth
    }

    fn needs_layout_up(&self) -> bool {
        self.needs_layout_up
    }

    fn needs_layout_down(&self) -> bool {
        self.needs_layout_down
    }

    fn layout_up(&mut self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, env: &mut E, s: MSlock<'_>) -> bool {
        debug_assert!(self.needs_layout_up);

        let mut handle = EnvRef(env);
        let mut subtree = Subtree {
            graph: &mut self.graph,
            owner: this,
        };
        let ret = self.provider.layout_up(&mut subtree, &mut handle, s);

        self.needs_layout_up = false;
        self.needs_layout_down = true;

        ret
    }

    fn try_layout_down(&mut self, this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>, env: &mut E, frame: Option<AlignedFrame>, s: MSlock<'_>) -> Result<(), ()> {
        let context = ();
        let context_ref: &dyn Any = &context;

        if let Some(r) = context_ref.downcast_ref::<P::DownContext>() {
            self.layout_down_with_context(this, frame.unwrap_or(self.last_frame), self.last_point, env, r, s);

            Ok(())
        }
        else {
            Err(())
        }
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
        debug_assert!(!self.needs_layout_up() && self.depth() != u32::MAX,
                      "This method must only be called after the subview \
                      has been mounted to the parent. Also, this view cannot be in an invalidated/dirty state");
        self.provider.sizes(s)
    }

    fn show(
        &mut self,
        this: &Arc<MainSlockCell<dyn InnerViewBase<E>>>,
        window: &Weak<MainSlockCell<dyn WindowEnvironmentBase<E>>>,
        e: &mut E,
        depth: u32,
        s: MSlock<'_>
    ) {
        /* save attributes */
        let new_window = Some(window.clone());
        if self.graph.window.is_some() && !std::ptr::addr_eq(self.graph.window.as_ref().unwrap().as_ptr(), window.as_ptr()) {
            panic!("Cannot add view to different window than the original one it was mounted on!")
        }

        self.graph.window = new_window;
        self.graph.depth = depth;

        /* push environment */
        self.push_environment(e, s);

        /* init backing if necessary */
        let first_mount = self.backing().is_null();
        if first_mount {
            let invalidator = Invalidator {
                view: Arc::downgrade(this)
            };
            let mut handle = EnvRef(e);
            let mut subtree = Subtree {
                graph: &mut self.graph,
                owner: this,
            };
            self.graph.backing = self.provider.init_backing(invalidator, &mut subtree, None, &mut handle, s);
        }

        // we do NOT ask the window to invalidate this view
        // instead, we only calculate layout_up after children do
        // in all other cases layout_down will generally be called
        // by the parent at some point in the future
        self.needs_layout_up = true;
        self.needs_layout_down = true;

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
                view_add_child_at(self.graph.backing.0, borrow.backing(), i, s);
            }
        }
        self.provider.post_show(s);

        // layout up now that subtree has done layout_up
        // again, in most scenarios parent will call
        // layout_down very soon (though technically not guaranteed)
        self.layout_up(this, e, s);

        /* pop environment */
        self.pop_environment(e, s);
    }

    fn hide(&mut self, s: MSlock<'_>) {
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

    fn set_needs_layout_down(&mut self) {
        self.needs_layout_down = true;
    }

    fn set_needs_layout_up(&mut self) {
        self.needs_layout_up = true;
    }

    fn invalidate(&mut self, this: Weak<MainSlockCell<dyn InnerViewBase<E>>>, s: Slock) {
        if let Some(window) = self.graph.window.as_ref().and_then(|window| window.upgrade()) {
            self.needs_layout_up = true;
            self.needs_layout_down = true;

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

pub struct NativeView(*mut c_void);

impl NativeView {
    pub unsafe fn new(owned_view: *mut c_void) -> NativeView {
        NativeView(owned_view)
    }

    pub fn layout_view(s: MSlock) -> NativeView {
        unsafe {
            NativeView::new(native::view::init_layout_view(s))
        }
    }

    pub fn view(&self) -> *mut c_void {
        self.0
    }
}

impl Drop for NativeView {
    fn drop(&mut self) {
        if !self.0.is_null() {
            assert!(native::global::is_main());
            native::view::free_view(self.0)
        }
    }
}

pub(crate) struct Graph<E> where E: Environment {
    backing: NativeView,

    superview: Option<Weak<MainSlockCell<dyn InnerViewBase<E>>>>,
    window: Option<Weak<MainSlockCell<dyn WindowEnvironmentBase<E>>>>,
    // u32::MAX indicates detached view
    depth: u32,

    subviews: Vec<Arc<MainSlockCell<dyn InnerViewBase<E>>>>,
    unsend_unsync: PhantomUnsendUnsync
}

impl<E> Graph<E> where E: Environment {
    pub(crate) fn clear_subviews(&mut self, s: MSlock) {
        if !self.backing.0.is_null() {
            view_clear_children(self.backing.0, s);
        }

        for subview in std::mem::take(&mut self.subviews) {
            subview.borrow_mut_main(s).hide(s);
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
    pub fn remove_subview_at(&mut self, index: usize, s: MSlock<'_>) {
        // remove from backing
        if !self.graph.backing.0.is_null() {
            view_remove_child(self.graph.backing.0, index, s);
        }

        let removed = self.graph.subviews.remove(index);
        let mut borrow = removed.borrow_mut_main(s);
        borrow.set_superview(None);
        borrow.hide(s);
    }

    pub fn remove_subview<P>(&mut self, subview: &View<E, P>, s: MSlock<'_>) where P: ViewProvider<E> {
        let comp = subview.0.clone() as Arc<MainSlockCell<dyn InnerViewBase<E>>>;
        let index = self.graph.subviews.iter()
            .position(|u| Arc::ptr_eq(u, &comp))
            .expect("Input view should be a child of the current view");

        self.remove_subview_at(index, s);
    }

    pub fn clear_subviews(&mut self, s: MSlock) {
        self.graph.clear_subviews(s)
    }

    // note that cyclic is technically possible if you work hard enough
    // but this will often just result in a stall or other weird effects
    pub fn insert_subview<P>(&mut self, subview: &View<E, P>, index: usize, env: &mut EnvRef<E>, s: MSlock<'_>) where P: ViewProvider<E> {
        subview.0.borrow_mut_main(s).set_superview(Some(Arc::downgrade(self.owner)));
        self.graph.subviews.insert(index, subview.0.clone());

        // 1. we are currently mounted
        if self.graph.depth != u32::MAX {
            let weak = self.graph.window.as_ref().unwrap().clone();
            let subview_this = subview.0.clone() as Arc<MainSlockCell<dyn InnerViewBase<E>>>;
            subview.0.borrow_mut_main(s).show(&subview_this, &weak, env.0, self.graph.depth + 1, s);
        }

        // add to backing
        if !self.graph.backing.0.is_null() {
            view_add_child_at(self.graph.backing.0, subview.0.borrow_main(s).backing(), index, s);
        }
    }

    pub fn push_subview<P>(&mut self, subview: &View<E, P>, env: &mut EnvRef<E>, s: MSlock<'_>) where P: ViewProvider<E> {
        self.insert_subview(subview, self.graph.subviews.len(), env, s);
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
                    backing: NativeView(0 as *mut c_void),
                    subviews: vec![],
                    unsend_unsync: Default::default(),
                },
                // note that upon initial mount
                // this will be set to true
                needs_layout_down: false,
                needs_layout_up: false,
                last_frame: AlignedFrame::default(),
                last_point: Point::default(),
                last_rect: Rect::default(),
                provider,
            }, s)
        )
    }
}