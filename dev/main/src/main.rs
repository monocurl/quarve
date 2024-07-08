use quarve::core::{Application, clock_signal, Environment, launch, MSlock};
use quarve::state::{Binding, Filterless, FixedSignal, GeneralSignal, Signal, Store};
use quarve::util::geo::{Alignment, Direction, Size, VerticalDirection};
use quarve::view::{ViewProvider, IntoViewProvider, Invalidator};
use quarve::view::color_view::EmptyView;
use quarve::view::conditional::{view_if, ViewElseIf};
use quarve::view::image_view::ImageView;
use quarve::view::layout::*;
use quarve::view::modifers::{Cursor, CursorModifiable, EnvironmentModifier, Frame, FrameModifiable, KeyListener, Layer, LayerModifiable, PaddingModifiable, WhenModifiable};
use quarve::view::scroll::ScrollView;
use quarve::view::util::Color;
use quarve::view::view_match::ViewMatchIVP;
use quarve::view_match;

struct Env(());

struct ApplicationProvider;

struct WindowProvider;

impl Environment for Env {
    type Const = ();
    type Variable = ();

    fn root_environment() -> Self {
        Env(())
    }

    fn const_env(&self) -> &Self::Const {
        &self.0
    }

    fn variable_env(&self) -> &Self::Variable {
        &self.0
    }

    fn variable_env_mut(&mut self) -> &mut Self::Variable {
        &mut self.0
    }
}

impl quarve::core::ApplicationProvider for ApplicationProvider {
    fn will_spawn(&self, app: &Application, s: MSlock) {
        app.spawn_window(WindowProvider, s);
    }
}

impl quarve::core::WindowProvider for WindowProvider {
    type Env = Env;

    fn title(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> impl Signal<Target=String> {
        // s.clock_signal()
        //     .map(|time| format!("Time {}", time), s)
        FixedSignal::new("Hello".to_owned())
    }

    fn root(&self, env: &<Env as Environment>::Const, s: MSlock) -> impl ViewProvider<Env, DownContext=()> {
        let clock = clock_signal(s);
        let count = clock
            .map(|val| {
                (0 ..((val).sin().abs() * 15.0) as usize)
                    .collect()
            }, s);
        count
            .sig_flexmap_options(
                |val, _| {
                    Color::new(100, 0, 0)
                        .frame(
                            Frame::default()
                                .intrinsic(100, 100.0 + 25.0 * (val % 2) as f64)
                                .stretched(150, 150.0)
                                .squished(50, 100.0 - 50.0 * (val % 2) as f64)
                        )
                        .flex(FlexContext::default()
                            .grow(0.5 + (val % 2) as f64)
                        )
                },
                FlexStackOptions::default()
                    .gap(10.0)
                    .wrap()
                    .cross_gap(10.0)
                    .direction(Direction::Up)
            )
            .layer(Layer::default().border(Color::black(), 1.0))
            .intrinsic(550, 500)
            .layer(Layer::default().border(Color::new(100, 100, 100), 1.0))
            .into_view_provider(env, s)
        //
        // FlexStack::hetero()
        //     .push(
        //         Color::new(100, 0, 0).intrinsic(400, 100)
        //             .flex(|f| f.grow(1.0))
        //     )
        //     .push(
        //         Color::new(0, 0, 100)
        //             .flex(|f| f.grow(1.0))
        //     )
        //     .push(
        //         Color::new(0, 100, 0).intrinsic(400, 100)
        //     )
        //     .into_view_provider(env, s)
    }

    fn size(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> (Size, Size, Size) {
        (
            Size::new(400.0, 400.0),
            Size::new(400.0, 400.0),
            Size::new(800.0, 1000.0)
        )
    }

    fn is_fullscreen(&self, env: &<Self::Env as Environment>::Const, s: MSlock) -> impl Binding<Filterless<bool>> {
        let ret = Store::new(false);
        ret.listen(|_, s| {
            true
        }, s);

        ret
    }
}

struct EnvModifier {

}

impl EnvironmentModifier<Env> for EnvModifier {
    fn init(&mut self, invalidator: Invalidator<Env>, s: MSlock) {

    }

    fn push_environment(&mut self, env: &mut <Env as Environment>::Variable, s: MSlock) {

    }

    fn pop_environment(&mut self, env: &mut <Env as Environment>::Variable, s: MSlock) {

    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
