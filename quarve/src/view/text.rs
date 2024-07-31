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
            (frame.full_rect(), frame.full_rect())
        }
    }
}
pub use text::*;

mod text_field {
    use std::ffi::c_void;
    use crate::core::{Environment, MSlock, StandardVarEnv};
    use crate::native::view::text_field::{text_field_init, text_field_size, text_field_update};
    use crate::state::{Binding, Filterless, Signal, TokenStore};
    use crate::util::geo;
    use crate::util::geo::{Rect, Size};
    use crate::view::{EnvRef, IntoViewProvider, NativeView, Subtree, ViewProvider, WeakInvalidator};

    pub struct TextField<B> where B: Binding<Filterless<String>> + Clone {
        text: B,
        max_lines: u32
    }

    struct TextFieldVP<B> where B: Binding<Filterless<String>> + Clone {
        text: B,
        max_lines: u32,
        size: Size,
        backing: *mut c_void,
    }

    impl<B> TextField<B> where B: Binding<Filterless<String>> + Clone {
        pub fn new(binding: B) -> Self {
            TextField {
                text: binding,
                max_lines: 1
            }
        }

        pub fn max_lines(mut self, max_lines: u32) -> Self {
            self.max_lines = max_lines;
            self
        }
    }

    impl<E, B> IntoViewProvider<E> for TextField<B>
        where E: Environment,
              E::Variable: AsRef<StandardVarEnv>,
              B: Binding<Filterless<String>> + Clone {
        type UpContext = ();
        type DownContext = ();

        fn into_view_provider(self, _env: &E::Const, _s: MSlock) -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext> {
            TextFieldVP {
                text: self.text,
                max_lines: self.max_lines,
                size: Size::default(),
                backing: 0 as *mut c_void,
            }
        }
    }

    impl<E, B> ViewProvider<E> for TextFieldVP<B>
        where E: Environment,
              E::Variable: AsRef<StandardVarEnv>,
              B: Binding<Filterless<String>> + Clone {
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
                    let focused = TokenStore::new(None);
                    NativeView::new(text_field_init(self.text.clone(), focused.binding(), s), s)
                }
            };

            self.backing = nv.backing();
            nv
        }

        fn layout_up(&mut self, _subtree: &mut Subtree<E>, env: &mut EnvRef<E>, s: MSlock) -> bool {
            println!("Env {:?}", env.variable_env().as_ref());
            text_field_update(
                self.backing,
                &*self.text.borrow(s),
                self.max_lines,
                env.variable_env().as_ref(),
                s
            );
            self.size = text_field_size(self.backing, Size::new(geo::UNBOUNDED, geo::UNBOUNDED), s);
            true
        }

        fn layout_down(&mut self, _subtree: &Subtree<E>, frame: Size, _layout_context: &Self::DownContext, _env: &mut EnvRef<E>, s: MSlock) -> (Rect, Rect) {
            (frame.full_rect(), frame.full_rect())
        }
    }
}
pub use text_field::*;

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

            println!("Push");
        }

        fn pop_environment(&mut self, env: &mut E::Variable, _s: MSlock) {
            env.as_mut().text = self.last_env.take().unwrap();
            println!("Pop");
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
            text.font = Some(Some(Resource::new(path.deref())));
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

struct TextView {

}

struct TextViewState {

}

trait TextViewProvider {
    type IntrinsicAttribute;
    type DerivedAttributes;
}