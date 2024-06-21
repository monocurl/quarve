use std::marker::PhantomData;
use crate::core::{Environment, MSlock};
use crate::event::{Event, EventResult};
use crate::state::slock_cell::MainSlockCell;
use crate::util::geo::{Rect, Size};
use crate::view::{EnvRef, InnerView, Invalidator, NativeView, Subtree, View};
use crate::view::util::SizeContainer;

pub trait ViewProvider<E>: Sized + 'static
    where E: Environment
{
    type UpContext: 'static;

    /// Additional context to be used when performing layouts
    /// Typically, this is set to ()
    /// This may be useful when information from parent views
    /// must be sent down to child (or grandchild) views
    type DownContext: 'static;

    fn into_view(self, s: MSlock) -> View<E, Self>
    {
        View(InnerView::new(self, s))
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

    fn intrinsic_size(&mut self, s: MSlock) -> Size;
    fn xsquished_size(&mut self, s: MSlock) -> Size;
    fn xstretched_size(&mut self, s: MSlock) -> Size;
    fn ysquished_size(&mut self, s: MSlock) -> Size;
    fn ystretched_size(&mut self, s: MSlock) -> Size;

    fn up_context(&mut self, s: MSlock) -> Self::UpContext;

    /// Allocate a backing and perform other initialization steps.
    /// This method will only be called once for a given view provider.
    ///
    /// * `backing_source` - The old backing that this view will be replacing.
    /// This will be None if we are not replacing an old view (i.e. a fresh allocation).
    /// It is guaranteed that the backing provided will be allocated from a view of the same type,
    /// specifically that which was provided the `replaced_provider`.
    /// The high level idea is that by providing the old backing,
    /// allocations may be avoided in a manner very similar to a recycler view.
    /// * `replaced_provider` - The provider that this view is replacing. None if if we are doing a
    /// fresh allocation and not replacing an old view.
    fn init_backing(
        &mut self,
        invalidator: Invalidator<E>,
        subtree: &mut Subtree<E>,
        backing_source: Option<(NativeView, Self)>,
        env: &mut EnvRef<E>,
        s: MSlock
    ) -> NativeView;

    /// The children have properly calculated their
    /// minimum, intrinsic, and maximum sizes
    /// We must now calculate ours
    /// If any changes to the bounds happened,
    /// this method should return true to indicate that
    /// the parent must recalculate as well
    /// This method is always called before layout down
    /// and is generally the place to relay state changes to backings
    fn layout_up(
        &mut self,
        subtree: &mut Subtree<E>,
        env: &mut EnvRef<E>,
        s: MSlock
    ) -> bool;

    /// The children have properly calculated their
    /// minimum, intrinsic, and maximum sizes
    /// (and so have we)
    /// Now, we must position them according to the given frame
    /// Return value (our_frame, total_exclusion)
    fn layout_down(
        &mut self,
        subtree: &Subtree<E>,
        frame: Size,
        layout_context: &Self::DownContext,
        env: &mut EnvRef<E>,
        s: MSlock
    ) -> (Rect, Rect);

    // callback methods
    #[allow(unused_variables)]
    fn pre_show(&mut self, s: MSlock) {

    }

    #[allow(unused_variables)]
    fn post_show(&mut self, s: MSlock) {

    }

    #[allow(unused_variables)]
    fn pre_hide(&mut self, s: MSlock) {

    }

    #[allow(unused_variables)]
    fn post_hide(&mut self, s: MSlock) {

    }

    // focus and unfocused state...
    #[allow(unused_variables)]
    fn focused(&mut self, s: MSlock) {

    }

    #[allow(unused_variables)]
    fn unfocused(&mut self, s: MSlock) {

    }

    #[allow(unused_variables)]
    fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {

    }

    #[allow(unused_variables)]
    fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {

    }

    #[allow(unused_variables)]
    fn handle_event(&mut self, e: Event, s: MSlock) -> EventResult {
        EventResult::NotHandled(e)
    }
}

pub struct UpContextAdapter<E, P, U>(P, PhantomData<MainSlockCell<(U, E)>>)
    where E: Environment,
          P: ViewProvider<E>,
          U: 'static,
          P::UpContext: Into<U>;
impl<E, P, U> UpContextAdapter<E, P, U>
    where E: Environment,
          P: ViewProvider<E>,
          U: 'static,
          P::UpContext: Into<U> {
    pub fn new(p: P) -> Self {
        UpContextAdapter(p, PhantomData)
    }
}

impl<E, P, U> ViewProvider<E> for UpContextAdapter<E, P, U>
    where E: Environment,
          P: ViewProvider<E>,
          U: 'static,
          P::UpContext: Into<U> {
    type UpContext = U;
    type DownContext = P::DownContext;

    fn intrinsic_size(&mut self, s: MSlock) -> Size {
        self.0.intrinsic_size(s)
    }

    fn xsquished_size(&mut self, s: MSlock) -> Size {
        self.0.xsquished_size(s)
    }

    fn xstretched_size(&mut self, s: MSlock) -> Size {
        self.0.xstretched_size(s)
    }

    fn ysquished_size(&mut self, s: MSlock) -> Size {
        self.0.ysquished_size(s)
    }

    fn ystretched_size(&mut self, s: MSlock) -> Size {
        self.0.ystretched_size(s)
    }

    fn up_context(&mut self, s: MSlock) -> Self::UpContext {
        self.0.up_context(s).into()
    }

    fn init_backing(&mut self, invalidator: Invalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
        let source = backing_source
            .map(|(n, p)| (n, p.0));

        self.0.init_backing(invalidator, subtree, source, env, s)
    }

    fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
        self.0.layout_up(subtree, env, s)
    }

    fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
        self.0.layout_down(subtree, frame, layout_context, env, s)
    }

    fn pre_show(&mut self, s: MSlock) {
        self.0
            .pre_show(s)
    }

    fn post_show(&mut self, s: MSlock) {
        self.0
            .post_show(s)
    }

    fn pre_hide(&mut self, s: MSlock) {
        self.0
            .pre_hide(s)
    }

    fn post_hide(&mut self, s: MSlock) {
        self.0
            .post_hide(s)
    }

    fn focused(&mut self, s: MSlock) {
        self.0
            .focused(s)
    }

    fn unfocused(&mut self, s: MSlock) {
        self.0
            .unfocused(s)
    }

    fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        self.0
            .push_environment(env, s);
    }

    fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
        self.0
            .pop_environment(env, s);
    }

    fn handle_event(&mut self, e: Event, s: MSlock) -> EventResult {
        self.0
            .handle_event(e, s)
    }
}

