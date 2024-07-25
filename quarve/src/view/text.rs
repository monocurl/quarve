// TODO

mod attribute {
    mod character {
        use crate::resource::Resource;
        use crate::util::geo::ScreenUnit;
        use crate::view::util::Color;
        pub enum CharacterAttribute {
            Bold,
            Italic,
            Underline,
            Strikethrough,
            BackColor(Color),
            ForeColor(Color),
            FontSize(ScreenUnit),
            Font(Option<Resource>),
        }
    }
    pub use character::*;

    mod run {
        use crate::util::geo::ScreenUnit;

        #[derive(Copy, Clone)]
        pub enum Justification {
            Leading,
            Center,
            Trailing
        }

        #[derive(Copy, Clone)]
        pub struct Indentation {
            leading: ScreenUnit,
            trailing: ScreenUnit
        }

        pub enum RunAttribute {
            Justification(Justification),
            Indentation(Indentation)
        }
    }
    pub use run::*;

    mod document {
        pub enum DocumentAttribute {

        }
    }
    pub use document::*;
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

        fn init_backing(&mut self, invalidator: WeakInvalidator<E>, subtree: &mut Subtree<E>, backing_source: Option<(NativeView, Self)>, env: &mut EnvRef<E>, s: MSlock) -> NativeView {
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

struct TextField {

}

struct TextView {

}

struct TextViewState {

}

trait TextViewProvider {
    type IntrinsicAttribute;
    type DerivedAttributes;
}