pub use attribute::*;
pub use env::*;
pub use text::*;
pub use text_field::*;
pub use text_view::*;

mod attribute {
    pub use character::*;
    pub use page::*;
    pub use run::*;

    mod character {
        use crate::view::util::Color;

        // For all attributes, a value of None
        // implies to leave it to the default
        #[derive(Default, Clone, Debug, PartialEq)]
        pub struct CharAttribute {
            pub bold: Option<bool>,
            pub italic: Option<bool>,
            pub underline: Option<bool>,
            pub strikethrough: Option<bool>,
            pub back_color: Option<Color>,
            pub fore_color: Option<Color>,
        }
    }

    mod run {
        use crate::util::geo::ScreenUnit;

        #[derive(Copy, Clone, Debug, PartialEq)]
        pub enum Justification {
            Leading,
            Center,
            Trailing
        }

        impl Default for Justification {
            fn default() -> Self {
                Justification::Leading
            }
        }

        #[derive(Copy, Clone, Debug, PartialEq, Default)]
        pub struct Indentation {
            pub leading: ScreenUnit,
            pub trailing: ScreenUnit
        }

        #[derive(Default, Copy, Clone, Debug, PartialEq)]
        pub struct RunAttribute {
            pub justification: Option<Justification>,
            pub indentation: Option<Indentation>,
        }
    }

