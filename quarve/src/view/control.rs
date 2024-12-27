pub use button::*;
pub use dropdown::*;

mod button {
    use std::ffi::c_void;
    use std::marker::PhantomData;

    use crate::core::{Environment, MSlock, StandardVarEnv};
    use crate::event::{Event, EventPayload, EventResult, MouseEvent};
    use crate::native::view::button::{init_button_view, update_button_view};
    use crate::state::{Binding, SetAction, Signal, Store};
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, View, ViewProvider, ViewRef, WeakInvalidator};
    use crate::view::text::Text;

    pub struct Button {
        phantom_data: PhantomData<()>
    }

    impl Button {
        pub fn new<E>(text: impl Into<String>, action: impl Fn(MSlock) + 'static)
                      -> impl IntoViewProvider<E, UpContext=(), DownContext=()>
            where E: Environment, E::Variable: AsRef<StandardVarEnv>
        {
            Self::new_with_label(
                Text::new(text),
                action
            )
        }

        pub fn new_with_label<E, I>(view: I, action: impl Fn(MSlock) + 'static)
                                    -> impl IntoViewProvider<E, UpContext=I::UpContext, DownContext=I::DownContext>
            where E: Environment, I: IntoViewProvider<E>
        {
            ButtonIVP {
                action,
                label: DefaultButtonLabel {
                    source: view,
                    phantom: Default::default(),
                },
                phantom: Default::default(),
            }
        }
    }


    pub struct ButtonIVP<E, A, L> where E: Environment, A: Fn(MSlock) + 'static, L: ButtonLabel<E> {
        action: A,
        label: L,
        phantom: PhantomData<E>
    }

    impl<E, A, L> IntoViewProvider<E> for ButtonIVP<E, A, L>
        where E: Environment,
              A: Fn(MSlock) + 'static,
              L: ButtonLabel<E>
    {
        type UpContext = L::UpContext;
        type DownContext = L::DownContext;

        fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            self.label
                .into_button_view_provider(self.action, env, s)
        }
    }

    struct ButtonVP<E, A, P>
        where E: Environment,
              A: Fn(MSlock) + 'static,
              P: ViewProvider<E>
    {
        is_hover: Store<bool>,
        is_click: Store<bool>,
        source: P,
        action: A,
        last_size: Size,
        phantom: PhantomData<E>
    }

    impl<E, A, P> ViewProvider<E> for ButtonVP<E, A, P>
        where E: Environment,
              A: Fn(MSlock) + 'static,
              P: ViewProvider<E>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.source.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.source.xsquished_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.source.xstretched_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.source.ysquished_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.source.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.source.up_context(s)
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            if let Some((nv, source)) = backing_source {
                self.source.init_backing(invalidator, subtree, Some((nv, source.source)), env, s)
            } else {
                self.source.init_backing(invalidator, subtree, None, env, s)
            }
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            self.source.layout_up(subtree, env, s)
        }

        fn layout_down(&mut self, subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let (ours, used) = self.source.layout_down(subtree, frame, layout_context, env, s);
            self.last_size = ours.size();
            (ours, used)
        }

        fn pre_show(&mut self, s: MSlock) {
            self.source.pre_show(s)
        }

        fn post_show(&mut self, s: MSlock) {
            self.source.post_show(s)
        }

        fn pre_hide(&mut self, s: MSlock) {
            self.source.pre_hide(s)
        }

        fn post_hide(&mut self, s: MSlock) {
            self.source.post_hide(s)
        }

        fn focused(&self, rel_depth: u32, s: MSlock) {
            self.source.focused(rel_depth, s)
        }

        fn unfocused(&self, rel_depth: u32, s: MSlock) {
            self.source.unfocused(rel_depth, s)
        }

        fn push_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.push_environment(env, s)
        }

        fn pop_environment(&mut self, env: &mut E::Variable, s: MSlock) {
            self.source.pop_environment(env, s)
        }

        fn handle_event(&self, e: &Event, s: MSlock) -> EventResult {
            if !e.is_mouse() {
                return self.source.handle_event(e, s);
            }

            let cursor = e.cursor();
            let inside = cursor.x >= 0.0 && cursor.x < self.last_size.w &&
                cursor.y >= 0.0 && cursor.y < self.last_size.h;
            if inside != *self.is_hover.borrow(s) {
                self.is_hover.apply(SetAction::Set(inside), s);
            }

            if !inside || matches!(&e.payload, EventPayload::Mouse(MouseEvent::LeftUp, _)) {
                if *self.is_click.borrow(s) {
                    self.is_click.apply(SetAction::Set(false), s);
                    return EventResult::Handled;
                }
            } else if inside && matches!(&e.payload, EventPayload::Mouse(MouseEvent::LeftDown, _)) {
                if !*self.is_click.borrow(s) {
                    (self.action)(s);

                    self.is_click.apply(SetAction::Set(true), s);
                    return EventResult::Handled;
                }
            }

            self.source.handle_event(e, s)
        }
    }

    pub trait ButtonLabel<E>: 'static + Sized where E: Environment {
        type UpContext: 'static;
        type DownContext: 'static;

        fn into_button_view_provider(self, action: impl Fn(MSlock) + 'static, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>
        {
            let is_hover = Store::new(false);
            let is_click = Store::new(false);
            let source = self.styled(is_hover.signal(), is_click.signal(), env, s);

            ButtonVP {
                is_hover,
                is_click,
                source,
                action,
                last_size: Default::default(),
                phantom: Default::default(),
            }
        }

        fn styled(self, is_hover: impl Signal<Target=bool> + Clone, is_click: impl Signal<Target=bool> + Clone, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;
    }

    struct DefaultButtonLabel<E, I> where E: Environment, I: IntoViewProvider<E> {
        source: I,
        phantom: PhantomData<E>
    }

    impl<E, I> ButtonLabel<E> for DefaultButtonLabel<E, I> where E: Environment, I: IntoViewProvider<E>
    {
        type UpContext = I::UpContext;
        type DownContext = I::DownContext;

        fn styled(self, _is_hover: impl Signal<Target=bool> + Clone, is_click: impl Signal<Target=bool> + Clone, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            DefaultButtonVP {
                child: self.source.into_view_provider(env, s).into_view(s),
                clicked: is_click,
                backing: 0 as *mut c_void,
                phantom: Default::default(),
            }
        }
    }

    struct DefaultButtonVP<E, P, S> where E: Environment, P: ViewProvider<E>, S: Signal<Target=bool> {
        child: View<E, P>,
        clicked: S,
        backing: *mut c_void,
        phantom: PhantomData<E>
    }

    impl<E, P, S> ViewProvider<E> for DefaultButtonVP<E, P, S> where E: Environment, P: ViewProvider<E>, S: Signal<Target=bool>
    {
        type UpContext = P::UpContext;
        type DownContext = P::DownContext;

        fn intrinsic_size(&mut self, s: MSlock) -> Size {
            self.child.intrinsic_size(s)
        }

        fn xsquished_size(&mut self, s: MSlock) -> Size {
            self.child.xsquished_size(s)
        }

        fn xstretched_size(&mut self, s: MSlock) -> Size {
            self.child.xstretched_size(s)
        }

        fn ysquished_size(&mut self, s: MSlock) -> Size {
            self.child.ysquished_size(s)
        }

        fn ystretched_size(&mut self, s: MSlock) -> Size {
            self.child.ystretched_size(s)
        }

        fn up_context(&mut self, s: MSlock) -> Self::UpContext {
            self.child.up_context(s)
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.clicked.listen(move |_, s| {
                invalidator.try_upgrade_invalidate(s)
            }, s);

            let nv = if let Some((nv, source)) = backing_source {
                self.child.take_backing(source.child, env, s);
                nv
            }
            else {
                unsafe {
                    NativeView::new(init_button_view(s), s)
                }
            };
            self.backing = nv.backing();
            subtree.push_subview(&self.child, env, s);

            nv
        }

        fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock) -> bool {
            update_button_view(self.backing, *self.clicked.borrow(s), s);
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let used = self.child.layout_down_with_context(frame.full_rect(), layout_context, env, s);
            (used, used)
        }
    }
}

