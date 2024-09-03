// TODO

mod attribute {
    mod character {
        use crate::resource::Resource;
        use crate::util::geo::ScreenUnit;
        use crate::view::util::Color;

        // For all attributes, a value of None
        // implies to leave it to the default
        #[derive(Default, Clone, Debug, PartialEq)]
        pub struct CharAttribute {
            bold: Option<bool>,
            italic: Option<bool>,
            underline: Option<bool>,
            strikethrough: Option<bool>,
            back_color: Option<Color>,
            fore_color: Option<Color>,
            size: Option<ScreenUnit>,
            font: Option<Resource>,
        }
    }
    pub use character::*;

    mod run {
        use crate::util::geo::ScreenUnit;

        #[derive(Copy, Clone, Debug, PartialEq)]
        pub enum Justification {
            Leading,
            Center,
            Trailing
        }

        #[derive(Copy, Clone, Debug, PartialEq)]
        pub struct Indentation {
            leading: ScreenUnit,
            trailing: ScreenUnit
        }

        #[derive(Default, Copy, Clone, Debug, PartialEq)]
        pub struct RunAttribute {
            justification: Option<Justification>,
            indentation: Option<Indentation>,
        }
    }
    pub use run::*;

    mod page {
        /// Currently, no page attributes
        #[derive(Default, Clone, Eq, PartialEq)]
        pub struct PageAttribute {

        }
    }
    pub use page::*;
}
pub use attribute::*;

mod text {
    use std::ffi::c_void;
    use crate::core::{Environment, MSlock, StandardVarEnv};
    use crate::native::view::text::{text_init, text_size, text_update};
    use crate::state::{FixedSignal, Signal};
    use crate::util::geo;
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};

    pub struct Text<S> where S: Signal<Target=String> {
        text: S,
        max_lines: u32
    }

    struct TextVP<S> where S: Signal<Target=String> {
        text: S,
        max_lines: u32,
        size: Size,
        backing: *mut c_void,
    }

    impl Text<FixedSignal<String>> {
        pub fn new(text: impl Into<String>) -> Self {
            Text {
                text: FixedSignal::new(text.into()),
                max_lines: 1
            }
        }
    }
    
    impl<S> Text<S> where S: Signal<Target=String> {
        pub fn from_signal(signal: S) -> Self {
            Text {
                text: signal,
                max_lines: 1
            }
        }

        pub fn max_lines(mut self, max_lines: u32) -> Self {
            self.max_lines = max_lines;
            self
        }
    }

    impl<E, S> IntoViewProvider<E> for Text<S>
        where E: Environment,
              E::Variable: AsRef<StandardVarEnv>,
              S: Signal<Target=String> {
        type UpContext = ();
        type DownContext = ();

        fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            TextVP {
                text: self.text,
                max_lines: self.max_lines,
                size: Size::default(),
                backing: 0 as *mut c_void,
            }
        }
    }

    impl<E, S> ViewProvider<E> for TextVP<S>
        where E: Environment,
              E::Variable: AsRef<StandardVarEnv>,
              S: Signal<Target=String> {
        type UpContext = ();
        type DownContext = ();

        fn intrinsic_size(&mut self, _s: MSlock) -> Size {
            self.size
        }

        fn xsquished_size(&mut self, _s: MSlock) -> Size {
            Size::new(0.0, 0.0)
        }

        fn xstretched_size(&mut self, _s: MSlock) -> Size {
            self.size
        }

        fn ysquished_size(&mut self, _s: MSlock) -> Size {
            self.size
        }

        fn ystretched_size(&mut self, _s: MSlock) -> Size {
            self.size
        }

        fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
            ()
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            self.text.listen(move |_, s| {
                let Some(invalidator) = invalidator.upgrade() else {
                    return false;
                };
                invalidator.invalidate(s);
                true
            }, s);

            let nv = if let Some((nv, _)) = backing_source {
                nv
            }
            else {
                unsafe {
                    NativeView::new(text_init(s), s)
                }
            };

            self.backing = nv.backing();
            nv
        }

        fn layout_up(&mut self, _subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            text_update(
                self.backing,
                &*self.text.borrow(s),
                self.max_lines,
                env.variable_env().as_ref(),
                s
            );
            self.size = text_size(self.backing, Size::new(geo::UNBOUNDED, geo::UNBOUNDED), s);
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            let used = text_size(self.backing, frame, s);
            (used.full_rect(), used.full_rect())
        }
    }
}
pub use text::*;

