use std::ffi::c_void;
use std::mem::{MaybeUninit, transmute};
use std::sync::{Arc, Weak};
use crate::core::{Environment, MSlock, Slock, WindowEnvironmentBase};
use crate::native;
use crate::native::view::{view_add_child_at, view_clear_children, view_remove_child, view_set_frame};
use crate::state::slock_cell::SlockCell;
use crate::util::geo::{AlignedFrame, Point, Rect};
use crate::util::rust_util::PhantomUnsendUnsync;
use crate::view::{Handle, Invalidator, View, ViewProvider};

pub(crate) trait InnerViewBase<E> where E: Environment {
    /* native methods */

    // must be called after show was called at least once
    // (otherwise will likely be null)
    fn backing(&self) -> *mut c_void;

    /* tree methods */

    fn window(&self) -> Option<Weak<SlockCell<dyn WindowEnvironmentBase<E>>>>;
    fn superview(&self) -> Option<Arc<SlockCell<dyn InnerViewBase<E>>>>;
    fn set_superview(&mut self, superview: Weak<SlockCell<dyn InnerViewBase<E>>>);
    fn subviews(&self) -> &Subviews<E>;

    fn depth(&self) -> u32;

    /* layout methods */
    fn needs_layout_up(&self) -> bool;
    fn needs_layout_down(&self) -> bool;