mod dropdown {
    use std::ffi::c_void;

    use crate::core::{Environment, MSlock};
    use crate::native::view::dropdown::{dropdown_clear, dropdown_push, dropdown_select, dropdown_size, init_dropdown};
    use crate::state::{Binding, Filterless};
    use crate::util::geo;
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};

    pub struct Dropdown<B> where B: Binding<Filterless<Option<String>>> + Clone {
        current: B,
        options: Vec<String>
    }

    impl<B> Dropdown<B> where B: Binding<Filterless<Option<String>>> + Clone {
        pub fn new(binding: B) -> Self {
            Dropdown {
                current: binding,
                options: Vec::new()
            }
        }

        pub fn new_with_options(binding: B, options: Vec<String>) -> Self {
            Dropdown {
                current: binding,
                options
            }
        }

        pub fn option(mut self, str: impl Into<String>) -> Self {
            self.options.push(str.into());
            self
        }
    }

    impl<E, B> IntoViewProvider<E> for Dropdown<B>
        where E: Environment,
              B: Binding<Filterless<Option<String>>> + Clone
    {
        type UpContext = ();
        type DownContext = ();

        fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            DropdownVP {
                current: self.current,
                options: self.options,
                backing: 0 as *mut c_void,
                intrinsic: Size::default(),
            }
        }
    }

    struct DropdownVP<B> where B: Binding<Filterless<Option<String>>> {
        current: B,
        options: Vec<String>,
        backing: *mut c_void,
        intrinsic: Size,
    }

    impl<E, B> ViewProvider<E> for DropdownVP<B>
        where E: Environment,
              B: Binding<Filterless<Option<String>>> + Clone
    {
        type UpContext = ();
        type DownContext = ();

        fn intrinsic_size(&mut self, _s: MSlock) -> Size {
            self.intrinsic
        }

        fn xsquished_size(&mut self, _s: MSlock) -> Size {
            self.intrinsic
        }

        fn xstretched_size(&mut self, _s: MSlock) -> Size {
            let i = self.intrinsic;
            Size::new(geo::UNBOUNDED, i.h)
        }

        fn ysquished_size(&mut self, _s: MSlock) -> Size {
            self.intrinsic
        }

        fn ystretched_size(&mut self, _s: MSlock) -> Size {
            self.intrinsic
        }

        fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
            ()
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.current.listen(move |_v, s| {
                let Some(invalidator) = invalidator.upgrade() else {
                    return false;
                };
                invalidator.invalidate(s);
                true
            }, s);

            let mut add_options = true;
            let nv = if let Some((nv, source)) = backing_source {
                if source.options != self.options {
                    dropdown_clear(nv.backing(), s);
                }
                else {
                    add_options = false;
                }

                nv
            }
            else {
                unsafe {
                    NativeView::new(init_dropdown(self.current.clone(), s), s)
                }
            };

            if add_options {
                for option in &self.options {
                    dropdown_push(nv.backing(), option.clone(), s);
                }
            }

            self.backing = nv.backing();
            self.intrinsic = dropdown_size(self.backing, s);
            nv
        }

        fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock) -> bool {
            if dropdown_select(self.backing, self.current.borrow(s).as_deref(), s) {
                panic!("Dropdown set to invalid option");
            }
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, _s: MSlock) -> (Rect, Rect) {
            (frame.full_rect(), frame.full_rect())
        }
    }
}
