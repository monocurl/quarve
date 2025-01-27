pub use inner_view::*;
pub use into_view_provider::*;
pub use view::*;
pub use view_provider::*;

mod inner_view;
mod view;

mod into_view_provider {
    use crate::core::{Environment, MSlock};
    use crate::state::Signal;
    use crate::util::geo::ScreenUnit;
    use crate::view::modifers::{Layer, LayerIVP, LayerModifiable, post_hide_wrap, post_show_wrap, pre_hide_wrap, pre_show_wrap};
    use crate::view::util::Color;
    use crate::view::ViewProvider;

    // it may seem like we will have to wait a while for
    // TAIT but in the meantime it's not so bad
    // since 99% of the time intoviewprovider is only called
    // from intoviewprovider methods, which means capturing
    // rules arent that bad. Otherwise, it's to elide
    // the capture rules anyways since ViewProvider references static data
    // (does require unsafe still though)
    pub trait IntoViewProvider<E: Environment>: 'static + Sized {
        type UpContext: 'static;
        type DownContext: 'static;

        fn into_view_provider(self, env: &E::Const, s: MSlock)
            -> impl ViewProvider<E, UpContext=Self::UpContext, DownContext=Self::DownContext>;

        #[cfg(all(target_os = "macos", not(feature="qt_backend")))]
        fn cfg_cocoa<P>(self, modifier: impl FnOnce(Self) -> P) -> P
            where P: IntoViewProvider<E>
        {
             modifier(self)
        }

        #[cfg(any(not(target_os = "macos"), feature="qt_backend"))]
        #[allow(unused_variables)]
        fn cfg_cocoa<P>(self, modifier: impl FnOnce(Self) -> P) -> Self
            where P: IntoViewProvider<E>
        {
            self
        }

        #[cfg(all(target_os = "macos", not(feature="qt_backend")))]
        #[allow(unused_variables)]
        fn cfg_qt<P>(self, modifier: impl FnOnce(Self) -> P) -> Self
            where P: IntoViewProvider<E>
        {
            self
        }

        #[cfg(any(not(target_os = "macos"), feature="qt_backend"))]
        fn cfg_qt<P>(self, modifier: impl FnOnce(Self) -> P) -> P
            where P: IntoViewProvider<E>
        {
            modifier(self)
        }

        // FIXME pre_show, layer methods, and related should be optimized for ShowHideIVP/LayerIVP
        fn pre_show(self, f: impl FnMut(MSlock) + 'static)
            -> impl IntoViewProvider<E, DownContext=Self::DownContext, UpContext=Self::UpContext> {
            pre_show_wrap(self, f)
        }

        fn post_show(self, f: impl FnMut(MSlock) + 'static)
            -> impl IntoViewProvider<E, DownContext=Self::DownContext, UpContext=Self::UpContext> {
            post_show_wrap(self, f)
        }

        fn pre_hide(self, f: impl FnMut(MSlock) + 'static)
            -> impl IntoViewProvider<E, DownContext=Self::DownContext, UpContext=Self::UpContext> {
            pre_hide_wrap(self, f)
        }

        fn post_hide(self, f: impl FnMut(MSlock) + 'static)
            -> impl IntoViewProvider<E, DownContext=Self::DownContext, UpContext=Self::UpContext> {
            post_hide_wrap(self, f)
        }

        fn border(self, color: Color, width: impl Into<ScreenUnit>)
            -> LayerIVP<E, Self, impl Signal<Target=Color>, impl Signal<Target=ScreenUnit>, impl Signal<Target=Color>, impl Signal<Target=ScreenUnit>, impl Signal<Target=f32>>
        {
            self.layer(Layer::default()
                .border(color, width)
            )
        }

        fn bg_color(self, color: Color)
            -> LayerIVP<E, Self, impl Signal<Target=Color>, impl Signal<Target=ScreenUnit>, impl Signal<Target=Color>, impl Signal<Target=ScreenUnit>, impl Signal<Target=f32>>
        {
            self.layer(Layer::default()
                .bg_color(color)
            )
        }
    }
}

mod view_provider;
pub mod layout;
pub mod modifers;
pub mod util;

pub mod color_view;
pub mod portal;
pub mod conditional;
pub mod image_view;
pub mod view_match;
pub mod text;
pub mod control;
pub mod scroll;
pub mod modal;
pub mod menu;
pub mod undo_manager;
pub mod functional_ivp;
