use std::cell::RefCell;
use std::ptr;
use std::rc::{Rc, Weak};
use crate::core::{Environment, MSlock};
use crate::event::{Event, EventResult};
use crate::util::geo::{Rect, Size};
use crate::view::{EnvRef, IntoViewProvider, Invalidator, NativeView, Subtree, ToArcViewBase, View, ViewProvider};
use crate::view::modifers::{ConditionalIVPModifier, ConditionalVPModifier};

struct PortalInner<E, U, D>
    where E: Environment, U: 'static, D: 'static {
    // number active senders (upon layout down, this should be 0 or 1)
    // if sender_count is zero, sent_view should be treated as garbage
    // even if the weak allocation points to something
    sender_count: usize,
    receiver_invalidator: Vec<Invalidator<E>>,
    sent_view: Option<Weak<dyn ToArcViewBase<E, UpContext=U, DownContext=D>>>
}

pub struct Portal<E, U=(), D=()> where E: Environment, U: 'static, D: 'static {
    inner: Rc<RefCell<PortalInner<E, U, D>>>
}

impl<E, U, D> Portal<E, U, D>
    where E: Environment, U: 'static, D: 'static {
    pub fn new() -> Self {
        Portal {
            inner: Rc::new(RefCell::new(PortalInner {
                sender_count: 0,
                receiver_invalidator: vec![],
                sent_view: None
            }))
        }
    }
}
impl<E, U, D> Clone for Portal<E, U, D>
    where E: Environment, U: 'static, D: 'static {
    fn clone(&self) -> Self {
        Portal {
            inner: self.inner.clone()
        }
    }
}

pub struct PortalReceiver<E, U, D> where E: Environment, U: Default + 'static, D: 'static {
    portal: Portal<E, U, D>,
    invalidator: Option<Invalidator<E>>,
}

impl<E, U, D> IntoViewProvider<E> for PortalReceiver<E, U, D>
    where E: Environment, U: Default + 'static, D: 'static
{
    type UpContext = U;
    type DownContext = D;

    fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        self
    }
}

impl<E, U, D> PortalReceiver<E, U, D>
    where E: Environment, U: Default + 'static, D: 'static
{
    pub fn new(portal: &Portal<E, U, D>) -> Self {
        PortalReceiver {
            portal: Portal { inner: portal.inner.clone() },
            invalidator: None,
        }
    }

    #[inline]
    fn mount(&mut self, s: MSlock) {
        let mut borrow = self.portal.inner.borrow_mut();

        // if not currently contained add it
        let our_invalidator = self.invalidator.as_ref().unwrap();
        if !borrow.receiver_invalidator.contains(our_invalidator) {
            borrow.receiver_invalidator.push(our_invalidator.clone());
        }

        // if we are the last, tell others to go away
        if borrow.receiver_invalidator.last() == self.invalidator.as_ref() {
            for i in 0 .. borrow.receiver_invalidator.len() - 1 {
                if let Some(inv) = borrow.receiver_invalidator[i].upgrade() {
                    inv.invalidate(s);
                }
            }
        }
    }

    #[inline]
    fn unmount(&self, s: MSlock) {
        self.portal.inner.borrow_mut()
            .receiver_invalidator
            .retain(|inv| {
                if inv != self.invalidator.as_ref().unwrap() {
                    if let Some(inv) = inv.upgrade() {
                        inv.invalidate(s);
                        return true
                    }
                }

                false
            })

    }

    #[inline]
    fn subview(&self) -> Option<Rc<dyn ToArcViewBase<E, UpContext=U, DownContext=D>>> {
        let borrow = self.portal.inner.borrow();
        if borrow.sender_count == 0 {
            None
        }
        else {
            self.portal.inner.borrow()
                .sent_view
                .as_ref()
                .and_then(|v| v.upgrade())
        }
    }
}