// when need to return None for Option<impl ViewProvider> and need concrete type
pub(crate) struct DummyProvider<E, U, D>(pub PhantomData<(E, U, D)>);
impl<E, U, D> ViewProvider<E> for DummyProvider<E, U, D>
    where E: Environment, U: 'static, D: 'static
{
    type UpContext = U;
    type DownContext = D;

    fn intrinsic_size(&mut self, _s: MSlock) -> Size {
        unreachable!()
    }

    fn xsquished_size(&mut self, _s: MSlock) -> Size {
        unreachable!()
    }

    fn xstretched_size(&mut self, _s: MSlock) -> Size {
        unreachable!()
    }

    fn ysquished_size(&mut self, _s: MSlock) -> Size {
        unreachable!()
    }

    fn ystretched_size(&mut self, _s: MSlock) -> Size {
        unreachable!()
    }

    fn up_context(&mut self, _s: MSlock) -> U {
        unreachable!()
    }

    fn init_backing(&mut self, _invalidator: Invalidator<E>, _subtree: &mut Subtree<E>, _backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, _s: MSlock<'_>) -> NativeView {
        unreachable!()
    }

    fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock<'_>) -> bool {
        unreachable!()
    }

    fn layout_down(&mut self, _subtree: &Subtree<E>, _frame: Size, _layout_context: &D, _env: &mut EnvRef<E>, _s: MSlock<'_>) -> (Rect, Rect) {
        unreachable!()
    }
}