    mod page {
        /// Currently, no page attributes
        #[derive(Default, Clone, PartialEq)]
        pub struct PageAttribute {
        }
    }
}

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
        focused: <TokenStore<Option<i32>> as Bindable<Filterless<Option<i32>>>>::Binding,
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
                focused: self.focused.unwrap_or(TokenStore::new(None).binding()),
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

            self.focused.store().equals(Some(self.focused_token), s)
                .listen(move |_, s| {
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
                let action = self.callback
                    .take()
                    .unwrap_or_else(|| Box::new(|_| {}));

                unsafe {
                    NativeView::new(text_field_init(self.text.clone(), self.focused.clone(), action, self.focused_token, self.unstyled, self.secret, s), s)
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

            let view = Arc::downgrade(subtree.owner());
            if *self.focused.borrow(s) == Some(self.focused_token) {
                subtree.window().and_then(|w| w.upgrade()).unwrap()
                    .borrow_main(s)
                    .request_focus(view);
            }
            else {
                subtree.window().and_then(|w| w.upgrade()).unwrap()
                    .borrow_main(s)
                    .unrequest_focus(view);
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
            if *self.focused.borrow(s) != Some(self.focused_token) {
                self.focused.apply(SetAction::Set(Some(self.focused_token)), s);
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
            if *self.focused.borrow(s) == Some(self.focused_token) {
                self.focused.apply(SetAction::Set(None), s);
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

mod text_view {
    pub use state::*;
    pub use text_view::*;

    mod state {
        pub use attribute_holder::*;
        pub use attribute_set::*;
        pub use cursor_state::*;
        pub use page::*;
        pub use run::*;
        pub use runs::*;
        pub use text_view_state::*;

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

        mod attribute_holder {
            use std::ops::{Mul, Range};

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
                // note that even if returns false, may actually be true
                // this is just for optimization purposes
                pub fn range_equals(&self, range: Range<usize>, attribute: &A) -> bool {
                    let mut at = 0;
                    for (a, len) in &self.attributes {
                        if at >= range.start {
                            return range.end <= at + len && a == attribute;
                        }

                        at += *len;
                    }
                    false
                }

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

                pub fn subrange(&self, range: Range<usize>) -> Self {
                    let mut attributes = Vec::new();

                    // scroll to initial position
                    let mut ind = 0;
                    // range position
                    let mut i = 0;
                    while ind < self.attributes.len() && i + self.attributes[ind].1 <= range.start {
                        i += self.attributes[ind].1;
                        ind += 1;
                    }

                    i = range.start;
                    while i < range.end {
                        let next = (i + self.attributes[ind].1).min(range.end);
                        attributes.push((self.attributes[ind].0.clone(), next - i));
                        i = next;
                    }

                    RangedAttributeHolder {
                        attributes,
                    }
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
            #[derive(Debug)]
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
                                if i < to.attributes.len() && to.attributes[i].0 == attribute {
                                    // try merge with next
                                    to.attributes[i].1 += len
                                }
                                else if i > 0 && to.attributes[i - 1].0 == attribute {
                                    // try merge with previous
                                    to.attributes[i - 1].1 += len
                                }
                                else {
                                    // insert normally
                                    to.attributes.insert(i, (attribute, len));
                                }
                            }
                            else {
                                // if in the middle of a current one, split (or extend if equal)
                                if to.attributes[i].0 == attribute {
                                    to.attributes[i].1 += len
                                }
                                else {
                                    let right = to.attributes[i].1 - (at - start);
                                    to.attributes[i].1 = at - start;
                                    to.attributes.insert(i + 1, (to.attributes[i].0.clone(), right));

                                    to.attributes.insert(i + 1, (attribute, len));
                                }
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

                            assert!(j < to.attributes.len() || end >= at + len, "Invalid Index");

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
                                to.attributes.remove(i);
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

        mod run_gui_info {
            use crate::state::{SetAction, Stateful};
            use crate::util::geo::ScreenUnit;
            use crate::util::marker::FalseMarker;

            #[derive(Copy, Clone)]
            pub struct RunGUIInfo {
                // maintained by run decorator
                pub added_decoration_listener: bool,
                // maintained by page vp
                pub added_vp_listener: bool,

                // maintained more or less by state which adds the listeners
                // for insertion, pagevp is responsible for setting it to true
                pub dirty: bool,
                // maintained by run
                pub codeunits: usize,

                // maintained by page vp
                pub line: usize,
                pub page_height: ScreenUnit,
            }

            impl Stateful for RunGUIInfo {
                type Action = SetAction<Self>;
                type HasInnerStores = FalseMarker;
            }
        }

        mod run {
            use std::ops::{Deref, Range};

            use quarve_derive::StoreContainer;

            use crate::core::{MSlock, Slock};
            use crate::state::{Bindable, Binding, DerivedStore, EditingString, GroupAction, SetAction, Signal, Store, StringActionBasis, Word};
            use crate::state::SetAction::Set;
            use crate::util::marker::ThreadMarker;
            use crate::util::rust_util::DerefMap;
            use crate::view::text::text_view::state::attribute_holder::{AttributeHolder, RangedAttributeAction, RangedAttributeHolder, RangedBasis};
            use crate::view::text::text_view::state::AttributeSet;
            use crate::view::text::text_view::state::run_gui_info::RunGUIInfo;
            use crate::view::undo_manager::history_elide;

            #[derive(StoreContainer)]
            pub struct Run<I, D> where I: AttributeSet, D: AttributeSet {
                content: Store<EditingString>,
                pub(crate) gui_info: DerivedStore<RunGUIInfo>,

                pub(crate) char_intrinsic_attribute: Store<RangedAttributeHolder<I::CharAttribute>>,
                // we can't actually make it derived store
                // since we still want that for actual insertions or deletions, the inverse action to be applied
                // all changes that shouldn't result in an undo
                // are manually carried out
                pub(crate) char_derived_attribute: Store<RangedAttributeHolder<D::CharAttribute>>,

                intrinsic_attribute: Store<AttributeHolder<I::RunAttribute>>,
                derived_attribute: DerivedStore<AttributeHolder<D::RunAttribute>>,
            }

            impl<I, D> Run<I, D> where I: AttributeSet, D: AttributeSet {
                pub(super) fn new_with(
                    initial: String,
                    intrinsic: RangedAttributeHolder<I::CharAttribute>,
                    derived: RangedAttributeHolder<D::CharAttribute>,
                    s: Slock<impl ThreadMarker>
                ) -> Self {
                    let codeunits = initial.encode_utf16().count();
                    let run = Run {
                        content: Store::new(EditingString(initial)),
                        gui_info: DerivedStore::new(RunGUIInfo {
                            added_decoration_listener: false,
                            added_vp_listener: false,
                            dirty: true,
                            codeunits,
                            line: 0,
                            page_height: 0.0,
                        }),
                        char_intrinsic_attribute: Store::new(intrinsic),
                        char_derived_attribute: Store::new(derived),
                        intrinsic_attribute: Store::default(),
                        derived_attribute: DerivedStore::default(),
                    };
                    run.debug_assertions(s);

                    // upon change, set dirty flag to true
                    let gui = run.gui_info.binding();
                    run.char_intrinsic_attribute.listen(move |_, s| {
                        let mut g = *gui.borrow(s);
                        if !g.dirty {
                            g.dirty = true;
                            gui.apply(Set(g), s);
                        }
                        true
                    }, s);

                    let gui = run.gui_info.binding();
                    run.char_derived_attribute.listen(move |_, s| {
                        let mut g = *gui.borrow(s);
                        if !g.dirty {
                            g.dirty = true;
                            gui.apply(Set(g), s);
                        }
                        true
                    }, s);

                    let gui = run.gui_info.binding();
                    run.intrinsic_attribute.listen(move |_, s| {
                        let mut g = *gui.borrow(s);
                        if !g.dirty {
                            g.dirty = true;
                            gui.apply(Set(g), s);
                        }
                        true
                    }, s);

                    let gui = run.gui_info.binding();
                    run.derived_attribute.listen(move |_, s| {
                        let mut g = *gui.borrow(s);
                        if !g.dirty {
                            g.dirty = true;
                            gui.apply(Set(g), s);
                        }
                        true
                    }, s);

                    let gui = run.gui_info.binding();
                    run.content.listen(move |c,  s| {
                        let mut g = *gui.borrow(s);
                        g.dirty = true;
                        g.codeunits = c.0.encode_utf16().count();
                        gui.apply(Set(g), s);
                        true
                    }, s);

                    run
                }
                pub(super) fn new(s: Slock<impl ThreadMarker>) -> Self {
                    Run::new_with(
                        "".to_owned(),
                        RangedAttributeHolder::default(), RangedAttributeHolder::default(),
                        s
                    )
                }

                pub(super) fn split_trail(&self, at: usize, s: MSlock) -> Run<I, D> {
                    let len = self.len(s);
                    let content = self.content.borrow(s).0[at..len].to_string();
                    let derived = self.char_derived_attribute.borrow(s)
                        .subrange(at..len);
                    let intrinsic = self.char_intrinsic_attribute.borrow(s)
                        .subrange(at..len);

                    self.replace(at..len, "", s);
                    self.debug_assertions(s);

                    Run::new_with(content, intrinsic, derived, s)
                }

                pub(crate) fn debug_assertions(&self, s: Slock<impl ThreadMarker>) {
                    let i = self.char_intrinsic_attribute.borrow(s).attributes.iter().fold(0, |a,b| a+b.1);
                    let j = self.char_derived_attribute.borrow(s).attributes.iter().fold(0, |a,b| a+b.1);
                    let k = self.content.borrow(s).0.len();
                    debug_assert!(i == j && j == k);
                }

                pub(super) fn merge_from(&self, run: &Run<I, D>, s: Slock<impl ThreadMarker>) {
                    // append content normally
                    let len = self.len(s);
                    self.content.apply(StringActionBasis::ReplaceSubrange(len..len, run.content.borrow(s).0.clone()), s);

                    // attributes aren't too bad either
                    let mut pos = len;
                    let mut actions = RangedAttributeAction::identity();
                    for (attr, len) in run.char_derived_attribute.borrow(s).attributes.iter() {
                        actions.actions.push(
                            RangedBasis::Insert {
                                at: pos,
                                len: *len,
                                attribute: attr.clone(),
                            }
                        );
                        pos += *len;
                    }
                    self.char_derived_attribute.apply(actions, s);

                    let mut pos = len;
                    let mut actions = RangedAttributeAction::identity();
                    for (attr, len) in run.char_intrinsic_attribute.borrow(s).attributes.iter() {
                        actions.actions.push(
                            RangedBasis::Insert {
                                at: pos,
                                len: *len,
                                attribute: attr.clone(),
                            }
                        );

                        pos += *len;
                    }

                    self.char_intrinsic_attribute.apply(actions, s);

                    self.debug_assertions(s);
                }

                pub fn content_signal(&self) -> impl Signal<Target=EditingString> {
                    self.content.signal()
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

                /// Due to a race condition, modification of text contents
                /// may only be performed on the main thread
                // (technically an undo could be called on other threads, but this wont happen in practice)
                pub(crate) fn replace(&self, range: Range<usize>, with: impl Into<String>, s: MSlock) {
                    self.replace_with_attributes(
                        range, with,
                        Default::default(), Default::default(),
                        s
                    );
                }

                /// Due to a race condition, modification of text contents
                /// may only be performed on the main thread
                pub(crate) fn replace_with_attributes(
                    &self,
                    range: Range<usize>,
                    with: impl Into<String>,
                    intrinsic: I::CharAttribute,
                    derived: D::CharAttribute,
                    s: MSlock
                ) {
                    // delete old attrs
                    if !range.is_empty() {
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
                    }

                    // modify content
                    let replacement = with.into();
                    assert!(replacement.chars().all(|c| c != '\n'), "Cannot replace with newline!. Use Page.replace_range instead");
                    let replacement_len = replacement.len();
                    self.content.apply(
                        StringActionBasis::ReplaceSubrange(range.clone(), replacement), s
                    );

                    // insert new attrs
                    if replacement_len > 0 {
                        self.char_derived_attribute.apply(RangedAttributeAction {
                            actions: vec![RangedBasis::Insert {
                                at: range.start,
                                len: replacement_len,
                                attribute: derived
                            }],
                        }, s);

                        self.char_intrinsic_attribute.apply(RangedAttributeAction {
                            actions: vec![RangedBasis::Insert {
                                at: range.start,
                                len: replacement_len,
                                attribute: intrinsic
                            }],
                        }, s);
                    }

                    self.debug_assertions(s);
                }

                pub fn set_char_intrinsic(&self, attribute: I::CharAttribute, for_range: Range<usize>, s: Slock<impl ThreadMarker>) {
                    if self.char_intrinsic_attribute.borrow(s).range_equals(for_range.clone(), &attribute) {
                        return
                    }

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

                    self.debug_assertions(s);
                }

                pub fn set_char_derived(&self, attribute: D::CharAttribute, for_range: Range<usize>, s: Slock<impl ThreadMarker>) {
                    if self.char_derived_attribute.borrow(s).range_equals(for_range.clone(), &attribute) {
                        return
                    }

                    history_elide(|| {
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
                    });

                    self.debug_assertions(s);
                }

                // FIXME, these two could be more efficient (no clones or vec)
                pub fn modify_char_intrinsic(&self, range: Range<usize>, mut f: impl FnMut(I::CharAttribute) -> I::CharAttribute, s: Slock<impl ThreadMarker>) {
                    let cia = self.char_intrinsic_attribute.borrow(s);
                    let subrange = cia.subrange(range.clone()).attributes;

                    let mut loc = range.start;
                    let mut any_changed = false;
                    let mut mapped_subrange: Vec<_> = subrange.into_iter().map(|(a, l)| {
                        let attribute = f(a);
                        // purposefully short circuit
                        any_changed = any_changed || attribute != *cia.attribute_at(loc);
                        let ret = RangedBasis::Insert {
                            at: loc,
                            len: l,
                            attribute,
                        };
                        loc += l;
                        ret
                    }).collect();

                    if !any_changed {
                        return;
                    }

                    mapped_subrange.insert(0, RangedBasis::Delete {
                        at: range.start,
                        len: range.len(),
                    });

                    self.char_intrinsic_attribute.apply(RangedAttributeAction {
                        actions: mapped_subrange
                    }, s);

                    self.debug_assertions(s);
                }

                pub fn modify_char_derived(&self, range: Range<usize>, mut f: impl FnMut(D::CharAttribute) -> D::CharAttribute, s: Slock<impl ThreadMarker>) {
                    let cda = self.char_derived_attribute.borrow(s);
                    let subrange = cda.subrange(range.clone()).attributes;

                    let mut loc = range.start;
                    let mut any_changed = false;
                    let mut mapped_subrange: Vec<_> = subrange.into_iter().map(|(a, l)| {
                        let attribute = f(a);
                        // purposefully short circuit
                        any_changed = any_changed || attribute != *cda.attribute_at(loc);
                        let ret = RangedBasis::Insert {
                            at: loc,
                            len: l,
                            attribute,
                        };
                        loc += l;
                        ret
                    }).collect();

                    if !any_changed {
                        return;
                    }

                    mapped_subrange.insert(0, RangedBasis::Delete {
                        at: range.start,
                        len: range.len(),
                    });
                    history_elide(|| {
                        self.char_derived_attribute.apply(RangedAttributeAction {
                            actions: mapped_subrange
                        }, s);
                    });

                    self.debug_assertions(s);
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

        mod runs {
            // The type that stores a list of runs
            pub type RunsContainer<T> = Vec<T>;
        }

        mod cursor_state {
            use crate::core::Slock;
            use crate::state::{Binding, Buffer, GeneralListener, InverseListener, Signal, Store, StoreContainer};
            use crate::state::SetAction::Set;
            use crate::util::marker::ThreadMarker;
            use crate::view::undo_manager::UndoBucket;

            pub struct CursorState {
                start_run: Store<usize>,
                start_char: Store<usize>,

                end_run: Store<usize>,
                end_char: Store<usize>,
            }

            impl StoreContainer for CursorState {
                fn subtree_general_listener<F: GeneralListener + Clone>(&self, f: F, s: Slock<impl ThreadMarker>) {
                    self.start_run.subtree_general_listener(f.clone(), s);
                    self.start_char.subtree_general_listener(f.clone(), s);
                    self.end_run.subtree_general_listener(f.clone(), s);
                    self.end_char.subtree_general_listener(f.clone(), s);
                }

                fn subtree_inverse_listener<F: InverseListener + Clone>(&self, _f: F, _s: Slock<impl ThreadMarker>) {

                }

                fn subtree_undo_bucket(&self, _bucket: UndoBucket, _s: Slock<impl ThreadMarker>) {

                }
            }

            impl CursorState {
                pub fn new() -> Self {
                    CursorState {
                        start_run: Store::default(),
                        start_char: Store::default(),
                        end_run: Store::default(),
                        end_char: Store::default(),
                    }
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

                // Function is called with start_run, start_char, end_run, end_char
                pub fn listen(
                    &self,
                    f: impl FnMut(usize, usize, usize, usize, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    let stores = [&self.start_run, &self.start_char, &self.end_run, &self.end_char];
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
                            state.0 = (state.1)(args[0], args[1], args[2], args[3], s);

                            state.0
                        }, s)
                    }
                }
            }
        }

        mod page {
            use std::ops::{Deref, Range};
            use std::sync::atomic::{AtomicI32, Ordering};

            use quarve_derive::StoreContainer;

            use crate::core::{MSlock, Slock};
            use crate::state::{Bindable, Binding, DerivedStore, Filterless, SetAction, Signal, Stateful, Store, StoreContainerSource, StoreContainerView, UndoBarrier, VecActionBasis, Word};
            use crate::state::SetAction::Set;
            use crate::util::geo::ScreenUnit;
            use crate::util::marker::{FalseMarker, ThreadMarker};
            use crate::util::rust_util::DerefMap;
            use crate::view::text::CursorState;
            use crate::view::text::text_view::state::{AttributeSet, Run, RunsContainer};
            use crate::view::text::text_view::state::attribute_holder::AttributeHolder;
            use crate::view::undo_manager::history_hook;

            static PAGE_ID_COUNTER: AtomicI32 = AtomicI32::new(1);

            #[derive(Copy, Clone)]
            pub(crate) struct PageGUIInfo {
                // maintained by textviewvp
                pub added_text_view_listener: bool,
                pub page_num: usize,
                pub start_y_pos: ScreenUnit,
                // maintained by individual pages (ignores insets)
                pub content_height: ScreenUnit
            }

            impl Stateful for PageGUIInfo {
                type Action = SetAction<PageGUIInfo>;
                type HasInnerStores = FalseMarker;
            }

            pub(crate) trait PageFrontCallback {
                fn replace_utf16_range(&self, range: Range<usize>, with: String, _s: MSlock);
                fn set_cursor_selection(&self, range: Range<usize>, _s: MSlock);
            }

            #[derive(StoreContainer)]
            pub struct Page<I, D> where I: AttributeSet, D: AttributeSet {
                #[quarve(ignore)]
                pub(crate) id: i32,
                pub(crate) gui_info: DerivedStore<PageGUIInfo>,

                #[quarve(ignore)]
                pub(crate) selected_page: Option<<Store<Option<i32>> as Bindable<Filterless<Option<i32>>>>::Binding>,
                pub(crate) cursor: StoreContainerSource<CursorState>,
                pub(crate) runs: Store<Vec<Run<I, D>>>,
                pub(crate) page_intrinsic_attribute: Store<AttributeHolder<I::PageAttribute>>,
                pub(crate) page_derived_attribute: DerivedStore<AttributeHolder<D::PageAttribute>>
            }

            impl<I, D> Page<I, D> where I: AttributeSet, D: AttributeSet {
                pub fn new(s: Slock<impl ThreadMarker>) -> Self {
                    Page {
                        id: PAGE_ID_COUNTER.fetch_add(1, Ordering::SeqCst),
                        gui_info: DerivedStore::new(PageGUIInfo {
                            added_text_view_listener: false,
                            page_num: 0,
                            start_y_pos: 0.0,
                            content_height: 0.0,
                        }),
                        selected_page: None,
                        cursor: StoreContainerSource::new(CursorState::new()),
                        runs: Store::new(vec![
                            Run::new(s)
                        ]),
                        page_intrinsic_attribute: Store::default(),
                        page_derived_attribute: DerivedStore::default(),
                    }
                }

                pub fn selection(&self) -> &CursorState {
                    &self.cursor
                }

                pub fn num_runs(&self, s: Slock<impl ThreadMarker>) -> usize {
                    self.runs.borrow(s).len()
                }

                pub fn runs<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=RunsContainer<Run<I, D>>> + 'a {
                    self.runs.borrow(s)
                }

                pub fn run<'a>(&'a self, index: usize, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=Run<I, D>> + 'a {
                    DerefMap::new(
                        self.runs.borrow(s),
                        move |runs| &runs[index]
                    )
                }

                fn with_hook(&self, start_end: (usize, usize), end_end: (usize, usize), transaction: impl FnOnce()) {
                    if let Some(page) = self.selected_page.clone() {
                        let id = self.id;
                        let cursor = self.cursor.view();
                        let cursor2 = self.cursor.view();
                        let page2 = page.clone();

                        history_hook(
                            move |s| {
                                page.apply(Set(Some(id)), s);
                                cursor.set_range(
                                    end_end.0, end_end.1,
                                    end_end.0, end_end.1, s
                                );
                            },
                            transaction,
                            move |s| {
                                page2.apply(Set(Some(id)), s);
                                cursor2.set_range(
                                    start_end.0, start_end.1,
                                    start_end.0, start_end.1, s
                                );
                            },
                        )
                    }
                    else {
                        transaction()
                    }
                }

                fn insert_run_helper(&self, at: usize, s: MSlock) {
                    self.runs.apply(VecActionBasis::Insert(Run::new(s), at), s);
                }

                /// Due to a race condition, modification of text contents
                /// may only be performed on the main thread
                pub fn insert_run<'a>(&'a self, at: usize, s: MSlock<'a>) -> impl Deref<Target=Run<I, D>> + 'a {
                    // hook
                    let org = if at == 0 { (0, 0) } else {
                        (at - 1, self.run(at - 1, s).len(s))
                    };

                    self.runs.undo_barrier(UndoBarrier::Weak, s);
                    self.with_hook(
                        org,
                        (at, 0),
                        || {
                            self.insert_run_helper(at, s);
                        });
                    self.runs.undo_barrier(UndoBarrier::Weak, s);

                    self.run(at, s)
                }

                fn remove_run_helper(&self, at: usize, s: MSlock) {
                    assert!(self.runs.borrow(s).len() > 1);
                    self.runs.apply(VecActionBasis::Remove(at), s);
                }

                pub fn remove_run(&self, at: usize, s: MSlock) {
                    // hook
                    let start = (at, self.run(at, s).len(s));
                    let next = if at == 0 { (0, 0) } else {
                        (at - 1, self.run(at - 1, s).len(s))
                    };

                    self.runs.undo_barrier(UndoBarrier::Weak, s);
                    self.with_hook(
                        start,
                        next,
                        || {
                            self.remove_run_helper(at, s);
                        });
                    self.runs.undo_barrier(UndoBarrier::Weak, s);
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

                /// Due to a race condition, modification of text contents
                /// may only be performed on the main thread
                pub fn replace_range(
                    &self,
                    start_run: usize, start_char: usize,
                    end_run: usize, end_char: usize,
                    with: impl Into<String>,
                    s: MSlock
                ) {
                    assert!(end_run >= start_run || (start_run == end_run && end_char >= end_run));

                    let with = with.into();
                    let segments: Vec<_> = with
                        .split('\n')
                        .collect();

                    let own_undo_group = end_run > start_run || segments.len() > 1;

                    // hook
                    let end = if segments.len() == 1 {
                        (start_run, start_char + with.len())
                    } else {
                        (start_run + segments.len() - 1, segments[segments.len() - 1].len())
                    };

                    self.with_hook(
                        (end_run, end_char),
                        end,
                        move || {
                        if own_undo_group {
                            self.runs.undo_barrier(UndoBarrier::Weak, s);
                        }

                        // strategy:
                        // 1) delete tips + intermediate runs

                        if start_run == end_run {
                            let range = start_char..end_char;
                            self.run(start_run, s)
                                .replace(range, "", s);
                        } else {
                            let start = self.run(start_run, s);
                            start.replace(start_char..start.len(s), "", s);

                            self.run(end_run, s)
                                .replace(0..end_char, "", s);
                        }

                        if end_run > start_run + 1 {
                            self.runs.apply(VecActionBasis::RemoveMany(start_run + 1..end_run), s);
                        }

                        // 2) if there was only one line and there's multiple segments, split this one run
                        if start_run == end_run && segments.len() > 1 {
                            let next = self.run(start_run, s).split_trail(start_char, s);
                            self.runs.apply(VecActionBasis::Insert(next, start_run + 1), s);
                        }

                        // 3) if there were multiple lines and there's only one segment, merge the first and last
                        if start_run < end_run && segments.len() == 1 {
                            {
                                let curr = self.run(start_run, s);
                                let next = self.run(start_run + 1, s);
                                curr.merge_from(next.deref(), s);
                            }
                            self.remove_run_helper(start_run + 1, s);
                        }

                        // 4) handle insertion end points
                        if segments.len() == 1 {
                            self.run(start_run, s)
                                .replace(start_char..start_char, segments[0], s);
                        } else {
                            self.run(start_run, s)
                                .replace(start_char..start_char, segments[0], s);

                            self.run(start_run + 1, s)
                                .replace(0..0, segments[segments.len() - 1], s);
                        }

                        // 5) handle intermediate runs relatively normally
                        let intermediate_runs: Vec<Run<I, D>> = segments[1..(segments.len() - 1).max(1)].iter()
                            .map(|seg| {
                                let run = Run::new(s);
                                run.replace(0..0, *seg, s);
                                run
                            })
                            .collect();
                        if !intermediate_runs.is_empty() {
                            self.runs.apply(VecActionBasis::InsertMany(intermediate_runs, start_run + 1), s);
                        }

                        if own_undo_group {
                            self.runs.undo_barrier(UndoBarrier::Weak, s);
                        }
                    });
                }

                pub fn build_full_content(&self, s: Slock<impl ThreadMarker>) -> String {
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
                    f: impl FnMut(&AttributeHolder<I::PageAttribute>, &SetAction<AttributeHolder<I::PageAttribute>>, Slock) -> bool + Send + 'static,
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

            impl<I, D> StoreContainerView<Page<I, D>>
                where I: AttributeSet, D: AttributeSet
            {
                pub(crate) fn utf16_to_position(&self, mut utf16_pos: usize, s: Slock) -> (usize, usize) {
                    let len = self.runs.borrow(s).len();
                    for (i, run) in self.runs.borrow(s).iter().enumerate() {
                        let cu = run.gui_info.borrow(s).codeunits;
                        if utf16_pos <= cu {
                            let mut utf16_count = 0;
                            if utf16_count == utf16_pos {
                                // empty string edge case
                                return (i, 0)
                            }

                            for (byte_idx, ch) in run.content(s).char_indices() {
                                let ch_utf16_len = ch.len_utf16();
                                if utf16_count + ch_utf16_len > utf16_pos {
                                    return (i, byte_idx);
                                }
                                utf16_count += ch_utf16_len;
                                if utf16_count == utf16_pos {
                                    return (i, byte_idx + ch.len_utf8());
                                }
                            }

                            unreachable!("bad utf16")
                        }

                        if i == len - 1 {
                            return (i, utf16_pos)
                        }
                        else {
                            // dont forget new line
                            utf16_pos -= cu + 1;
                        }
                    }

                    unreachable!("Should always have at least one run")
                }

                pub(crate) fn position_to_utf16(&self, line: usize, char: usize, s: Slock) -> usize {
                    let mut utf16 = 0;
                    for (i, run) in self.runs(s).iter().take(line + 1).enumerate() {
                        if i == line {
                            utf16 += run.content(s)[0..char.min(run.content(s).len())]
                                .encode_utf16().count()
                        }
                        else {
                            // include new line
                            utf16 += run.gui_info.borrow(s).codeunits + 1;
                        }
                    }

                    utf16
                }
            }

            impl<I, D> PageFrontCallback for StoreContainerView<Page<I, D>>
                where I: AttributeSet, D: AttributeSet
            {
                fn replace_utf16_range(&self, range: Range<usize>, with: String, s: MSlock) {
                    let start = self.utf16_to_position(range.start, s.to_general_slock());
                    let end = self.utf16_to_position(range.end, s.to_general_slock());
                    self.replace_range(start.0, start.1, end.0, end.1, with, s);
                }

                fn set_cursor_selection(&self, range: Range<usize>, s: MSlock) {
                    let start = self.utf16_to_position(range.start, s.to_general_slock());
                    let end = self.utf16_to_position(range.end, s.to_general_slock());
                    self.selection()
                        .set_range(
                            start.0, start.1,
                            end.0, end.1, s
                        );
                }
            }
        }

        mod text_view_state {
            use std::ops::Deref;
            use std::sync::atomic::{AtomicBool, Ordering};
            use quarve_derive::StoreContainer;

            use crate::core::{MSlock, Slock};
            use crate::state::{Bindable, Binding, SetAction, Signal, Store, StoreContainerSource, UndoBarrier, VecActionBasis, Word};
            use crate::util::marker::ThreadMarker;
            use crate::util::rust_util::DerefMap;
            use crate::view::text::text_view::state::{AttributeSet, Page};

            #[derive(StoreContainer)]
            pub struct TextViewState<I, D> where I: AttributeSet, D: AttributeSet {
                pub(crate) pages: Store<Vec<StoreContainerSource<Page<I, D>>>>,

                #[quarve(ignore)]
                pub(crate) currently_mounted: AtomicBool,
                // i32 so it matches the focus
                // maybe change this in the future? usize is better...
                // explicitly excluded from undo history
                #[quarve(ignore)]
                pub(crate) selected_page: Store<Option<i32>>
            }

            impl<I, D> TextViewState<I, D> where I: AttributeSet, D: AttributeSet {
                pub fn new() -> Self {
                    TextViewState {
                        pages: Store::new(vec![]),
                        currently_mounted: AtomicBool::new(false),
                        selected_page: Store::new(None)
                    }
                }

                pub(crate) fn mount(&self) {
                    assert!(!self.currently_mounted.load(Ordering::SeqCst), "Currently, a TextViewState can only be associated with a single TextView in its life");
                    self.currently_mounted.store(true, Ordering::SeqCst);
                }

                pub fn pages<'a>(&'a self, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=Vec<StoreContainerSource<Page<I, D>>>> + 'a {
                    self.pages.borrow(s)
                }

                pub fn page<'a>(&'a self, at: usize, s: Slock<'a, impl ThreadMarker>) -> impl Deref<Target=Page<I, D>> + 'a {
                    DerefMap::new(
                        self.pages.borrow(s),
                        move |p| p[at].deref()
                    )
                }

                pub fn insert_page(&self, mut page: Page<I, D>, at: usize, s: Slock<impl ThreadMarker>) {
                    self.pages.undo_barrier(UndoBarrier::Weak, s);
                    page.selected_page = Some(self.selected_page.binding());
                    self.pages.apply(
                        VecActionBasis::Insert(StoreContainerSource::new(page), at), s
                    );
                    self.pages.undo_barrier(UndoBarrier::Weak, s);
                }

                pub fn remove_page(&self, at: usize, s: Slock<impl ThreadMarker>) {
                    self.pages.undo_barrier(UndoBarrier::Weak, s);
                    self.pages.apply(
                        VecActionBasis::Remove(at), s
                    );
                    self.pages.undo_barrier(UndoBarrier::Weak, s);
                }

                /// To avoid race conditions, any text modification must be done on main thread
                pub fn replace_selection(&self, with: impl Into<String>, s: MSlock){
                    let Some(page_num) = self.selected_page(s) else {
                        return;
                    };
                    let page = self.page(page_num as usize, s);
                    let cursor = page.selection();
                    page.replace_range(
                        cursor.start_run(s),
                        cursor.start_char(s),
                        cursor.end_run(s),
                        cursor.end_char(s),
                        with, s
                    );
                }

                pub fn selected_page(&self, s: Slock<impl ThreadMarker>) -> Option<i32> {
                    let id = *self.selected_page.borrow(s);
                    id.and_then(|id| {
                         self.pages.borrow(s).iter()
                             .position(|page| page.id == id)
                             .map(|i| i as i32)
                    })
                }

                pub fn set_selected_page(&self, page_num: Option<i32>, s: Slock<impl ThreadMarker>) {
                    let id =
                        page_num.and_then(|p| {
                            let pages = self.pages.borrow(s);
                            if p >= 0 && p < pages.len() as i32 {
                                Some(pages[p as usize].id)
                            }
                            else {
                                None
                            }
                        });

                    self.selected_page.apply(SetAction::Set(id), s);
                }

                pub fn action_listen(
                    &self,
                    f: impl FnMut(&Vec<StoreContainerSource<Page<I, D>>>, &Word<VecActionBasis<StoreContainerSource<Page<I, D>>>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.pages.action_listen(f, s);
                }

                pub fn listen(
                    &self,
                    f: impl FnMut(&Vec<StoreContainerSource<Page<I, D>>>, Slock) -> bool + Send + 'static,
                    s: Slock<impl ThreadMarker>
                ) {
                    self.pages.listen(f, s);
                }
            }
        }
    }

    mod text_view {
        use std::cell::Cell;
        use std::ffi::c_void;
        use std::marker::PhantomData;
        use std::mem::take;
        use std::ops::{Deref, Range};
        use std::sync::Arc;

        use crate::core::{Environment, MSlock, run_main_async, Slock, StandardConstEnv};
        use crate::event::{Event, EventPayload, EventResult, MouseEvent};
        use crate::native;
        use crate::native::view::text_view::{text_view_begin_editing, text_view_copy, text_view_cut, text_view_end_editing, text_view_focus, text_view_full_replace, text_view_get_line_height, text_view_get_selection, text_view_init, text_view_paste, text_view_select_all, text_view_set_char_attributes, text_view_set_font, text_view_set_page_id, text_view_set_run_attributes, text_view_set_selection, text_view_unfocus};
        use crate::resource::Resource;
        use crate::state::{ActualDiffSignal, Bindable, Binding, Buffer, Filterless, Signal, Store, StoreContainer, StoreContainerView, StringActionBasis, VecActionBasis, WeakBinding, Word};
        use crate::state::SetAction::Set;
        use crate::state::slock_cell::MainSlockCell;
        use crate::util::geo;
        use crate::util::geo::{Inset, Rect, ScreenUnit, Size};
        use crate::view::{EnvRef, IntoViewProvider, NativeView, NativeViewState, Subtree, TrivialContextViewRef, View, ViewProvider, ViewRef, WeakInvalidator};
        use crate::view::layout::{BindingVMap, LayoutProvider, VStackOptions};
        use crate::view::menu::MenuChannel;
        use crate::view::text::{AttributeSet, Page, Run, TextViewState, ToCharAttribute, ToRunAttribute};

        thread_local! {
            pub(crate) static IN_TEXTVIEW_FRONT_CALLBACK: Cell<bool> = Cell::new(false)
        }

        pub trait TextViewProvider<E> : 'static where E: Environment {
            type IntrinsicAttribute: AttributeSet;
            type DerivedAttribute: AttributeSet;

            const PAGE_INSET: Inset;

            fn font() -> Option<Resource> {
                None
            }

            fn font_size() -> ScreenUnit {
                14.0
            }

            fn init(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock);

            #[allow(unused_variables)]
            fn tab(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
                false
            }

            #[allow(unused_variables)]
            fn untab(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
                false
            }

            #[allow(unused_variables)]
            fn newline(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
                false
            }

            #[allow(unused_variables)]
            fn alt_newline(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
                false
            }

            #[allow(unused_variables)]
            fn escape(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
                false
            }

            #[allow(unused_variables)]
            fn left_arrow(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
                false
            }

            #[allow(unused_variables)]
            fn right_arrow(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
                false
            }

            #[allow(unused_variables)]
            fn down_arrow(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
                if page.cursor.end_run(s) == page.runs.borrow(s).len() - 1 {
                    let curr = state.selected_page(s);
                    if curr.is_some_and(|c| c != state.pages.borrow(s).len() as i32 - 1) {
                        state.set_selected_page(Some(curr.unwrap() + 1), s);
                        return true;
                    }
                }

                false
            }

            #[allow(unused_variables)]
            fn up_arrow(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
                if page.cursor.end_run(s) == 0 {
                    let curr = state.selected_page(s);
                    if curr.is_some_and(|c| c != 0) {
                        state.set_selected_page(Some(curr.unwrap() - 1), s);
                        return true;
                    }
                }

                false
            }

            fn run_decoration(
                &self,
                number: impl Signal<Target=usize>,
                run: &Run<Self::IntrinsicAttribute, Self::DerivedAttribute>,
                s: MSlock
            ) -> impl IntoViewProvider<E, DownContext=(), UpContext=()> + 'static;

            fn page_background(
                &self, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock
            ) -> impl IntoViewProvider<E, DownContext=()> + 'static;
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
            pub fn new(state: StoreContainerView<TextViewState<P::IntrinsicAttribute, P::DerivedAttribute>>, provider: P) -> Self {
                state.mount();

                TextView {
                    provider,
                    state,
                }
            }
        }

        impl<E, P> IntoViewProvider<E> for TextView<E, P>
            where E: Environment,
                  E::Const: AsRef<StandardConstEnv>,
                  P: TextViewProvider<E> {
            type UpContext = ();
            type DownContext = ();

            fn into_view_provider(mut self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
                // merge them in undo history
                self.state.group_undos(s);

                self.provider.init(self.state.deref(), s);
                let shared_provider = Arc::new(MainSlockCell::new_main(self.provider, s));

                let y = Store::new(0.0);
                let h = Store::new(0.0);

                let sp = shared_provider.clone();
                let yb = y.binding();
                let hb = h.binding();
                let state_clone = self.state.clone();
                let pages = self.state.pages.binding().binding_vmap_options(move |p, s| {
                    new_page_coordinator(sp.clone(), state_clone.clone(), p.view(), yb.clone(), hb.clone(), s)
                }, VStackOptions::default().spacing(0.0)).into_view_provider(env, s).into_view(s);

                TextViewVP {
                    provider: shared_provider,
                    state: self.state,
                    scroll_y: y,
                    height: h,
                    scroll_view: 0 as *mut c_void,
                    pages,
                }
            }
        }

        struct TextViewVP<E, P, VP>
            where P: TextViewProvider<E>,
                  VP: ViewProvider<E, DownContext=()>,
                  E: Environment
        {
            provider: Arc<MainSlockCell<P>>,
            state: StoreContainerView<TextViewState<P::IntrinsicAttribute, P::DerivedAttribute>>,
            scroll_y: Store<ScreenUnit>,
            height: Store<ScreenUnit>,
            scroll_view: *mut c_void,
            pages: View<E, VP>,
        }

        impl<E, P, VP> ViewProvider<E> for TextViewVP<E, P, VP>
            where E: Environment,
                  E::Const: AsRef<StandardConstEnv>,
                  P: TextViewProvider<E>,
                  VP: ViewProvider<E, DownContext=()>
        {
            type UpContext = ();
            type DownContext = ();

            fn intrinsic_size(&mut self, _s: MSlock) -> Size {
                Size::new(0.0, 0.0)
            }

            fn xsquished_size(&mut self, _s: MSlock) -> Size {
                Size::new(0.0, 0.0)
            }

            fn xstretched_size(&mut self, _s: MSlock) -> Size {
                Size::new(geo::UNBOUNDED, geo::UNBOUNDED)
            }

            fn ysquished_size(&mut self, _s: MSlock) -> Size {
                Size::new(0.0, 0.0)
            }

            fn ystretched_size(&mut self, _s: MSlock) -> Size {
                Size::new(geo::UNBOUNDED, geo::UNBOUNDED)
            }

            fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
                ()
            }

            fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
                let state = Buffer::new(NativeViewState::default());

                let mut nv = {
                    if let Some((nv, bs)) = backing_source {
                        self.pages.take_backing(bs.pages, env, s);
                        nv
                    }
                    else {
                        unsafe {
                            NativeView::new(native::view::scroll::init_scroll_view(true, false, self.scroll_y.binding(), Store::new(0.0).binding(), s), s)
                        }
                    }
                };
                nv.set_clips_subviews();
                self.scroll_view = nv.backing();

                let weak_y = state.downgrade();
                self.scroll_y.diff_listen(move |y, s| {
                    let Some(strong) = weak_y.upgrade() else {
                        return false;
                    };

                    strong.borrow_mut(s).offset_y = *y;
                    true
                }, s);

                let handle_page = |page: &Page<P::IntrinsicAttribute, P::DerivedAttribute>, inv: WeakInvalidator<E>, s: Slock| {
                    let mut curr = *page.gui_info.borrow(s);
                    if !curr.added_text_view_listener {
                        curr.added_text_view_listener = true;
                        page.gui_info.apply(Set(curr), s);

                        let mut last_height = 0.0;
                        page.gui_info.listen(move |gui, s| {
                            let Some(invalidator) = inv.upgrade() else {
                                return false;
                            };
                            if gui.content_height != last_height {
                                last_height = gui.content_height;
                                invalidator.invalidate(s);
                            }
                            true
                        }, s);
                    }
                };
                self.state.pages.borrow(s).iter().for_each(|p| handle_page(&*p, invalidator.clone(), s.to_general_slock()));

                self.state.pages.action_listen(move |_, a, s| {
                    let Some(inv) = invalidator.upgrade() else {
                        return false;
                    };
                    inv.invalidate(s);

                    for action in a.iter() {
                        match action {
                            VecActionBasis::Insert(page, _) => {
                                handle_page(&*page, invalidator.clone(), s);
                            }
                            VecActionBasis::InsertMany(pages, _) => {
                                pages.iter().for_each(|page| handle_page(&*page, invalidator.clone(), s));
                            }
                            _ => { }
                        }
                    }

                    true
                }, s);

                subtree.push_subview(&self.pages, env, s);
                nv.set_state(state);
                nv
            }

            fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock) -> bool {
                // adjust page numbers and positions
                let pages = self.state.pages.borrow(s);
                let mut y_pos = 0.0;
                for (i, p) in pages.iter().enumerate() {
                    let mut gui = *p.gui_info.borrow(s);
                    gui.start_y_pos = y_pos;
                    gui.page_num = i;
                    y_pos += gui.content_height + P::PAGE_INSET.t + P::PAGE_INSET.b;
                    p.gui_info.apply(Set(gui), s);
                }
                true
            }

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
                let w = frame.w;
                let h = geo::UNBOUNDED;

                self.height.apply(Set(frame.h), s);

                let unbounded = Rect::new(0.0, 0.0, w, h);

                self.pages.layout_down_with_context(unbounded, layout_context, env, s);
                (frame.full_rect(), frame.full_rect())
            }
        }

        // all components of a page
        struct PageCoordinator<E, B, P, D>
            where E: Environment,
                  E::Const: AsRef<StandardConstEnv>,
                  B: IntoViewProvider<E, DownContext=()>,
                  P: TextViewProvider<E>,
                  D: IntoViewProvider<E, DownContext=()>
        {
            provider: Arc<MainSlockCell<P>>,
            background: B,
            page_view: PageIVP<E, P>,
            decorations: D,
            phantom: PhantomData<E>
        }

        fn new_page_coordinator<E, P>(
            provider: Arc<MainSlockCell<P>>,
            full_state: StoreContainerView<TextViewState<P::IntrinsicAttribute, P::DerivedAttribute>>,
            page: StoreContainerView<Page<P::IntrinsicAttribute, P::DerivedAttribute>>,
            y: impl Binding<Filterless<ScreenUnit>>,
            height: impl Binding<Filterless<ScreenUnit>>,
            s: MSlock
        ) -> PageCoordinator<E, impl IntoViewProvider<E, DownContext=()>, P, impl IntoViewProvider<E, DownContext=()>>
            where E: Environment,
                  E::Const: AsRef<StandardConstEnv>,
                  P: TextViewProvider<E>
        {
            fn _background<E, P>(page: &'static Page<P::IntrinsicAttribute, P::DerivedAttribute>, s: MSlock<'static>, p: &'static P)
                                 -> impl IntoViewProvider<E, DownContext=()> + 'static
                where E: Environment, P: TextViewProvider<E> {
                p.page_background(page, s)
            }

            fn _run_decoration<E, P>(run: &'static Run<P::IntrinsicAttribute, P::DerivedAttribute>, s: MSlock<'static>, p: &'static P)
                                     -> impl IntoViewProvider<E, DownContext=(), UpContext=()> + 'static
                where E: Environment, P: TextViewProvider<E> {
                let line_number =
                    run.gui_info.map(|g| g.line, s);

                p.run_decoration(line_number, run, s)
            }

            let (background, decorations) = {
                let provider_clone = provider.clone();
                let provider = provider.borrow_main(s);

                // safety: see below (basically since .background is static in general, it cant borrow from anything)
                let (static_provider, long_s, long_page):
                    (&'static P, MSlock<'static>, &'static Page<P::IntrinsicAttribute, P::DerivedAttribute>)
                    = unsafe {
                    std::mem::transmute((provider.deref(), s, page.deref()))
                };

                let background = _background(long_page, long_s, static_provider);
                let decorations = RunDecorationsIVP {
                    page: page.clone(),
                    scroll_y: y,
                    scroll_h: height,
                    map: move |run, s| {
                        // TODO dont like this unsafe
                        // safety:
                        // we require that .run_decoration gives a static reference
                        // so that it cannot borrow from
                        // (see layout.rs _into_view_provider for detailed proof)
                        let provider_borrow = provider_clone.borrow_main(s);
                        let (static_provider, long_s, long_run):
                            (&'static P, MSlock<'static>, &'static Run<P::IntrinsicAttribute, P::DerivedAttribute>)
                            = unsafe {
                            std::mem::transmute((provider_borrow.deref(), s, run))
                        };

                        _run_decoration(long_run, long_s, static_provider)
                    },
                    env: Default::default(),
                    phantom: PhantomData::<P>
                };

                (background, decorations)
            };

            PageCoordinator {
                provider: provider.clone(),
                background,
                page_view: PageIVP {
                    full_state,
                    page,
                    provider,
                },
                decorations,
                phantom: Default::default(),
            }
        }

        impl<E, B, P, D> IntoViewProvider<E> for PageCoordinator<E, B, P, D>
            where E: Environment,
                  E::Const: AsRef<StandardConstEnv>,
                  B: IntoViewProvider<E, DownContext=()>,
                  P: TextViewProvider<E>,
                  D: IntoViewProvider<E, DownContext=()>
        {
            type UpContext = ();
            type DownContext = ();

            fn into_view_provider(self, env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
                let lp = PageCoordinatorLP {
                    provider: self.provider,
                    background: self.background.into_view_provider(env, s).into_view(s),
                    page_view: self.page_view.into_view_provider(env, s).into_view(s),
                    decorations: self.decorations.into_view_provider(env, s).into_view(s),
                    phantom: PhantomData
                };

                lp.into_layout_view_provider()
            }
        }

        struct PageCoordinatorLP<E, B, P, D, V>
            where E: Environment,
                  B: ViewProvider<E, DownContext=()>,
                  P: TextViewProvider<E>,
                  D: ViewProvider<E, DownContext=()>,
                  V: ViewProvider<E, DownContext=()>
        {
            provider: Arc<MainSlockCell<P>>,
            background: View<E, B>,
            page_view: View<E, V>,
            decorations: View<E, D>,
            phantom: PhantomData<E>
        }

        impl<E, B, P, D, V> LayoutProvider<E> for PageCoordinatorLP<E, B, P, D, V>
            where E: Environment,
                  B: ViewProvider<E, DownContext=()>,
                  P: TextViewProvider<E>,
                  D: ViewProvider<E, DownContext=()>,
                  V: ViewProvider<E, DownContext=()>
        {
            type DownContext = ();
            type UpContext = ();

            fn intrinsic_size(&mut self, s: MSlock) -> Size {
                self.page_view.intrinsic_size(s)
            }

            fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
                ()
            }

            fn init(&mut self, _invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, source_provider: Option<Self>, env: &mut EnvRef<E>, s: MSlock) {
                if let Some(other) = source_provider {
                    self.background.take_backing(other.background, env, s);
                    self.page_view.take_backing(other.page_view, env, s);
                    self.decorations.take_backing(other.decorations, env, s);
                }

                subtree.push_subview(&self.background, env, s);
                subtree.push_subview(&self.page_view, env, s);
                subtree.push_subview(&self.decorations, env, s);
            }

            fn layout_up(&mut self, _subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, _s: MSlock) -> bool {
                false
            }

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _layout_context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> Rect {
                let inset = P::PAGE_INSET;

                let page = self.page_view.layout_down(
                    Rect::new(inset.l, inset.t, frame.w - (inset.l + inset.r).min(frame.w), frame.h - (inset.b + inset.t).min(frame.h)),
                    env, s
                );
                self.background.layout_down(page, env, s);
                self.decorations.layout_down(page, env, s);

                Rect::new(
                    page.x - inset.l,
                    page.y - inset.t,
                    page.w + inset.l + inset.r,
                    page.h + inset.b + inset.t
                )
            }
        }

        struct RunDecorationsIVP<E, P, Y, H, V, M>
            where E: Environment, P: TextViewProvider<E>,
                  Y: Binding<Filterless<ScreenUnit>>,
                  H: Binding<Filterless<ScreenUnit>>,
                  V: IntoViewProvider<E, DownContext=()>,
                  M: FnMut(&Run<P::IntrinsicAttribute, P::DerivedAttribute>, MSlock) -> V + Send + 'static,
        {
            page: StoreContainerView<Page<P::IntrinsicAttribute, P::DerivedAttribute>>,
            scroll_y: Y,
            scroll_h: H,
            map: M,
            env: PhantomData<(E, fn() -> V)>,
            phantom: PhantomData<P>
        }

        impl<E, P, Y, H, V, M> IntoViewProvider<E> for RunDecorationsIVP<E, P, Y, H, V, M>
            where E: Environment, P: TextViewProvider<E>,
                  Y: Binding<Filterless<ScreenUnit>>,
                  H: Binding<Filterless<ScreenUnit>>,
                  V: IntoViewProvider<E, DownContext=()>,
                  M: FnMut(&Run<P::IntrinsicAttribute, P::DerivedAttribute>, MSlock) -> V + Send + 'static
        {
            type UpContext = ();
            type DownContext = ();

            fn into_view_provider(self, _env: &E::Const, s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
                RunDecorations {
                    page: self.page,
                    scroll_y: self.scroll_y,
                    scroll_h: self.scroll_h,
                    map: Some(self.map),
                    buffered_views: Arc::new(MainSlockCell::new_main(Word::default(), s)),
                    views: vec![],
                    view_displayed: vec![],
                    to_vp: |ivp, e, s| {
                        // rust is worried that it will borrow from e/s
                        // but that's clearly impossible if it's static
                        // todo dont like this unsafe either
                        let (static_e, static_s): (&'static E::Const, MSlock<'static>) = unsafe {
                            std::mem::transmute((e, s))
                        };
                        ivp.into_view_provider(static_e, static_s)
                    },
                    phantom: PhantomData::<P>
                }
            }
        }

        struct RunDecorations<E, P, Y, H, V, VP, M, F>
            where E: Environment, P: TextViewProvider<E>,
                  Y: Binding<Filterless<ScreenUnit>>,
                  H: Binding<Filterless<ScreenUnit>>,
                  V: IntoViewProvider<E, DownContext=()>,
                  VP: ViewProvider<E, DownContext=()>,
                  M: FnMut(&Run<P::IntrinsicAttribute, P::DerivedAttribute>, MSlock) -> V + Send + 'static,
                  F: Fn(V, &E::Const, MSlock) -> VP + Send + 'static
        {
            page: StoreContainerView<Page<P::IntrinsicAttribute, P::DerivedAttribute>>,
            scroll_y: Y,
            scroll_h: H,
            map: Option<M>,
            buffered_views: Arc<MainSlockCell<Word<VecActionBasis<V>>>>,
            views: Vec<View<E, VP>>,
            // for any view index, whether or not it is actually
            // displayed
            view_displayed: Vec<bool>,
            to_vp: F,
            phantom: PhantomData<P>
        }

        impl<E, P, Y, H, V, VP, M, F> ViewProvider<E> for RunDecorations<E, P, Y, H, V, VP, M, F>
            where E: Environment, P: TextViewProvider<E>,
                  Y: Binding<Filterless<ScreenUnit>>,
                  H: Binding<Filterless<ScreenUnit>>,
                  V: IntoViewProvider<E, DownContext=()>,
                  VP: ViewProvider<E, DownContext=()>,
                  M: FnMut(&Run<P::IntrinsicAttribute, P::DerivedAttribute>, MSlock) -> V + Send + 'static,
                  F: Fn(V, &E::Const, MSlock) -> VP + Send + 'static
        {
            type UpContext = ();
            type DownContext = ();

            // sizes aren't used by page coordinator
            fn intrinsic_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn xsquished_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn xstretched_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn ysquished_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn ystretched_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
                ()
            }

            fn init_backing(&mut self, invalidator: WeakInvalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
                let inv = invalidator.clone();
                let mut map = self.map.take().unwrap();

                // initial runs
                self.views = self.page.runs.borrow(s)
                    .iter()
                    .map(|r| {
                        (self.to_vp)(map(r, s), env.const_env(), s).into_view(s)
                    })
                    .collect();
                self.view_displayed = vec![false; self.views.len()];

                // run listeners
                let buffered_views = self.buffered_views.clone();
                let handle_run = |run: &Run<P::IntrinsicAttribute, P::DerivedAttribute>, inv: WeakInvalidator<E>, s: MSlock| {
                    let mut curr = *run.gui_info.borrow(s);
                    if !curr.added_decoration_listener {
                        curr.added_decoration_listener = true;
                        run.gui_info.apply(Set(curr), s);

                        let mut last_height = 0.0;
                        run.gui_info.listen(move |gui, s| {
                            let Some(invalidator) = inv.upgrade() else {
                                return false;
                            };
                            if gui.page_height != last_height {
                                last_height = gui.page_height;
                                invalidator.invalidate(s);
                            }
                            true
                        }, s);
                    }
                };
                self.page.runs.borrow(s).iter().for_each(|r| handle_run(r, invalidator.clone(), s));

                self.page.runs
                    .action_listen(move |_r, a, s| {
                        let Some(invalidator) = inv.upgrade() else {
                            return false;
                        };

                        let s = s.try_to_main_slock().unwrap();

                        for action in a.iter() {
                            match action {
                                VecActionBasis::Insert(run, _) => {
                                    handle_run(run, inv.clone(), s);
                                }
                                VecActionBasis::InsertMany(runs, _) => {
                                    runs.iter().for_each(|run| handle_run(run, inv.clone(), s));
                                }
                                _ => { }
                            }
                        }

                        let mut all_actions = buffered_views.borrow_mut_main(s);
                        let curr = std::mem::replace(&mut *all_actions, Word::new(vec![]));
                        *all_actions = Word::new(
                            a.iter().map(|action| {
                                match action {
                                    VecActionBasis::Insert(run, at) => {
                                        VecActionBasis::Insert(map(run, s), *at)
                                    }
                                    VecActionBasis::Remove(at) => VecActionBasis::Remove(*at),
                                    VecActionBasis::InsertMany(runs, at) => {
                                        VecActionBasis::InsertMany(runs.iter().map(|run| {
                                            map(run, s)
                                        }).collect(), *at)
                                    }
                                    VecActionBasis::RemoveMany(range) => VecActionBasis::RemoveMany(range.clone()),
                                    _ => unreachable!()
                                }
                            }).rev().collect()
                        ) * curr;

                        invalidator.invalidate(s);

                        true
                    }, s);

                // invalidate on changed position
                let inv = invalidator.clone();
                self.scroll_y.diff_listen(move |_, s| {
                    inv.try_upgrade_invalidate(s)
                }, s);

                // invalidate when this page position changes
                let inv = invalidator.clone();
                self.page.gui_info.map(move |gui| (gui.start_y_pos, gui.content_height), s).diff_listen(move |_, s| {
                    inv.try_upgrade_invalidate(s)
                }, s);

                // invalidate whenever the scroll view becomes larger
                let inv = invalidator.clone();
                self.scroll_h.diff_listen(move |_, s| {
                    inv.try_upgrade_invalidate(s)
                }, s);

                if let Some((native, provider)) = backing_source {
                    for (dst, src) in std::iter::zip(self.views.iter(), provider.views.into_iter()) {
                        dst.take_backing(src, env, s);
                    }

                    native
                }
                else {
                    NativeView::layout_view(s)
                }
            }

            fn layout_up(&mut self, subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
                let action = {
                    let mut b = self.buffered_views.borrow_mut_main(s);
                    take(&mut *b)
                };

                let mut updated = false;

                // add new views and in some cases remove old ones
                for a in action.into_iter() {
                    updated = true;

                    let handle_insertion = |views: &mut Vec<View<E, VP>>, view_displayed: & mut Vec<bool>, view, env: &E::Const, at| {
                        views.insert(at, (self.to_vp)(view, env, s).into_view(s));
                        view_displayed.insert(at, false);
                    };

                    let mut handle_removal = |views: &mut Vec<View<E, VP>>, view_displayed: &mut Vec<bool>, env: &mut EnvRef<E>, at| {
                        if view_displayed[at] {
                            subtree.remove_subview(&views[at], env, s);
                        }

                        views.remove(at);
                        view_displayed.remove(at);
                    };

                    match a {
                        VecActionBasis::Insert(view, at) => {
                            handle_insertion(&mut self.views, &mut self.view_displayed, view, env.const_env(), at)
                        },
                        VecActionBasis::Remove(at) => {
                            handle_removal(&mut self.views, &mut self.view_displayed, env, at)
                        },
                        VecActionBasis::InsertMany(views, at) => {
                            for (i, view) in views.into_iter().enumerate() {
                                handle_insertion(&mut self.views, &mut self.view_displayed, view, env.const_env(), i + at);
                            }
                        }
                        VecActionBasis::RemoveMany(range) => range.rev().for_each(|at| handle_removal(&mut self.views, &mut self.view_displayed, env, at)),
                        VecActionBasis::Swap(_, _) => unreachable!()
                    }
                }


                let scroll_y = *self.scroll_y.borrow(s);
                let mut effective_y_pos = self.page.gui_info.borrow(s).start_y_pos - scroll_y;
                let want_range = 0.0 .. *self.scroll_h.borrow(s);
                for (view, (run, is_visible)) in self.views.iter().zip(self.page.runs.borrow(s).iter().zip(self.view_displayed.iter_mut())) {
                    let height = run.gui_info.borrow(s).page_height;
                    let now_visible = effective_y_pos < want_range.end &&  (effective_y_pos + height) > want_range.start;

                    if *is_visible && !now_visible {
                        // remove (slightly inefficient full scan)
                        subtree.remove_subview(view, env, s);
                        updated = true;
                    }
                    else if !*is_visible && now_visible {
                        // add
                        subtree.push_subview(view, env, s);
                        updated = true;
                    }

                    *is_visible = now_visible;
                    effective_y_pos += height;
                }

                updated
            }

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _context: &Self::DownContext, env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
                // run gui filled by PageVP
                let runs = &self.page.runs;

                let mut y = 0.0;
                for ((run, view), displayed) in runs.borrow(s).iter().zip(self.views.iter()).zip(self.view_displayed.iter()) {
                    let h = run.gui_info.borrow(s).page_height;
                    if *displayed {
                        view.layout_down(Rect::new(0.0, y, frame.w, h), env, s);
                    }
                    y += h;
                }

                (frame.full_rect(), frame.full_rect())
            }
        }

        // just the text
        struct PageIVP<E, P>
            where P: TextViewProvider<E>,
                  E: Environment,
                  E::Const: AsRef<StandardConstEnv>
        {
            full_state: StoreContainerView<TextViewState<P::IntrinsicAttribute, P::DerivedAttribute>>,
            page: StoreContainerView<Page<P::IntrinsicAttribute, P::DerivedAttribute>>,
            provider: Arc<MainSlockCell<P>>,
        }

        impl<E, P> IntoViewProvider<E> for PageIVP<E, P>
            where E: Environment, P: TextViewProvider<E>,
                  E::Const: AsRef<StandardConstEnv>
        {
            type UpContext = ();
            type DownContext = ();

            fn into_view_provider(self, env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
                let env = env.as_ref();
                PageVP {
                    full_state: self.full_state,
                    page: self.page,
                    provider: self.provider,
                    text_view: 0 as *mut c_void,
                    last_up_width: -1.0,
                    last_size: Default::default(),
                    invalidator: None,
                    select_all_menu: env.channels.select_all_menu.clone(),
                    cut_menu: env.channels.cut_menu.clone(),
                    copy_menu: env.channels.copy_menu.clone(),
                    paste_menu: env.channels.paste_menu.clone(),
                }
            }
        }

        struct PageVP<E, P> where P: TextViewProvider<E>, E: Environment {
            full_state: StoreContainerView<TextViewState<P::IntrinsicAttribute, P::DerivedAttribute>>,
            page: StoreContainerView<Page<P::IntrinsicAttribute, P::DerivedAttribute>>,
            provider: Arc<MainSlockCell<P>>,
            text_view: *mut c_void,

            last_up_width: ScreenUnit,
            last_size: Size,
            invalidator: Option<WeakInvalidator<E>>,

            select_all_menu: MenuChannel,
            cut_menu: MenuChannel,
            copy_menu: MenuChannel,
            paste_menu: MenuChannel,
        }

        impl<E, P> ViewProvider<E> for PageVP<E, P>
            where E: Environment, P: TextViewProvider<E>
        {
            type UpContext = ();
            type DownContext = ();

            fn intrinsic_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn xsquished_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn xstretched_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn ysquished_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn ystretched_size(&mut self, _s: MSlock) -> Size {
                Size::default()
            }

            fn up_context(&mut self, _s: MSlock) -> Self::UpContext {
                ()
            }

            fn init_backing(&mut self, invalidator: WeakInvalidator<E>, _subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, _env: &mut EnvRef<E>, s: MSlock) -> NativeView {
                let backing =
                    if let Some((bs, _)) = backing_source {
                        text_view_full_replace(bs.backing(), &self.page.build_full_content(s), self.full_state.clone(),
                                               self.page.clone(), self.full_state.selected_page.binding(), self.provider.clone(), s);
                        text_view_set_font(bs.backing(), P::font(), P::font_size(), s);
                        bs.backing()
                    }
                    else {
                        let backing = text_view_init(s);
                        text_view_full_replace(backing, &self.page.build_full_content(s), self.full_state.clone(),
                                               self.page.clone(), self.full_state.selected_page.binding(), self.provider.clone(),
                                               s);
                        text_view_set_font(backing, P::font(), P::font_size(), s);
                        backing
                    };
                text_view_set_page_id(backing, self.page.id, s);
                let backing_id = backing as usize;

                // mark all lines as dirty initially so we can update lines
                for run in self.page.runs.borrow(s).iter() {
                    let mut gui = *run.gui_info.borrow(s);
                    gui.dirty = true;
                    run.gui_info.apply(Set(gui), s);
                }

                // cursor changes
                let inv = invalidator.clone();
                self.full_state.selected_page.diff_listen(move |_l, s| {
                    inv.try_upgrade_invalidate(s)
                }, s);

                let inv = invalidator.clone();
                self.page.cursor.listen(move |_, _, _, _, s| {
                    inv.try_upgrade_invalidate(s)
                }, s);

                self.run_listen(invalidator.clone(), s, backing_id);
                self.invalidator = Some(invalidator);

                self.text_view = backing;
                unsafe {
                    NativeView::new(backing, s)
                }
            }

            fn layout_up(&mut self, subtree: &mut Subtree<E>, _env: &mut EnvRef<E>, s: MSlock) -> bool {
                // focus
                let token = self.page.id;

                let view = Arc::downgrade(subtree.owner());
                if *self.full_state.selected_page.borrow(s) == Some(token) {
                    subtree.window().and_then(|w| w.upgrade()).unwrap()
                        .borrow_main(s)
                        .request_focus(view);
                }
                else {
                    subtree.window().and_then(|w| w.upgrade()).unwrap()
                        .borrow_main(s)
                        .unrequest_focus(view);
                }


                let lines = self.page.runs.borrow(s).len();
                // relay attrs of affected lines
                {
                    let mut utf16_chars = 0;
                    text_view_begin_editing(self.text_view, s);
                    for (i, run) in self.page.runs.borrow(s).iter().enumerate() {
                        let gui = *run.gui_info.borrow(s);
                        let next_code_units = utf16_chars + gui.codeunits + if i < lines - 1 { 1 } else { 0 };

                        if gui.dirty {
                            self.assign_attributes(utf16_chars..next_code_units, i, run, s);
                        }

                        utf16_chars = next_code_units;
                    }
                    text_view_end_editing(self.text_view, s);
                }

                // rewrite line numbers
                // and recalculate line heights
                {
                    let mut utf16_chars = 0;
                    let mut total_height = 0.0;
                    for (i, run) in self.page.runs.borrow(s).iter().enumerate() {
                        let mut gui = *run.gui_info.borrow(s);
                        let next_code_units = utf16_chars + gui.codeunits + if i < lines - 1 { 1 } else { 0 };

                        if gui.dirty || self.last_up_width != self.last_size.w {
                            // calculating line height
                            gui.page_height = text_view_get_line_height(self.text_view, i, utf16_chars..utf16_chars + gui.codeunits, self.last_size.w, s);
                        }

                        // only send flush if necessary
                        if gui.line != i || gui.dirty {
                            gui.dirty = false;
                            gui.line = i;
                            run.gui_info.apply(Set(gui), s);
                        }

                        total_height += gui.page_height;
                        utf16_chars = next_code_units;
                    }

                    let mut gui = *self.page.gui_info.borrow(s);
                    gui.content_height = total_height;
                    self.page.gui_info.apply(Set(gui), s);

                    self.last_up_width = self.last_size.w;
                }

                // relay cursor information + number (easy)
                let start = self.page.position_to_utf16(
                    self.page.cursor.start_run(s),
                    self.page.cursor.start_char(s), s.to_general_slock()
                );
                let end = self.page.position_to_utf16(
                    self.page.cursor.end_run(s),
                    self.page.cursor.end_char(s), s.to_general_slock()
                );
                text_view_set_selection(self.text_view, start..end, s);

                let selection = text_view_get_selection(self.text_view);
                let (sl, sc) = self.page.utf16_to_position(selection.start, s.to_general_slock());
                let (el, ec) = self.page.utf16_to_position(selection.end, s.to_general_slock());
                self.page.cursor.set_range(sl, sc, el, ec, s);

                true
            }

            fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
                let total_height = self.page.gui_info.borrow(s).content_height;
                let rect = Rect::new(0.0, 0.0, frame.w, total_height);

                // very ugly, but cold branch anyways
                // note that we can't invalidate right now
                // as that's a double borrow. Might find way to relax this in future
                if (rect.w - self.last_size.w).abs() > 1e-3 {
                    let inv = self.invalidator.clone().unwrap();
                    run_main_async(move |s| {
                        let Some(invalidator) = inv.upgrade() else {
                            return;
                        };
                        invalidator.invalidate(s);
                    });
                }
                self.last_size = rect.size();

                (rect, rect)
            }

            fn focused(&self, _rel_depth: u32, s: MSlock) {
                let token = self.page.id;
                if *self.full_state.selected_page.borrow(s) != Some(token) {
                    self.full_state.selected_page.apply(Set(Some(token)), s);
                }

                text_view_focus(self.text_view, s);

                let backing = self.text_view;
                self.select_all_menu.set(Box::new(move |s| {
                    text_view_select_all(backing, s);
                }), None, s);
                self.cut_menu.set(Box::new(move |s| {
                    text_view_cut(backing, s);
                }), None, s);
                self.copy_menu.set(Box::new(move |s| {
                    text_view_copy(backing, s);
                }), None, s);
                self.paste_menu.set(Box::new(move |s| {
                    text_view_paste(backing, s);
                }), None, s);
            }

            fn unfocused(&self, _rel_depth: u32, s: MSlock) {
                let token = self.page.id;
                if *self.full_state.selected_page.borrow(s) == Some(token) {
                    self.full_state.selected_page.apply(Set(None), s);
                }

                text_view_unfocus(self.text_view, s);

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
                    EventResult::Handled
                }
            }
        }

        impl<E, P> PageVP<E, P> where E: Environment, P: TextViewProvider<E> {
            fn assign_attributes(&self, utf16_char_range: Range<usize>, line: usize, run: &Run<P::IntrinsicAttribute, P::DerivedAttribute>, s: MSlock) {
                // adjust line attributes
                let run_intrinsic = run.intrinsic(s);
                let run_derived = run.derived(s);
                text_view_set_run_attributes(self.text_view, line, utf16_char_range.clone(),
                                             run_intrinsic.to_run_attribute().as_ref(),
                                             run_derived.to_run_attribute().as_ref(), s);

                let string = run.content(s);
                if string.is_empty() {
                    return;
                }

                let derived = run.char_derived_attribute.borrow(s);
                let intrinsic = run.char_intrinsic_attribute.borrow(s);

                let derived_chars = &derived.attributes;
                let intrinsic_chars = &intrinsic.attributes;

                assert_eq!(derived_chars.iter().fold(0usize,|u,v|u + v.1), intrinsic_chars.iter().fold(0usize,|u,v|u + v.1));

                let mut i = 0;
                let mut j = 0;

                let mut current_derived = derived_chars[i].0.to_char_attribute();
                let mut current_intrinsic = intrinsic_chars[j].0.to_char_attribute();

                let mut d_utf8_offset = 0;
                let mut i_utf8_offset = 0;
                let mut utf16_offset = utf16_char_range.start;

                while i < derived_chars.len() || j < intrinsic_chars.len() {
                    // can't have it so only one is at the end
                    debug_assert!(i < derived_chars.len() && j < intrinsic_chars.len());

                    let next_utf8 = (d_utf8_offset + derived_chars[i].1).min(i_utf8_offset + intrinsic_chars[j].1);
                    let utf16_len = string[d_utf8_offset.max(i_utf8_offset)..next_utf8].encode_utf16().count();

                    text_view_set_char_attributes(self.text_view, utf16_offset..utf16_offset + utf16_len, current_intrinsic.as_ref(), current_derived.as_ref(), s);

                    // advance i and j
                    if next_utf8 == d_utf8_offset + derived_chars[i].1 {
                        d_utf8_offset = next_utf8;
                        i += 1;
                        if i < derived_chars.len() {
                            current_derived = derived_chars[i].0.to_char_attribute();
                        }
                    }

                    if next_utf8 == i_utf8_offset + intrinsic_chars[j].1 {
                        i_utf8_offset = next_utf8;
                        j += 1;
                        if j < intrinsic_chars.len() {
                            current_intrinsic = intrinsic_chars[j].0.to_char_attribute();
                        }
                    }

                    utf16_offset += utf16_len;
                }
            }

            fn run_listen(&mut self, invalidator: WeakInvalidator<E>, s: MSlock, backing_id: usize) {
                let weak_runs = self.page.runs.weak_binding();
                let weak_inv = invalidator.clone();
                let handle_run = move |run: &Run<P::IntrinsicAttribute, P::DerivedAttribute>, s: Slock| {
                    let mut curr = *run.gui_info.borrow(s);
                    curr.dirty = true;

                    if !curr.added_vp_listener {
                        curr.added_vp_listener = true;

                        // range updating
                        let runs = weak_runs.clone();
                        let id = run.gui_info.address();
                        run.content_action_listen(move |c, a, s| {
                            let runs = runs.upgrade().unwrap();
                            let mut pos = 0;
                            let mut found = false;
                            let len = runs.borrow(s).len();
                            for (i, run) in runs.borrow(s).iter().enumerate() {
                                if run.gui_info.address() == id {
                                    found = true;
                                    break
                                }

                                // notice that action listen finishes before the
                                // codeunits are updated, but we're looking
                                // at previous run codeunits which are valid
                                pos += run.gui_info.borrow(s).codeunits;
                                // new line character
                                if i != len - 1 {
                                    pos += 1;
                                }
                            }

                            if !found {
                                return false;
                            }

                            if !IN_TEXTVIEW_FRONT_CALLBACK.get() {
                                // must be called on main thread
                                let mslock = s.try_to_main_slock().unwrap();
                                for StringActionBasis::ReplaceSubrange(act, with) in a.iter() {
                                    let exact_pos = pos + c.0[..act.start].encode_utf16().count();
                                    let end_pos = exact_pos + c.0[act.start..act.end].encode_utf16().count();
                                    native::view::text_view::text_view_replace(backing_id as *mut c_void, exact_pos..end_pos, &with, mslock);
                                }
                            }

                            true
                        }, s);
                        // make sure to set it before we add
                        // the listener so we don't invalidate instantly
                        run.gui_info.apply(Set(curr), s);

                        let invalidator_copy = weak_inv.clone();
                        run.gui_info.listen(move |gui, s| {
                            let Some(invalidator) = invalidator_copy.upgrade() else {
                                return false;
                            };
                            if gui.dirty {
                                invalidator.invalidate(s);
                            }
                            true
                        }, s);
                    } else {
                        run.gui_info.apply(Set(curr), s);
                    }
                };

                for run in self.page.runs.borrow(s).iter() {
                    handle_run(run, s.to_general_slock())
                }

                // invalidate whenever a run is inserted or deleted
                let weak_inv = invalidator;
                self.page.runs.action_listen(move |r, a, s| {
                    let Some(inv) = weak_inv.upgrade() else {
                        return false;
                    };
                    // invalidate whenever a particular run is edited
                    for action in a.iter() {
                        let replaced_range: Range<usize>;
                        let mut with = "".to_string();

                        match action {
                            VecActionBasis::Insert(run, at) => {
                                handle_run(run, s);

                                let mut pos = 0;
                                for i in 0..*at {
                                    pos += r[i].gui_info.borrow(s).codeunits;
                                    // include newline
                                    if i != r.len() - 1 {
                                        pos += 1
                                    }
                                }

                                replaced_range = pos..pos;
                                with = run.content(s).deref().to_string() + "\n";
                                // rotate
                                if *at == r.len() {
                                    if *at == 0 {
                                        with.remove(with.len() - 1);
                                    } else {
                                        with = "\n".to_owned() + &with[..with.len() - 1]
                                    }
                                }
                            }
                            VecActionBasis::InsertMany(runs, at) => {
                                runs.iter().for_each(|r| handle_run(r, s));

                                let mut pos = 0;
                                for i in 0..*at {
                                    pos += r[i].gui_info.borrow(s).codeunits;
                                    // include newline
                                    if i != r.len() - 1 {
                                        pos += 1
                                    }
                                }

                                replaced_range = pos..pos;
                                with = runs
                                    .iter()
                                    .map(|r| r.content(s))
                                    .fold("".to_string(), |a, b| a + &b + "\n");

                                // if last, flip position of "\n"
                                if *at == r.len() {
                                    if *at == 0 {
                                        with.remove(with.len() - 1);
                                    } else {
                                        with = "\n".to_owned() + &with[..with.len() - 1]
                                    }
                                }
                            }
                            VecActionBasis::Remove(at) => {
                                let mut pos = 0;
                                for i in 0..*at {
                                    pos += r[i].gui_info.borrow(s).codeunits;
                                    // include newline
                                    if i != r.len() - 1 {
                                        pos += 1
                                    }
                                }

                                let end = pos + r[*at].gui_info.borrow(s).codeunits + 1;
                                // if last, shift
                                if *at == r.len() - 1 {
                                    if *at == 0 {
                                        replaced_range = pos..(end - 1);
                                    } else {
                                        replaced_range = (pos - 1)..(end - 1);
                                    }
                                } else {
                                    replaced_range = pos..end;
                                };
                            }
                            VecActionBasis::RemoveMany(range) => {
                                let mut pos = 0;
                                for i in 0..range.start {
                                    pos += r[i].gui_info.borrow(s).codeunits;
                                    // include newline
                                    if i != r.len() - 1 {
                                        pos += 1
                                    }
                                }

                                let mut end = pos;
                                for i in range.clone() {
                                    end += r[i].gui_info.borrow(s).codeunits;
                                    // include newline
                                    if i != r.len() - 1 {
                                        end += 1
                                    }
                                }

                                if range.end == r.len() {
                                    if range.start == 0 {
                                        replaced_range = 0..end
                                    } else {
                                        replaced_range = pos - 1..end
                                    }
                                } else {
                                    replaced_range = pos..end;
                                }
                            }
                            VecActionBasis::Swap(_, _) => {
                                // we dont use swaps
                                unreachable!()
                            }
                        }

                        if !IN_TEXTVIEW_FRONT_CALLBACK.get() {
                            let mslock = s.try_to_main_slock().unwrap();
                            native::view::text_view::text_view_replace(backing_id as *mut c_void, replaced_range, &with, mslock);
                        }
                    }
                    inv.invalidate(s);
                    true
                }, s);
            }
        }
    }
}

mod env {
    use std::ops::Deref;
    use std::path::Path;

    use crate::core::{Environment, MSlock, StandardVarEnv, TextEnv};
    use crate::resource::Resource;
    use crate::util::geo::ScreenUnit;
    use crate::view::{IntoViewProvider, WeakInvalidator};
    use crate::view::modifers::{EnvironmentModifier, EnvModifiable, EnvModifierIVP};
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