mod text_field {
    use std::ffi::c_void;
    use std::sync::Arc;
    use crate::core::{Environment, MSlock, StandardConstEnv, StandardVarEnv};
    use crate::event::{Event, EventPayload, EventResult, MouseEvent};
    use crate::native::view::text_field::{text_field_copy, text_field_cut, text_field_focus, text_field_init, text_field_paste, text_field_select_all, text_field_size, text_field_unfocus, text_field_update};
    use crate::state::{Bindable, Binding, Filterless, SetAction, Signal, TokenStore};
    use crate::util::geo;
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};
    use crate::view::menu::MenuChannel;

    pub struct TextField<B>
        where B: Binding<Filterless<String>> + Clone,
    {
        text: B,
        focused_token: i32,
        focused: Option<<TokenStore<Option<i32>> as Bindable<Filterless<Option<i32>>>>::Binding>,
        callback: Option<Box<dyn FnMut(MSlock)>>,
        autofocus: bool,
        unstyled: bool,
        secret: bool,
        max_lines: u32
    }

    struct TextFieldVP<B>
        where B: Binding<Filterless<String>> + Clone,
    {
        text: B,
        focused_token: i32,
        focused: Option<<TokenStore<Option<i32>> as Bindable<Filterless<Option<i32>>>>::Binding>,
        callback: Option<Box<dyn FnMut(MSlock)>>,
        autofocus: bool,
        unstyled: bool,
        secret: bool,
        max_lines: u32,
        intrinsic_size: Size,
        last_size: Size,
        backing: *mut c_void,

        select_all_menu: MenuChannel,
        cut_menu: MenuChannel,
        copy_menu: MenuChannel,
        paste_menu: MenuChannel,
    }

    impl<B> TextField<B>
        where B: Binding<Filterless<String>> + Clone,
    {
        pub fn new(binding: B) -> Self
        {
            TextField {
                text: binding,
                focused_token: 0,
                focused: None,
                callback: None,
                autofocus: false,
                unstyled: false,
                secret: false,
                max_lines: 1
            }
        }

        pub fn focused_if_eq(mut self, indicator: <TokenStore<Option<i32>> as Bindable<Filterless<Option<i32>>>>::Binding, token: i32) -> Self {
            self.focused = Some(indicator);
            self.focused_token = token;
            self
        }

        // TODO textfield autofocus
        // pub fn autofocus(mut self) -> Self {
        //     self.autofocus = true;
        //     self
        // }

        // TODO password text
        // pub fn secret(mut self) -> Self {
        //     self.secret = true;
        //     self
        // }

        pub fn action(mut self, f: impl FnMut(MSlock) + 'static) -> Self {
            self.callback = Some(Box::new(f));
            self
        }

        pub fn unstyled(mut self) -> Self {
            self.unstyled = true;
            self
        }

        pub fn max_lines(mut self, max_lines: u32) -> Self {
            self.max_lines = max_lines;
            self
        }
    }

    impl<E, B> IntoViewProvider<E> for TextField<B>
        where E: Environment,
              E::Const: AsRef<StandardConstEnv>,
              E::Variable: AsRef<StandardVarEnv>,
              B: Binding<Filterless<String>> + Clone,
    {
        type UpContext = ();
        type DownContext = ();

        fn into_view_provider(self, env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            TextFieldVP {
                text: self.text,
                focused_token: self.focused_token,
                focused: self.focused,
                callback: self.callback,
                autofocus: self.autofocus,
                unstyled: self.unstyled,
                secret: self.secret,
                max_lines: self.max_lines,
                intrinsic_size: Size::default(),
                last_size: Size::default(),
                backing: 0 as *mut c_void,
                select_all_menu: env.as_ref().channels.select_all_menu.clone(),
                cut_menu: env.as_ref().channels.cut_menu.clone(),
                copy_menu: env.as_ref().channels.copy_menu.clone(),
                paste_menu: env.as_ref().channels.paste_menu.clone(),
            }
        }
    }

    impl<E, B> ViewProvider<E> for TextFieldVP<B>
        where E: Environment,
              E::Variable: AsRef<StandardVarEnv>,
              B: Binding<Filterless<String>> + Clone,
    {
        type UpContext = ();
        type DownContext = ();

        fn intrinsic_size(&mut self, _s: MSlock) -> Size {
            self.intrinsic_size
        }

        fn xsquished_size(&mut self, _s: MSlock) -> Size {
            Size::new(0.0, 0.0)
        }

        fn xstretched_size(&mut self, _s: MSlock) -> Size {
            Size::new(geo::UNBOUNDED, 0.0)
        }

        fn ysquished_size(&mut self, _s: MSlock) -> Size {
            Size::new(0.0, 0.0)
        }

        fn ystretched_size(&mut self, _s: MSlock) -> Size {
            Size::new(0.0, geo::UNBOUNDED)
        }

        fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
            ()
        }

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
            let inv = invalidator.clone();
            self.text.listen(move |_, s| {
                let Some(invalidator) = inv.upgrade() else {
                    return false;
                };
                invalidator.invalidate(s);
                true
            }, s);

            if let Some(ref focused) = self.focused {
                focused.store().equals(Some(self.focused_token), s)
                    .listen(move |_, s| {
                        let Some(invalidator) = invalidator.upgrade() else {
                            return false;
                        };
                        invalidator.invalidate(s);
                        true
                    }, s);
            }

            let nv = if let Some((nv, _)) = backing_source {
                nv
            }
            else {
                let action = self.callback
                    .take()
                    .unwrap_or_else(|| Box::new(|_| {}));

                unsafe {
                    if let Some(ref focused) = self.focused {
                        NativeView::new(text_field_init(self.text.clone(), focused.clone(), action, self.focused_token, self.unstyled, self.secret, s), s)
                    }
                    else {
                        let focused = TokenStore::new(None);
                        NativeView::new(text_field_init(self.text.clone(), focused.binding(), action, self.focused_token, self.unstyled, self.secret, s), s)
                    }
                }
            };

            self.backing = nv.backing();
            nv
        }

        fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            if self.autofocus {
                let view = Arc::downgrade(subtree.owner());
                subtree.window().and_then(|w| w.upgrade()).unwrap()
                    .borrow_main(s)
                    .request_default_focus(view)
            }

            if let Some(ref focused) = self.focused {
                let view = Arc::downgrade(subtree.owner());
                if *focused.borrow(s) == Some(self.focused_token) {
                    subtree.window().and_then(|w| w.upgrade()).unwrap()
                        .borrow_main(s)
                        .request_focus(view);
                }
                else {
                    subtree.window().and_then(|w| w.upgrade()).unwrap()
                        .borrow_main(s)
                        .unrequest_focus(view);
                }
            }

            text_field_update(
                self.backing,
                &*self.text.borrow(s),
                self.max_lines,
                env.variable_env().as_ref(),
                s
            );

            self.intrinsic_size = text_field_size(self.backing, Size::new(geo::UNBOUNDED, geo::UNBOUNDED), s);
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            // reduce a little to avoid flickering
            let inset = Size::new(
                (frame.w - 4.0).max(0.0),
                (frame.h - 4.0).max(0.0)
            );
            let mut size = text_field_size(self.backing, inset, s);
            // always use fully given width
            size.w = frame.w;
            self.last_size = size;

            assert!(size.w <= geo::EFFECTIVELY_UNBOUNDED,
                    "Suggested width is too large for this textfield; \
                     help: set the intrinsic size manually of this textfield");

            (size.full_rect(), size.full_rect())
        }

        fn focused(&self, _rel_depth: u32, s: MSlock) {
            if let Some(ref f) = self.focused {
                if *f.borrow(s) != Some(self.focused_token) {
                    f.apply(SetAction::Set(Some(self.focused_token)), s);
                }
            }

            text_field_focus(self.backing, s);

            let backing = self.backing;
            self.select_all_menu.set(Box::new(move |s| {
                text_field_select_all(backing, s);
            }), None, s);
            self.cut_menu.set(Box::new(move |s| {
                text_field_cut(backing, s);
            }), None, s);
            self.copy_menu.set(Box::new(move |s| {
                text_field_copy(backing, s);
            }), None, s);
            self.paste_menu.set(Box::new(move |s| {
                text_field_paste(backing, s);
            }), None, s);
        }

        fn unfocused(&self, _rel_depth: u32, s: MSlock) {
            if let Some(ref f) = self.focused {
                if *f.borrow(s) == Some(self.focused_token) {
                    f.apply(SetAction::Set(None), s);
                }
            }

            text_field_unfocus(self.backing, s);

            self.select_all_menu.unset(s);
            self.copy_menu.unset(s);
            self.cut_menu.unset(s);
            self.paste_menu.unset(s);
        }
        
        fn handle_event(&self, e: &Event, _s: MSlock) -> EventResult {
            if e.is_mouse() {
                if let EventPayload::Mouse(MouseEvent::LeftDown, at) = &e.payload {
                    if self.last_size.full_rect().contains(*at) {
                        EventResult::FocusAcquire
                    }
                    else {
                        EventResult::FocusRelease
                    }
                }
                else {
                    EventResult::NotHandled
                }
            }
            else {
                // FIXME autofocus is not that great right now
                if self.autofocus {
                    EventResult::FocusAcquire
                }
                else {
                    EventResult::Handled
                }
            }
        }
    }
}
pub use text_field::*;

