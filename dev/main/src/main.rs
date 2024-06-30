use quarve::core::{Application, Environment, launch, MSlock};
use quarve::state::{FixedSignal, GeneralSignal, Signal};
use quarve::util::geo::{Alignment, Size, VerticalDirection};
use quarve::view::{ViewProvider, IntoViewProvider, Invalidator};
use quarve::view::conditional::{view_if, ViewElseIf};
use quarve::view::image_view::ImageView;
use quarve::view::layout::*;
use quarve::view::modifers::{EnvironmentModifier, Frame, FrameModifiable, Layer, LayerModifiable, WhenModifiable};
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

    fn title(&self, _s: MSlock) -> impl Signal<Target=String> {
        // s.clock_signal()
        //     .map(|time| format!("Time {}", time), s)
        FixedSignal::new("Hello".to_owned())
    }

    fn style(&self, _s: MSlock) {

    }

    fn root(&self, env: &<Env as Environment>::Const, s: MSlock) -> impl ViewProvider<Env, DownContext=()> {
        let clock = s.clock_signal();
        let count: GeneralSignal<Vec<_>> = clock
            .map(|val| {
                (0 ..((val / 10.0).sin().abs() * 15.0) as usize)
                    .collect()
            }, s);
        let count_clone = count.clone();

        VStack::hetero_options(VStackOptions::default().direction(VerticalDirection::Up))
            .push(
                view_if(count.clone().map(|val| val.len() % 2 == 0, s), Color::black())
                    .view_else(Color::white())
                    .intrinsic(200, 100)
            )
            .push(
                view_match!(
                    count.clone().map(|val| val.len(), s);
                    1 => Color::black(),
                    _ => Color::white()
                )
                    .intrinsic(150, 100)
            )
            .push(
                ImageView::named("rose.png")
                    .frame(Frame::default().unlimited_stretch())
                    .layer(Layer::default().radius(20.0).border(Color::black(), 1.0))
            )
            .frame(
                Frame::default()
                    .unlimited_stretch()
                    .align(Alignment::Center)
            )
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

    fn size(&self) -> (Size, Size, Size) {
        (
            Size::new(400.0, 400.0),
            Size::new(400.0, 400.0),
            Size::new(800.0, 1000.0)
        )
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