    // true if we need to go to the parent and lay that up
    fn layout_up(&mut self, env: &mut E, s: MSlock<'_>) -> bool;

    // fails if the current view requires context
    // in such a case, we must go to the parent and retry
    // this method should only be called if we know for sure
    // the last frame is valid
    fn try_layout_down(&mut self, env: &mut E, s: MSlock<'_>) -> Result<(), ()>;

    /* mounting and unmounting */

    fn show(
        &mut self,
        this: Weak<SlockCell<dyn InnerViewBase<E>>>,
        window: &Weak<SlockCell<dyn WindowEnvironmentBase<E>>>,
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
    fn invalidate(&mut self, this: Weak<SlockCell<dyn InnerViewBase<E>>>, s: Slock);

    /* environment */

    fn push_environment(&self, env: &mut E, s: MSlock);
    fn pop_environment(&self, env: &mut E, s: MSlock);
}

// contains a backing and
pub(crate) struct InnerView<E, P> where E: Environment, P: ViewProvider<E> {
    /* tree */
    window: Option<Weak<SlockCell<dyn WindowEnvironmentBase<E>>>>,
    superview: Option<Weak<SlockCell<dyn InnerViewBase<E>>>>,
    depth: u32,
    /* also contains backing */
    subviews: Subviews<E>,

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
    #[inline]
    pub(super) fn is_trivial_context(&self) -> bool {
        std::any::TypeId::of::<P::LayoutContext>() == std::any::TypeId::of::<()>()
    }

    pub(super) fn provider(&self) -> &P {
        &self.provider
    }

    // returns position of rect
    // in superview coordinate system
    pub(super) fn layout_down_with_context(
        &mut self,
        frame: AlignedFrame,
        at: Point,
        env: &mut E,
        context: &P::LayoutContext,
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

        self.needs_layout_down = false;

        let untranslated = if actually_needs_layout {
            self.provider.push_environment(env, s);
            let ret = self.provider.layout_down(frame, context, &mut Handle(env), s);
            self.provider.pop_environment(env, s);

            ret
        }
        else {
            self.last_rect
        };


        self.last_point = at;
        self.last_rect = untranslated;
        self.last_frame = frame;

        let translated = untranslated.translate(at);
        view_set_frame(self.backing(), translated, s);

        translated
    }

    pub(super) fn replace_provider(&mut self, this: &Arc<SlockCell<Self>>, provider: P, env: &mut Handle<E>, s: MSlock) {
        // if no backing, can do a trivial swap
        if self.backing().is_null() {
            self.provider = provider;
        }
        else {
            self.provider = provider;
            self.subviews.backing = 0 as *mut c_void;
            self.subviews.clear(s);

            if self.superview.is_some() {
                // we are currently mounted
                // so we may init the backing directly
                // this operation is in fact isomorphic to a show
                let weak = Arc::downgrade(this) as Weak<SlockCell<dyn InnerViewBase<E>>>;
                let window = self.window.clone().unwrap();
                self.show(weak, &window, env.0, self.depth, s);
            }
            // otherwise:
            // not currently mounted
            // all we have to dset the provider and reset the backing
            // and upon the next mounting the magic will be done
            // (i dont think this path can actually be called since it requires env?)
        }
    }
}

impl<E, P> InnerViewBase<E> for InnerView<E, P> where E: Environment, P: ViewProvider<E> {
    fn backing(&self) -> *mut c_void {
        self.subviews.backing
    }

    fn window(&self) -> Option<Weak<SlockCell<dyn WindowEnvironmentBase<E>>>> {
        self.window.clone()
    }

    fn superview(&self) -> Option<Arc<SlockCell<dyn InnerViewBase<E>>>> {
        self.superview.as_ref().and_then(|s| s.upgrade())
    }

    fn set_superview(&mut self, superview: Weak<SlockCell<dyn InnerViewBase<E>>>) {
        if self.superview.is_some() {
            panic!("Attempt to add view to superview when the subview is already mounted to a view. \
                        Please remove the view from the other view before proceeding");
        }

        self.superview = Some(superview);
    }

    fn subviews(&self) -> &Subviews<E> {
        &self.subviews
    }

    fn depth(&self) -> u32 {
        self.depth
    }

    fn needs_layout_up(&self) -> bool {
        self.needs_layout_up
    }

    fn needs_layout_down(&self) -> bool {
        self.needs_layout_down
    }

    fn layout_up(&mut self, env: &mut E, s: MSlock<'_>) -> bool {
        self.needs_layout_up = false;

        let mut handle = Handle(env);
        self.provider.layout_up(&mut self.subviews, &mut handle, s)
    }

    fn try_layout_down(&mut self, env: &mut E, s: MSlock<'_>) -> Result<(), ()> {
        // with optimizations, this has been tested to inline
        if self.is_trivial_context() {
            // safety: checked that P::LayoutContext == ()
            let layout_context = unsafe {
                std::mem::transmute_copy::<(), P::LayoutContext>(&())
            };

            self.layout_down_with_context(self.last_frame, self.last_point, env, &layout_context, s);

            Ok(())
        }
        else {
            Err(())
        }
    }

    fn show(
        &mut self,
        this: Weak<SlockCell<dyn InnerViewBase<E>>>,
        window: &Weak<SlockCell<dyn WindowEnvironmentBase<E>>>,
        e: &mut E,
        depth: u32,
        s: MSlock<'_>
    ) {
        /* save attributes */
        let new_window = Some(window.clone());
        if self.window.is_some() && !std::ptr::addr_eq(self.window.as_ref().unwrap().as_ptr(), window.as_ptr()) {
            panic!("Cannot add view to different window than the original one it was mounted on!")
        }

        self.window = new_window;
        self.depth = depth;

        /* push environment */
        self.push_environment(e, s);

        /* init backing if necessary */
        let first_mount = self.backing().is_null();
        if first_mount {
            let invalidator = Invalidator {
                view: this.clone()
            };
            let mut handle = Handle(e);
            self.subviews.backing = self.provider.init_backing(invalidator, &mut self.subviews, None, None, &mut handle, s);
        }

        /* invalidate this view */
        self.invalidate(this, s.as_general_slock());

        /* main notifications to provider and subtree */
        self.provider.pre_show(s);
        for (i, subview) in self.subviews.subviews.iter().enumerate() {
            let mut borrow = subview.borrow_mut_main(s);
            borrow.show(
                Arc::downgrade(subview),
                window,
                e,
                depth + 1,
                s
            );

            /* add subview if this first time backing allocated */
            if first_mount {
                view_add_child_at(self.subviews.backing, borrow.backing(), i, s);
            }
        }
        self.provider.post_show(s);

        /* pop environment */
        self.pop_environment(e, s);
    }

    fn hide(&mut self, s: MSlock<'_>) {
        // keep window, just remove superview
        self.superview = None;

        self.provider.pre_hide(s);
        for subview in &self.subviews.subviews {
            subview.borrow_mut_main(s).hide(s);
        }
        self.provider.post_hide(s);
    }

    fn set_needs_layout_down(&mut self) {
        self.needs_layout_down = true;
    }

    fn invalidate(&mut self, this: Weak<SlockCell<dyn InnerViewBase<E>>>, s: Slock) {
        if let Some(window) = self.window.as_ref().and_then(|window| window.upgrade()) {
            self.needs_layout_up = true;
            self.needs_layout_down = true;

            // safety:
            // the only part of window we're touching
            // is send (guaranteed by protocol)
            unsafe {
                window.borrow_non_main_non_send(s)
                    .invalidate_view(Arc::downgrade(&window), this, self.depth, s);
            }
        }
    }

    fn push_environment(&self, env: &mut E, s: MSlock) {
        self.provider.push_environment(env, s);
    }

    fn pop_environment(&self, env: &mut E, s: MSlock) {
        self.provider.pop_environment(env, s);
    }
}

// TODO at some point separate this out as the component stored by the view
// and the reference that needs to be sent to the different layout methods
// it would be more natural for this to just be a
// backreference to innerviewbase,
// however then it would need to have to be parameterized
// by P, which would make some provider methods weird
// We'll see better designs in the future
// but this suffices for now
pub struct Subviews<E> {
    owner: Weak<SlockCell<dyn InnerViewBase<E>>>,
    backing: *mut c_void,
    subviews: Vec<Arc<SlockCell<dyn InnerViewBase<E>>>>,
    unsend_unsync: PhantomUnsendUnsync
}

impl<E> Subviews<E> where E: Environment {
    pub(super) fn subviews(&self) -> &Vec<Arc<SlockCell<dyn InnerViewBase<E>>>> {
        &self.subviews
    }

    pub fn remove_at(&mut self, index: usize, s: MSlock<'_>) {
        // remove from backing
        if !self.backing.is_null() {
            view_remove_child(self.backing, index, s);
        }

        let removed = self.subviews.remove(index);
        removed.borrow_mut_main(s).hide(s);
    }

    pub fn remove<P>(&mut self, subview: &View<E, P>, s: MSlock<'_>) where P: ViewProvider<E> {
        let comp = subview.0.clone() as Arc<SlockCell<dyn InnerViewBase<E>>>;
        let index = self.subviews.iter()
            .position(|u| Arc::ptr_eq(u, &comp))
            .expect("Input view should be a child of the current view");

        self.remove_at(index, s);
    }

    pub fn clear(&mut self, s: MSlock) {
        if !self.backing.is_null() {
            view_clear_children(self.backing, s);
        }

        for subview in std::mem::take(&mut self.subviews) {
            subview.borrow_mut_main(s).hide(s);
        }
    }

    pub fn insert<P>(&mut self, subview: &View<E, P>, index: usize, env: &mut Handle<E>, s: MSlock<'_>) where P: ViewProvider<E> {
        subview.0.borrow_mut_main(s).set_superview(self.owner.clone());
        self.subviews.insert(index, subview.0.clone());

        // if currently mounted, have subtree show called
        if !self.backing.is_null() {
            let this = self.owner.upgrade().unwrap();
            let borrow = this.borrow_main(s);
            let window = borrow.window();

            if let Some(window) = window.and_then(|window| window.upgrade()) {
                let weak = Arc::downgrade(&window);
                let depth = borrow.depth();

                let subview_this = Arc::downgrade(&subview.0) as
                    Weak<SlockCell<dyn InnerViewBase<E>>>;
                subview.0.borrow_mut_main(s).show(subview_this, &weak, env.0, depth + 1, s);
            }
        }

        // add to backing
        if !self.backing.is_null() {
            view_add_child_at(self.backing, subview.0.borrow_main(s).backing(), index, s);
        }
    }

    pub fn push<P>(&mut self, subview: &View<E, P>, env: &mut Handle<E>, s: MSlock<'_>) where P: ViewProvider<E> {
        self.insert(subview, self.subviews.len(), env, s);
    }
}

impl<E, P> InnerView<E, P> where E: Environment, P: ViewProvider<E> {
    pub(super) fn new(provider: P, s: MSlock) -> Arc<SlockCell<Self>> {
        // TODO see if theres way to do this without unsafe
        let org = Arc::new(SlockCell::new_main(MaybeUninit::uninit(), s));
        let weak_transmute = unsafe {
            // safety: data layout of maybe uninit and
            // Self are the same. Arc only contains a reference
            // so the daya layouts remain the same
            // in particular, Arc does not directly contain T in the layout
            let init: Arc<SlockCell<InnerView<E, P>>> = transmute(org.clone());
            Arc::downgrade(&init) as Weak<SlockCell<dyn InnerViewBase<E>>>
        };

        *org.borrow_mut_main(s) = MaybeUninit::new(InnerView {
            window: None,
            superview: None,
            depth: 0,
            subviews: Subviews {
                owner: weak_transmute,
                backing: 0 as *mut c_void,
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
        });

        unsafe {
            // once again data layouts are the same
            transmute(org)
        }
    }
}

impl<E, P> Drop for InnerView<E, P> where E: Environment, P: ViewProvider<E> {
    fn drop(&mut self) {
        if self.backing() as usize != 0 {
            native::view::free_view(self.backing());
        }
    }
}