impl<E, U, D> ViewProvider<E> for PortalReceiver<E, U, D>
    where E: Environment, U: Default + 'static, D: 'static
{
    type UpContext = U;
    type DownContext = D;

    fn intrinsic_size(&mut self, s: MSlock) -> Size {
        self.subview()
            .map(|r| r.intrinsic_size(s))
            .unwrap_or(Size::default())
    }

    fn xsquished_size(&mut self, s: MSlock) -> Size {
        self.subview()
            .map(|r| r.xsquished_size(s))
            .unwrap_or(Size::default())
    }

    fn xstretched_size(&mut self, s: MSlock) -> Size {
        self.subview()
            .map(|r| r.xstretched_size(s))
            .unwrap_or(Size::default())
    }

    fn ysquished_size(&mut self, s: MSlock) -> Size {
        self.subview()
            .map(|r| r.ysquished_size(s))
            .unwrap_or(Size::default())
    }

    fn ystretched_size(&mut self, s: MSlock) -> Size {
        self.subview()
            .map(|r| r.ystretched_size(s))
            .unwrap_or(Size::default())
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        self.subview()
            .map(|v| v.up_context(s))
            .unwrap_or_default()
    }

    fn init_backing(&mut self, invalidator: Invalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        self.invalidator = Some(invalidator);

        if let Some((nv, _)) = backing_source {
            nv
        }
        else {
            NativeView::layout_view(s)
        }
    }

    fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
        subtree.clear_subviews(s);

        if let Some(arc) = self.subview().map(|v| v.to_view_base())
        {
            subtree.insert_arc_even_if_mounted_on_another_view(arc, 0, env, s);
        }

        self.mount(s);

        true
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        if self.portal.inner.borrow().receiver_invalidator.len() > 1 {
            panic!("Multiple Receivers active at the same time!")
        }

        if let Some(ref view) = self.subview() {
            let used = view.layout_down_with_context(frame.full_rect(), layout_context, env, s);
            (used, used)
        }
        else {
            (Rect::default(), Rect::default())
        }
    }

    fn pre_hide(&mut self, s: MSlock) {
        self.unmount(s);
    }
}

pub struct PortalSenderIVP<E, U, D, I, W>
    where E: Environment, U: 'static, D: 'static,
          I: IntoViewProvider<E, UpContext=U, DownContext=D>,
          W: IntoViewProvider<E>
{
    portal: Portal<E, U, D>,
    wrapping: W,
    view: I
}

impl<E, U, D, I, W> IntoViewProvider<E> for PortalSenderIVP<E, U, D, I, W>
    where E: Environment, U: 'static, D: 'static,
          I: IntoViewProvider<E, UpContext=U, DownContext=D>,
          W: IntoViewProvider<E>
{
    type UpContext = W::UpContext;
    type DownContext = W::DownContext;

    fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        PortalSenderVP {
            portal: self.portal,
            content: Rc::new(self.view.into_view_provider(env, s).into_view(s)),
            wrapping: self.wrapping.into_view_provider(env, s),
            invalidator: None,
            conditional_enabled: true,
            mounted: false,
        }
    }
}

impl<E, U, D, I, W> ConditionalIVPModifier<E> for PortalSenderIVP<E, U, D, I, W>
    where E: Environment, U: 'static, D: 'static,
          I: IntoViewProvider<E, UpContext=U, DownContext=D>,
          W: ConditionalIVPModifier<E>
{
    type Modifying = W;

    fn into_conditional_view_provider(self, env: &E::Const, s: MSlock) -> impl ConditionalVPModifier<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
        PortalSenderVP {
            portal: self.portal,
            content: Rc::new(self.view.into_view_provider(env, s).into_view(s)),
            wrapping: self.wrapping.into_conditional_view_provider(env, s),
            invalidator: None,
            conditional_enabled: true,
            mounted: false
        }
    }
}

struct PortalSenderVP<E, U, D, P, W>
    where E: Environment, U: 'static, D: 'static,
          P: ViewProvider<E, UpContext=U, DownContext=D>,
          W: ViewProvider<E>
{
    portal: Portal<E, U, D>,
    wrapping: W,
    content: Rc<View<E, P>>,
    conditional_enabled: bool,
    mounted: bool,
    invalidator: Option<Invalidator<E>>
}

impl<E, U, D, P, W> PortalSenderVP<E, U, D, P, W>
    where E: Environment,
          U: 'static,
          D: 'static,
          P: ViewProvider<E, UpContext=U, DownContext=D>,
          W: ViewProvider<E>
{
    fn try_mount(&mut self, s: MSlock) {
        if !self.conditional_enabled {
            return
        }
        // we may want to mount even if mounted is true in case another sender tried to take over

        let mut borrow = self.portal.inner.borrow_mut();

        let needs_change = {
            borrow.sent_view.is_none() ||
                !ptr::addr_eq(Weak::as_ptr(borrow.sent_view.as_ref().unwrap()), Rc::as_ptr(&self.content))
        };

        if needs_change {
            borrow.sent_view = Some(Rc::downgrade(&self.content) as Weak<dyn ToArcViewBase<E, UpContext=U, DownContext=D>>);
            if let Some(ref inv) = borrow.receiver_invalidator.last().and_then(|x| x.upgrade()) {
                inv.invalidate(s.as_general_slock());
            }
        }

        if !self.mounted {
            borrow.sender_count += 1;
            self.mounted = true;
        }
    }

    fn unmount(&mut self, s: MSlock) {
        if !self.mounted {
            return;
        }

        let mut borrow = self.portal.inner.borrow_mut();

        if borrow.sent_view.is_some() {
            // borrow.sent_view = None;
            // so we don't actually set sent_view to None
            // because on transition points, the incoming portal may be mounted
            // right before we unmount, and in such a case it would be unwanted
            // for us to unmount, since then we are unmounting the other's sent_view
            // the receiver then relies on the sender count to determine whether
            // or not there is a view
            if let Some(ref inv) = borrow.receiver_invalidator.last().and_then(|x| x.upgrade()) {
                inv.invalidate(s.as_general_slock());
            }
        }

        borrow.sender_count -= 1;
        self.mounted = false;
    }
}