mod text_view {
    mod state {
        mod attribute_set {
            use crate::view::text::{CharAttribute, PageAttribute, RunAttribute};

            pub trait ToCharAttribute: Default + Send + PartialEq + Clone + 'static {
                fn to_char_attribute(&self) -> impl AsRef<CharAttribute>;
            }

            impl AsRef<CharAttribute> for CharAttribute {
                fn as_ref(&self) -> &CharAttribute {
                    self
                }
            }

            impl ToCharAttribute for CharAttribute {
                fn to_char_attribute(&self) -> impl AsRef<CharAttribute> {
                    self
                }
            }

            pub trait ToRunAttribute: Default + Send + PartialEq + Clone + 'static {
                fn to_run_attribute(&self) -> impl AsRef<RunAttribute>;
                fn merge(first: &Self, second: &Self) -> Self;
            }

            impl AsRef<RunAttribute> for RunAttribute {
                fn as_ref(&self) -> &RunAttribute {
                    self
                }
            }

            impl ToRunAttribute for RunAttribute {
                fn to_run_attribute(&self) -> impl AsRef<RunAttribute> {
                    self
                }

                fn merge(first: &Self, _second: &Self) -> Self {
                    *first
                }
            }

            pub trait ToPageAttribute: Default + Send + PartialEq + Clone + 'static {
                fn to_page_attribute(&self) -> impl AsRef<PageAttribute>;
            }

            impl AsRef<PageAttribute> for PageAttribute {
                fn as_ref(&self) -> &PageAttribute {
                    self
                }
            }

            impl ToPageAttribute for PageAttribute {
                fn to_page_attribute(&self) -> impl AsRef<PageAttribute> {
                    self
                }
            }

            // AttributeSet is just a collection of associated types
            // Send + 'static requirement should be automatically fulfilled
            pub trait AttributeSet: Send + 'static {
                type CharAttribute: ToCharAttribute;
                type RunAttribute: ToRunAttribute;
                type PageAttribute: ToPageAttribute;
            }
        }
        pub use attribute_set::*;

        mod attribute_holder {
            use std::ops::{Mul};
            use crate::state::{GroupAction, GroupBasis, SetAction, Stateful};
            use crate::util::marker::FalseMarker;
            use crate::view::text::text_view::state::ToCharAttribute;

            #[derive(Default)]
            pub struct AttributeHolder<A> {
                pub attribute: A
            }

            impl<A> Stateful for AttributeHolder<A> where A: Send + 'static {
                type Action = SetAction<Self>;
                type HasInnerStores = FalseMarker;
            }

            pub struct RangedAttributeHolder<A> where A: ToCharAttribute {
                // A and its length
                pub attributes: Vec<(A, usize)>
            }

            impl<A> RangedAttributeHolder<A> where A: ToCharAttribute {
                pub fn attribute_at(&self, mut at: usize) -> &A {
                    // FIXME maybe bin search this if size is greater than threshold
                    self.attributes.iter()
                        .find_map(|(a, len)| {
                            if at < *len {
                                Some(a)
                            }
                            else {
                                at -= len;
                                None
                            }
                        }).expect("Invalid Index")
                }
            }
            
            impl<A> Default for RangedAttributeHolder<A> where A: ToCharAttribute {
                fn default() -> Self {
                    RangedAttributeHolder {
                        attributes: vec![],
                    }
                }
            }

            // cant exactly use a word since a single modification doesnt always have a single inverse
            pub enum RangedBasis<A> where A: ToCharAttribute {
                Insert {
                    at: usize,
                    len: usize,
                    attribute: A
                },
                Delete {
                    at: usize,
                    len: usize,
                }
            }

            impl<A> RangedBasis<A> where A: ToCharAttribute {
                fn apply(self, to: &mut RangedAttributeHolder<A>, inverse: &mut Vec<RangedBasis<A>>) {
                    // note that an insert-delete inverse pair may not lead to an identical state
                    // since we do not recombine
                    match self {
                        RangedBasis::Insert { at, len, attribute } => {
                            if len == 0 {
                                return;
                            }

                            let mut start = 0;
                            let mut i = 0;
                            while i < to.attributes.len() && start + to.attributes[i].1 <= at {
                                start += to.attributes[i].1;
                                i += 1
                            }

                            // by this time at is contained in the i'th interval's range
                            // start is the start position of the ith interval
                            assert!(start <= at, "Invalid index");

                            if at == start {
                                // insert normally
                                to.attributes.insert(i, (attribute, len));
                            }
                            else {
                                // if in the middle of a current one, split
                                let right = to.attributes[i].1 - (at - start);
                                to.attributes[i].1 = at - start;
                                to.attributes.insert(i + 1, (to.attributes[i].0.clone(), right));

                                to.attributes.insert(i + 1, (attribute, len));
                            }

                            // inverse action
                            inverse.push(RangedBasis::Delete {
                                at, len
                            })
                        }
                        RangedBasis::Delete { at, len } => {
                            if len == 0 {
                                return;
                            }

                            // half open interval denoting regions that are subsets of this range
                            let mut start = 0;
                            let mut i = 0;
                            while i < to.attributes.len() && at > start {
                                start += to.attributes[i].1;
                                i += 1
                            }

                            assert!(at <= start, "Invalid index");

                            let mut j = i;
                            let mut end = start;
                            while j < to.attributes.len() && end + to.attributes[j].1 <= at + len {
                                end += to.attributes[j].1;
                                j += 1;
                            }

                            assert!(at + len <= end, "Invalid length");

                            // (we go right to left to avoid index issues)
                            if start > at + len {
                                // delete was entirely within a given range
                                debug_assert!(i > 0 && i == j);

                                // decrease previous (which is effectively a split)
                                to.attributes[i - 1].1 -= len;
                                inverse.push(RangedBasis::Insert {
                                    at,
                                    len,
                                    attribute: to.attributes[i - 1].0.clone(),
                                });
                                // fully handled
                                return;
                            }

                            // possibly clip next
                            if j < to.attributes.len() && at + len != end {
                                to.attributes[j].1 -= at + len - end;

                                inverse.push(RangedBasis::Insert {
                                    at: end,
                                    len: at + len - end,
                                    attribute: to.attributes[j].0.clone(),
                                })
                            }

                            // cut the main section
                            let mut delete_loc = end;
                            inverse.extend(
                                to.attributes.splice(i .. j, std::iter::empty())
                                    .rev()
                                    .map(|(a, l)| {
                                        delete_loc -= l;
                                        RangedBasis::Insert {
                                            at: delete_loc,
                                            len: l,
                                            attribute: a,
                                        }
                                    })
                            );

                            // possibly clip previous
                            if i > 0 && at != start{
                                // clip prev
                                to.attributes[i - 1].1 -= start - at;

                                inverse.push(RangedBasis::Insert {
                                    at,
                                    len: start - at,
                                    attribute: to.attributes[i - 1].0.clone(),
                                })
                            }

                            // possibly rejoin i - 1 with j (which has now become i) if the attributes are now equal
                            if i > 0 && i < to.attributes.len() && to.attributes[i - 1] == to.attributes[i] {
                                to.attributes[i - 1].1 += to.attributes[i].1;
                                to.attributes.remove(j);
                            }
                        }
                    }
                }
            }

            pub struct RangedAttributeAction<A> where A: ToCharAttribute {
                pub actions: Vec<RangedBasis<A>>
            }

            impl<A> GroupBasis<RangedAttributeHolder<A>> for RangedAttributeAction<A> where A: ToCharAttribute {
                fn apply(self, to: &mut RangedAttributeHolder<A>) -> Self {
                    let mut inverse = vec![];
                    for action in self.actions {
                        action.apply(to, &mut inverse);
                    }

                    inverse.reverse();
                    RangedAttributeAction {
                        actions: inverse,
                    }
                }

                fn forward_description(&self) -> impl Into<String> {
                    "Change"
                }

                fn backward_description(&self) -> impl Into<String> {
                    "Change"
                }
            }

            impl<A> Mul for RangedAttributeAction<A> where A: ToCharAttribute {
                type Output = Self;

                fn mul(mut self, rhs: Self) -> Self::Output {
                    self.actions.extend(rhs.actions);
                    self
                }
            }

            impl<A> GroupAction<RangedAttributeHolder<A>> for RangedAttributeAction<A> where A: ToCharAttribute {
                fn identity() -> Self {
                    RangedAttributeAction {
                        actions: vec![],
                    }
                }
            }

            impl<A> Stateful for RangedAttributeHolder<A> where A: ToCharAttribute {
                type Action = RangedAttributeAction<A>;
                type HasInnerStores = FalseMarker;
            }
        }
        pub use attribute_holder::*;


        mod run_gui_info {
            use crate::state::{SetAction, Stateful};
            use crate::util::geo::ScreenUnit;
            use crate::util::marker::FalseMarker;

            #[derive(Default)]
            pub struct RunGUIInfo {
                pub height: ScreenUnit,
                pub line: usize,
                pub start_char: usize,
                pub page_position: ScreenUnit
            }

            impl Stateful for RunGUIInfo {
                type Action = SetAction<Self>;
                type HasInnerStores = FalseMarker;
            }
        }

        mod run {
            use std::ops::{Deref, Range};
            use quarve_derive::StoreContainer;
            use crate::core::Slock;
            use crate::state::{Binding, DerivedStore, EditingString, SetAction, Signal, Store, StringActionBasis, Word};
            use crate::state::SetAction::Set;
            use crate::util::marker::ThreadMarker;
            use crate::util::rust_util::DerefMap;
            use crate::view::text::text_view::state::{AttributeSet};
            use crate::view::text::text_view::state::attribute_holder::{AttributeHolder, RangedAttributeAction, RangedAttributeHolder, RangedBasis};
            use crate::view::text::text_view::state::run_gui_info::RunGUIInfo;

            #[derive(StoreContainer)]
            pub struct Run<I, D> where I: AttributeSet, D: AttributeSet {
                content: Store<EditingString>,
                gui_info: DerivedStore<RunGUIInfo>,

                char_intrinsic_attribute: Store<RangedAttributeHolder<I::CharAttribute>>,
                char_derived_attribute: DerivedStore<RangedAttributeHolder<D::CharAttribute>>,

                intrinsic_attribute: Store<AttributeHolder<I::RunAttribute>>,
                derived_attribute: DerivedStore<AttributeHolder<D::RunAttribute>>,
            }

            impl<I, D> Run<I, D> where I: AttributeSet, D: AttributeSet {
                pub(super) fn new() -> Self {
                    Run {
                        content: Store::default(),
                        gui_info: DerivedStore::default(),
                        char_intrinsic_attribute: Store::default(),
                        char_derived_attribute: DerivedStore::default(),
                        intrinsic_attribute: Store::default(),
                        derived_attribute: DerivedStore::default(),
                    }
                }

                pub fn content<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=str> + 'a {
                    DerefMap::new(
                        self.content.borrow(s),
                        |e| e.0.deref()
                    )
                }

                pub fn len(&self, s: Slock<impl ThreadMarker>) -> usize {
                    self.content.borrow(s).0.len()
                }

                pub fn intrinsic<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=I::RunAttribute> + 'a {
                    DerefMap::new(
                        self.intrinsic_attribute.borrow(s),
                        |i| &i.attribute
                    )
                }

                pub fn derived<'a>(&'a self, s: Slock<'a, impl ThreadMarker> ) -> impl Deref<Target=D::RunAttribute> + 'a {
                    DerefMap::new(
                        self.derived_attribute.borrow(s),
                        |i| &i.attribute
                    )
                }

                pub fn char_intrinsic<'a>(&'a self, at: usize, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=I::CharAttribute> + 'a {
                    DerefMap::new(
                        self.char_intrinsic_attribute.borrow(s),
                        move |c| c.attribute_at(at)
                    )
                }

                pub fn char_derived<'a>(&'a self, at: usize, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=D::CharAttribute> +'a {
                    DerefMap::new(
                        self.char_derived_attribute.borrow(s),
                        move |c| c.attribute_at(at)
                    )
                }

                pub fn set_intrinsic(&self, intrinsic: I::RunAttribute, s: Slock<impl ThreadMarker>) {
                    self.intrinsic_attribute.apply(Set(AttributeHolder {
                        attribute: intrinsic
                    }), s);
                }

                pub fn set_derived(&self, derived: D::RunAttribute, s: Slock<impl ThreadMarker>) {
                    self.derived_attribute.apply(Set(AttributeHolder {
                        attribute: derived
                    }), s);
                }

                pub fn replace(&self, range: Range<usize>, with: impl Into<String>, s: Slock<impl ThreadMarker>) {
                    self.replace_with_attributes(
                        range, with,
                        Default::default(), Default::default(),
                        s
                    );
                }

                pub fn replace_with_attributes(
                    &self,
                    range: Range<usize>,
                    with: impl Into<String>,
                    intrinsic: I::CharAttribute,
                    derived: D::CharAttribute,
                    s: Slock<impl ThreadMarker>
                ) {
                    // delete old attrs
                    self.char_derived_attribute.apply(RangedAttributeAction {
                        actions: vec![RangedBasis::Delete {
                            at: range.start,
                            len: range.len(),
                        }],
                    }, s);

                    self.char_intrinsic_attribute.apply(RangedAttributeAction {
                        actions: vec![RangedBasis::Delete {
                            at: range.start,
                            len: range.len(),
                        }],
                    }, s);

                    // modify content
                    self.content.apply(
                        StringActionBasis::ReplaceSubrange(range.clone(), with.into()), s
                    );

                    // insert new attrs
                    self.char_derived_attribute.apply(RangedAttributeAction {
                        actions: vec![RangedBasis::Insert {
                            at: range.start,
                            len: range.len(),
                            attribute: derived
                        }],
                    }, s);

                    self.char_intrinsic_attribute.apply(RangedAttributeAction {
                        actions: vec![RangedBasis::Insert {
                            at: range.start,
                            len: range.len(),
                            attribute: intrinsic
                        }],
                    }, s);
                }

                pub fn set_char_intrinsic(&self, attribute: I::CharAttribute, for_range: Range<usize>, s: Slock<impl ThreadMarker>) {
                    self.char_intrinsic_attribute.apply(RangedAttributeAction {
                        actions: vec![
                            RangedBasis::Delete {
                                at: for_range.start,
                                len: for_range.len(),
                            },
                            RangedBasis::Insert {
                                at: for_range.start,
                                len: for_range.len(),
                                attribute
                            }
                        ],
                    }, s);
                }

                pub fn set_char_derived(&self, attribute: D::CharAttribute, for_range: Range<usize>, s: Slock<impl ThreadMarker>) {
                    self.char_derived_attribute.apply(RangedAttributeAction {
                        actions: vec![
                            RangedBasis::Delete {
                                at: for_range.start,
                                len: for_range.len(),
                            },
                            RangedBasis::Insert {
                                at: for_range.start,
                                len: for_range.len(),
                                attribute
                            }
                        ],
                    }, s);
                }

                pub fn content_action_listen(
                    &self,
                    f: impl FnMut(&EditingString, &Word<StringActionBasis>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.content.action_listen(f, s)
                }

                pub fn content_listen(
                    &self,
                    f: impl FnMut(&EditingString, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.content.listen(f, s)
                }

                pub fn derived_action_listen(
                    &self,
                    f: impl FnMut(&AttributeHolder<D::RunAttribute>, &SetAction<AttributeHolder<D::RunAttribute>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.derived_attribute.action_listen(f, s)
                }

                pub fn derived_listen(
                    &self,
                    f: impl FnMut(&AttributeHolder<D::RunAttribute>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.derived_attribute.listen(f, s)
                }

                pub fn intrinsic_action_listen(
                    &self,
                    f: impl FnMut(&AttributeHolder<I::RunAttribute>, &SetAction<AttributeHolder<I::RunAttribute>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.intrinsic_attribute.action_listen(f, s)
                }

                pub fn intrinsic_listen(
                    &self,
                    f: impl FnMut(&AttributeHolder<I::RunAttribute>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.intrinsic_attribute.listen(f, s)
                }

                pub fn char_derived_action_listen(
                    &self,
                    f: impl FnMut(&RangedAttributeHolder<D::CharAttribute>, &RangedAttributeAction<D::CharAttribute>, Slock) -> bool + Send + 'static,
                    s: Slock
                ) {
                    self.char_derived_attribute.action_listen(f, s);
                }

                pub fn char_derived_listen(
                    &self,
                    f: impl FnMut(&RangedAttributeHolder<D::CharAttribute>, Slock) -> bool + Send + 'static,
                    s: Slock
                ) {
                    self.char_derived_attribute.listen(f, s);
                }

                pub fn char_intrinsic_action_listen(
                    &self,
                    f: impl FnMut(&RangedAttributeHolder<I::CharAttribute>, &RangedAttributeAction<I::CharAttribute>, Slock) -> bool + Send + 'static,
                    s: Slock
                ) {
                    self.char_intrinsic_attribute.action_listen(f, s);
                }

                pub fn char_intrinsic_listen(
                    &self,
                    f: impl FnMut(&RangedAttributeHolder<I::CharAttribute>, Slock) -> bool + Send + 'static,
                    s: Slock
                ) {
                    self.char_intrinsic_attribute.listen(f, s);
                }
            }
        }
        pub use run::*;

        mod runs {
            // The type that stores a list of runs
            pub type RunsContainer<T> = Vec<T>;
        }
        pub use runs::*;

        mod page {
            use std::ops::Deref;
            use quarve_derive::StoreContainer;
            use crate::core::Slock;
            use crate::state::{Binding, DerivedStore, SetAction, Signal, Store, VecActionBasis, Word};
            use crate::state::SetAction::Set;
            use crate::util::marker::ThreadMarker;
            use crate::util::rust_util::DerefMap;
            use crate::view::text::text_view::state::{AttributeSet, Run, RunsContainer};
            use crate::view::text::text_view::state::attribute_holder::{AttributeHolder};

            #[derive(StoreContainer)]
            pub struct Page<I, D> where I: AttributeSet, D: AttributeSet {
                pub(crate) runs: Store<Vec<Run<I, D>>>,

                pub(crate) page_intrinsic_attribute: Store<AttributeHolder<I::PageAttribute>>,
                pub(crate) page_derived_attribute: DerivedStore<AttributeHolder<D::PageAttribute>>
            }

            impl<I, D> Page<I, D> where I: AttributeSet, D: AttributeSet {
                pub fn new() -> Self {
                    Page {
                        runs: Store::new(vec![]),
                        page_intrinsic_attribute: Store::default(),
                        page_derived_attribute: DerivedStore::default(),
                    }
                }

                pub fn num_runs(&self, s: Slock<impl ThreadMarker>) -> usize {
                    self.runs.borrow(s).len()
                }

                pub fn runs<'a>(&'a mut self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=RunsContainer<Run<I, D>>> + 'a {
                    self.runs.borrow(s)
                }

                pub fn run<'a>(&'a self, index: usize, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=Run<I, D>> + 'a {
                    DerefMap::new(
                        self.runs.borrow(s),
                        move |runs| &runs[index]
                    )
                }

                pub fn insert_run<'a>(&'a self, at: usize, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=Run<I, D>> + 'a {
                    self.runs.apply(VecActionBasis::Insert(Run::new(), at), s);
                    self.run(at, s)
                }

                pub fn remove_run(&self, at: usize, s: Slock<impl ThreadMarker>) {
                    self.runs.apply(VecActionBasis::Remove(at), s);
                }

                pub fn intrinsic<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=I::PageAttribute> + 'a {
                    DerefMap::new(
                        self.page_intrinsic_attribute.borrow(s),
                        |p| &p.attribute
                    )
                }

                pub fn derived<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=D::PageAttribute> + 'a {
                    DerefMap::new(
                        self.page_derived_attribute.borrow(s),
                        |d| &d.attribute
                    )
                }

                pub fn set_intrinsic(&self, attribute: I::PageAttribute, s: Slock<impl ThreadMarker>) {
                    self.page_intrinsic_attribute.apply(Set(AttributeHolder {
                        attribute
                    }), s);
                }

                pub fn set_derived(&self, attribute: D::PageAttribute, s: Slock<impl ThreadMarker>) {
                    self.page_derived_attribute.apply(Set(AttributeHolder {
                        attribute
                    }), s);
                }

                pub fn replace_range(
                    &self,
                    start_run: usize,
                    start_char: usize,
                    end_run: usize,
                    end_char: usize,
                    with: impl Into<String>,
                    s: Slock<impl ThreadMarker>
                ) {
                    // if start_run == end_run {
                    //     let range = start_char .. end_char;
                    //     self.run(start_run, s)
                    //         .replace(range, with, s);
                    // }
                    // else {
                    //     // delete all intermediate runs
                    //     if end_run > start_run + 1 {
                    //         self.runs.apply(VecActionBasis::RemoveMany(start_run + 1 .. end_run), s);
                    //     }
                    //
                    //     let new_run_attribute =
                    //         I::RunAttribute::merge(self.run(start_run, s).deref(), self.run(start_run + 1, s).deref());
                    //
                    //     let next_run = self.run(start_run + 1, s);
                    //     self.run(start_run, s)
                    //         .replace(start_char);
                    //
                    //     hello
                    //     // elide old run
                    //     self.remove_run(start_run + 1, s);
                    //
                    // }

                    // insert
                }

                pub fn build_full_content(&mut self, s: Slock<impl ThreadMarker>) -> String {
                    let runs = self.runs(s);
                    let contents = runs.iter()
                            .map(|run| run.content(s));
                    let mut ret = String::new();

                    for line in contents {
                        ret.push_str(line.deref());
                        ret.push('\n');
                    }

                    if !ret.is_empty() {
                        // remove trailing new line
                        ret.pop();
                    }

                    ret
                }

                // NOTE, currently a vector is used
                // but this is planned to be changed
                pub fn runs_action_listen(
                    &self,
                    f: impl FnMut(&RunsContainer<Run<I, D>>, &Word<VecActionBasis<Run<I, D>>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.runs.action_listen(f, s);
                }

                // listens whenever a run is inserted or removed
                // DOES not get called whenever a run is modified
                pub fn runs_listen(
                    &self,
                    f: impl FnMut(&RunsContainer<Run<I, D>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.runs.listen(f, s);
                }

                pub fn intrinsic_action_listen(
                    &self,
                    f: impl FnMut(&AttributeHolder<I::PageAttribute>,  &SetAction<AttributeHolder<I::PageAttribute>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.page_intrinsic_attribute.action_listen(f, s);
                }

                pub fn intrinsic_listen(
                    &self,
                    f: impl FnMut(&AttributeHolder<I::PageAttribute>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.page_intrinsic_attribute.listen(f, s);
                }

                pub fn derived_action_listen(
                    &self,
                    f: impl FnMut(&AttributeHolder<D::PageAttribute>, &SetAction<AttributeHolder<D::PageAttribute>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.page_derived_attribute.action_listen(f, s);
                }

                pub fn derived_listen(
                    &self,
                    f: impl FnMut(&AttributeHolder<D::PageAttribute>, Slock) -> bool + Send + 'static + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.page_derived_attribute.listen(f, s);
                }
            }
        }
        pub use page::*;

        mod cursor_state {
            use quarve_derive::StoreContainer;
            use crate::core::Slock;
            use crate::state::{Bindable, Binding, Buffer, Filterless, Signal, Store};
            use crate::state::SetAction::Set;
            use crate::util::marker::ThreadMarker;

            #[derive(StoreContainer)]
            pub struct CursorState {
                page_num: Store<usize>,

                start_run: Store<usize>,
                start_char: Store<usize>,

                end_run: Store<usize>,
                end_char: Store<usize>,
            }

            impl CursorState {
                pub fn new() -> Self {
                    CursorState {
                        page_num: Store::default(),
                        start_run: Store::default(),
                        start_char: Store::default(),
                        end_run: Store::default(),
                        end_char: Store::default(),
                    }
                }

                pub fn page_binding(&self) -> impl Binding<Filterless<usize>> {
                    self.page_num.binding()
                }

                pub fn start_run_binding(&self) -> impl Binding<Filterless<usize>> {
                    self.start_run.binding()
                }

                pub fn end_run_binding(&self) -> impl Binding<Filterless<usize>> {
                    self.end_run.binding()
                }

                pub fn start_char_binding(&self) -> impl Binding<Filterless<usize>> {
                    self.start_char.binding()
                }

                pub fn end_char_binding(&self) -> impl Binding<Filterless<usize>> {
                    self.end_char.binding()
                }

                pub fn page(&self, s: Slock<impl ThreadMarker>) -> usize {
                    *self.page_num.borrow(s)
                }

                pub fn start_run(&self, s: Slock<impl ThreadMarker>) -> usize {
                    *self.start_run.borrow(s)
                }

                pub fn end_run(&self, s: Slock<impl ThreadMarker>) -> usize {
                    *self.end_run.borrow(s)
                }

                pub fn start_char(&self, s: Slock<impl ThreadMarker>) -> usize {
                    *self.start_char.borrow(s)
                }

                pub fn end_char(&self, s: Slock<impl ThreadMarker>) -> usize {
                    *self.end_char.borrow(s)
                }

                pub fn set_range(
                    &self,
                    start_run: usize,
                    start_char: usize,
                    end_run: usize,
                    end_char: usize,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.start_run.apply(Set(start_run), s);
                    self.start_char.apply(Set(start_char), s);
                    self.end_run.apply(Set(end_run), s);
                    self.end_char.apply(Set(end_char), s);
                }

                pub fn set_page(&self, page: usize, s: Slock<impl ThreadMarker>) {
                    self.page_num.apply(Set(page), s);
                }

                // Function is called with page, start_run, start_char, end_run, end_char
                pub fn listen(
                    &self,
                    f: impl FnMut(usize, usize, usize, usize, usize, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    let stores = [&self.page_num, &self.start_run, &self.start_char, &self.end_run, &self.end_char];
                    // (alive, function, current args)
                    let state = Buffer::new(
                        (true, f, stores.map(|store| *store.borrow(s)))
                    );

                    for (i, store) in stores.into_iter().enumerate() {
                        // basically a clone
                        // we use the function to determine when to return false
                        // rather than weak/strong (we also dont need to worry about cycles)
                        let my_state = state.downgrade().upgrade().unwrap();
                        store.listen(move |val, s| {
                            let mut state = my_state.borrow_mut(s);
                            if !state.0 {
                                return false;
                            }

                            // update appropriate argument
                            state.2[i] = *val;
                            let args = state.2;
                            state.0 = (state.1)(args[0], args[1], args[2], args[3], args[4], s);

                            state.0
                        }, s)
                    }
                }
            }
        }
        pub use cursor_state::*;

        mod text_view_state {
            use std::ops::Deref;
            use quarve_derive::StoreContainer;
            use crate::core::Slock;
            use crate::state::{Signal, Binding, Store, VecActionBasis, Word};
            use crate::util::marker::ThreadMarker;
            use crate::util::rust_util::DerefMap;
            use crate::view::text::text_view::state::{AttributeSet, CursorState, Page};

            #[derive(StoreContainer)]
            pub struct TextViewState<I, D> where I: AttributeSet, D: AttributeSet {
                pages: Store<Vec<Page<I, D>>>,
                cursor: CursorState,
            }

            impl<I, D> TextViewState<I, D> where I: AttributeSet, D: AttributeSet {
                pub fn new() -> Self {
                    TextViewState {
                        pages: Store::new(vec![]),
                        cursor: CursorState::new(),
                    }
                }

                pub fn pages<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=Vec<Page<I, D>>> + 'a {
                    self.pages.borrow(s)
                }

                pub fn page<'a>(&'a self, at: usize, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=Page<I, D>> + 'a {
                    DerefMap::new(
                        self.pages.borrow(s),
                        move |p| &p[at]
                    )
                }

                pub fn insert_page(&self, page: Page<I, D>, at: usize, s: Slock) {
                    self.pages.apply(
                        VecActionBasis::Insert(page, at), s
                    )
                }

                pub fn remove_page(&self, at: usize, s: Slock<impl ThreadMarker>) {
                    self.pages.apply(
                        VecActionBasis::Remove(at), s
                    )
                }


                pub fn replace_selection(&self, with: impl Into<String>, s: Slock<impl ThreadMarker>) {
                    self.page(self.cursor.page(s), s)
                        .replace_range(
                            self.cursor.start_run(s),
                            self.cursor.start_char(s),
                            self.cursor.end_run(s),
                            self.cursor.end_char(s),
                            with, s
                        );
                }

                pub fn action_listen(
                    &self,
                    f: impl FnMut(&Vec<Page<I, D>>, &Word<VecActionBasis<Page<I, D>>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.pages.action_listen(f, s);
                }

                pub fn listen(
                    &self,
                    f: impl FnMut(&Vec<Page<I, D>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.pages.listen(f, s);
                }
            }
        }
        pub use text_view_state::*;
    }
    pub use state::*;

    mod text_view {
        use std::ffi::c_void;
        use crate::core::{Environment, MSlock};
        use crate::state::{Signal, StoreContainerView};
        use crate::util::geo::Size;
        use crate::view::{IntoViewProvider};
        use crate::view::text::{AttributeSet, Run, TextViewState};

        trait TextViewProvider<E> where E: Environment {
            type IntrinsicAttribute: AttributeSet;
            type DerivedAttribute: AttributeSet;

            const PAGE_INSET: Size;


            fn init(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock);

            fn tab(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock);
            fn untab(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock);
            fn newline(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock);
            fn alt_newline(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock);

            fn run_decoration(
                &mut self,
                number: impl Signal<Target=usize>,
                run: &Run<Self::IntrinsicAttribute, Self::DerivedAttribute>
            ) -> impl IntoViewProvider<E>;

            fn page_background(
                &self,
            ) -> impl IntoViewProvider<E>;
        }

        // Composed as a series of pages
        // We handle the scrollview stuff ourselves then
        pub struct TextView<E, P>
            where P: TextViewProvider<E>,
                  E: Environment
        {
            provider: P,
            state: StoreContainerView<TextViewState<P::IntrinsicAttribute, P::DerivedAttribute>>,
        }

        impl<E, P> TextView<E, P>
            where P: TextViewProvider<E>,
                  E: Environment
        {
            pub fn new(state: StoreContainerView<TextViewState<P::IntrinsicAttribute, P::DerivedAttribute>>) -> Self {
                todo!()
            }
        }

        // struct PageVP {
        //     backing: *mut c_void,
        //     decoration: P
        // }

        struct TextViewVP<E, P>
            where P: TextViewProvider<E>,
                  E: Environment
        {
            provider: P,
            state: StoreContainerView<TextViewState<P::IntrinsicAttribute, P::DerivedAttribute>>,

            scroll_view: *mut c_void,
            // pages: Vec<View<E, PageVP<>>>
        }
    }
    pub use text_view::*;
}
pub use text_view::*;

mod env {
    use std::ops::Deref;
    use std::path::Path;
    use crate::core::{Environment, MSlock, StandardVarEnv, TextEnv};
    use crate::resource::Resource;
    use crate::util::geo::ScreenUnit;
    use crate::view::modifers::{EnvironmentModifier, EnvModifiable, EnvModifierIVP};
    use crate::view::{IntoViewProvider, WeakInvalidator};
    use crate::view::util::Color;

    // FIXME unnecessary clones for many operations
    #[derive(Default)]
    pub struct TextEnvModifier {
        last_env: Option<TextEnv>,
        bold: Option<bool>,
        italic: Option<bool>,
        underline: Option<bool>,
        strikethrough: Option<bool>,
        color: Option<Color>,
        backcolor: Option<Color>,
        font: Option<Option<Resource>>,
        size: Option<ScreenUnit>,
    }

    impl<E> EnvironmentModifier<E> for TextEnvModifier where E: Environment, E::Variable: AsMut<StandardVarEnv> {
        fn init(&mut self, _invalidator: WeakInvalidator<E>, _s: MSlock) {

        }

        fn push_environment(&mut self, env: &mut E::Variable, _s: MSlock) {
            self.last_env = Some(env.as_mut().text.clone());

            let text = &mut env.as_mut().text;
            text.bold = self.bold.unwrap_or(text.bold);
            text.italic = self.italic.unwrap_or(text.italic);
            text.underline = self.underline.unwrap_or(text.underline);
            text.strikethrough = self.strikethrough.unwrap_or(text.strikethrough);
            text.color = self.color.unwrap_or(text.color);
            text.backcolor = self.backcolor.unwrap_or(text.backcolor);
            text.font = self.font.clone().unwrap_or_else(|| text.font.clone());
            text.size = self.size.unwrap_or(text.size);
        }

        fn pop_environment(&mut self, env: &mut E::Variable, _s: MSlock) {
            env.as_mut().text = self.last_env.take().unwrap();
        }
    }

    pub trait TextModifier<E>: IntoViewProvider<E> where E: Environment, E::Variable: AsMut<StandardVarEnv> {
        fn bold(self) -> EnvModifierIVP<E, Self, TextEnvModifier>;
        fn italic(self) -> EnvModifierIVP<E, Self, TextEnvModifier>;
        fn underline(self) -> EnvModifierIVP<E, Self, TextEnvModifier>;
        fn strikethrough(self) -> EnvModifierIVP<E, Self, TextEnvModifier>;

        fn text_color(self, color: Color) -> EnvModifierIVP<E, Self, TextEnvModifier>;
        fn text_backcolor(self, color: Color) -> EnvModifierIVP<E, Self, TextEnvModifier>;
        fn text_font(self, rel_path: &str) -> EnvModifierIVP<E, Self, TextEnvModifier>;
        fn text_font_resource(self, resource: Resource) -> EnvModifierIVP<E, Self, TextEnvModifier>;
        fn text_size(self, size: impl Into<ScreenUnit>) -> EnvModifierIVP<E, Self, TextEnvModifier>;
    }

    impl<E, I> TextModifier<E> for I where E: Environment, E::Variable: AsMut<StandardVarEnv>, I: IntoViewProvider<E> {
        fn bold(self) -> EnvModifierIVP<E, Self, TextEnvModifier> {
            let mut text = TextEnvModifier::default();
            text.bold = Some(true);
            self.env_modifier(text)
        }

        fn italic(self) -> EnvModifierIVP<E, Self, TextEnvModifier> {
            let mut text = TextEnvModifier::default();
            text.italic = Some(true);
            self.env_modifier(text)
        }

        fn underline(self) -> EnvModifierIVP<E, Self, TextEnvModifier> {
            let mut text = TextEnvModifier::default();
            text.underline = Some(true);
            self.env_modifier(text)
        }

        fn strikethrough(self) -> EnvModifierIVP<E, Self, TextEnvModifier> {
            let mut text = TextEnvModifier::default();
            text.strikethrough = Some(true);
            self.env_modifier(text)
        }

        fn text_color(self, color: Color) -> EnvModifierIVP<E, Self, TextEnvModifier> {
            let mut text = TextEnvModifier::default();
            text.color = Some(color);
            self.env_modifier(text)
        }

        fn text_backcolor(self, color: Color) -> EnvModifierIVP<E, Self, TextEnvModifier> {
            let mut text = TextEnvModifier::default();
            text.backcolor = Some(color);
            self.env_modifier(text)
        }

        fn text_font(self, rel_path: &str) -> EnvModifierIVP<E, Self, TextEnvModifier> {
            let path = Path::new("font").join(rel_path);

            let mut text = TextEnvModifier::default();
            text.font = Some(Some(Resource::named(path.deref())));
            self.env_modifier(text)
        }

        fn text_font_resource(self, resource: Resource) -> EnvModifierIVP<E, Self, TextEnvModifier> {
            let mut text = TextEnvModifier::default();
            text.font = Some(Some(resource));
            self.env_modifier(text)
        }

        fn text_size(self, size: impl Into<ScreenUnit>) -> EnvModifierIVP<E, Self, TextEnvModifier> {
            let mut text = TextEnvModifier::default();
            text.size = Some(size.into());
            self.env_modifier(text)
        }
    }
}
pub use env::*;