impl<E, U, D, P, W> ViewProvider<E> for PortalSenderVP<E, U, D, P, W>
    where E: Environment,
          U: 'static,
          D: 'static,
          P: ViewProvider<E, UpContext=U, DownContext=D>,
          W: ViewProvider<E>
{
    type UpContext = W::UpContext;
    type DownContext = W::DownContext;

    fn intrinsic_size(&mut self, s: MSlock) -> Size {
        self.wrapping.intrinsic_size(s)
    }

    fn xsquished_size(&mut self, s: MSlock) -> Size {
        self.wrapping.xsquished_size(s)
    }

    fn xstretched_size(&mut self, s: MSlock) -> Size {
        self.wrapping.xstretched_size(s)
    }

    fn ysquished_size(&mut self, s: MSlock) -> Size {
        self.wrapping.ysquished_size(s)
    }

    fn ystretched_size(&mut self, s: MSlock) -> Size {
        self.wrapping.ysquished_size(s)
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        self.wrapping.up_context(s)
    }

    fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        self.invalidator = Some(invalidator.clone());

        let ret = if let Some((nv, src)) = backing_source {
            if let Some(this) = Rc::into_inner(src.content) {
                self.content.take_backing(this, env, s);
            }

            self.wrapping.init_backing(invalidator, subtree, Some((nv, src.wrapping)), env, s)
        }
        else {
            self.wrapping.init_backing(invalidator, subtree, None, env, s)
        };
        assert_eq!(subtree.len(), 0, "Portal Sender must be attached to a view with zero children!");
        ret
    }

    fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
        if self.conditional_enabled {
            self.try_mount(s);
        }

        let ret = self.wrapping.layout_up(subtree, env, s);
        assert_eq!(subtree.len(), 0, "Portal Sender must be attached to a view with zero children!");
        ret
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        if self.conditional_enabled {
            if self.portal.inner.borrow().sender_count != 1 {
                panic!("Multiple portal senders active at the same time!");
            }
        }

        self.wrapping.layout_down(subtree, frame, layout_context, env, s)
    }

    fn pre_show(&mut self, s: MSlock) {
        self.wrapping.pre_show(s)
    }

    fn post_show(&mut self, s: MSlock) {
        self.wrapping.post_show(s)
    }

    fn pre_hide(&mut self, s: MSlock) {
        self.unmount(s);
        self.wrapping.pre_hide(s)
    }

    fn post_hide(&mut self, s: MSlock) {
        self.wrapping.post_hide(s)
    }

    fn focused(&mut self, rel_depth: u32, s: MSlock) {
        self.wrapping.focused(rel_depth, s);
    }

    fn unfocused(&mut self, rel_depth: u32, s: MSlock) {
        self.wrapping.unfocused(rel_depth, s);
    }

    fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        self.wrapping.push_environment(env, s);
    }

    fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        self.wrapping.pop_environment(env, s);
    }

    fn handle_event(&mut self, e: &Event, s: MSlock) -> EventResult {
        self.wrapping.handle_event(e, s)
    }
}

impl<E, U, D, P, W> ConditionalVPModifier<E> for PortalSenderVP<E, U, D, P, W>
    where E: Environment,
          U: 'static,
          D: 'static,
          P: ViewProvider<E, UpContext=U, DownContext=D>,
          W: ConditionalVPModifier<E>
{
    fn enable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
        self.conditional_enabled = true;
        self.try_mount(s);
        self.wrapping.enable(subtree, env, s);
    }

    fn disable(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) {
        self.unmount(s);
        self.wrapping.disable(subtree, env, s);
        self.conditional_enabled = false;
    }
}

pub trait PortalSendable<E>: IntoViewProvider<E> where E: Environment {
    fn portal_sender<V: IntoViewProvider<E>>(self, portal: &Portal<E, V::UpContext, V::DownContext>, view: V) -> PortalSenderIVP<E, V::UpContext, V::DownContext, V, Self>;
}

impl<E, I> PortalSendable<E> for I where E: Environment, I: IntoViewProvider<E> {
    fn portal_sender<V: IntoViewProvider<E>>(self, portal: &Portal<E, V::UpContext, V::DownContext>, view: V) -> PortalSenderIVP<E, V::UpContext, V::DownContext, V, Self>
    {
        PortalSenderIVP {
            portal: Portal { inner: portal.inner.clone() },
            wrapping: self,
            view,
        }
    }
}