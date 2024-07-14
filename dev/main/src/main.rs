use quarve::core::{Application, Environment, launch, MSlock};
use quarve::state::{Binding, Filterless, FixedSignal, Signal, Stateful, Store};
use quarve::util::geo::{Alignment, HorizontalAlignment, Size};
use quarve::view::{ViewProvider, IntoViewProvider, WeakInvalidator};
use quarve::view::control::{Button, Dropdown};
use quarve::view::layout::*;
use quarve::view::modifers::{Cursor, CursorModifiable, EnvironmentModifier, Frame, FrameModifiable, KeyListener, Layer, LayerModifiable, OffsetModifiable, PaddingModifiable, WhenModifiable};
use quarve::view::scroll::ScrollView;
use quarve::view::util::Color;
use quarve_derive::StoreContainer;

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
        let offset_y = Store::new(0.0);
        let selected = Store::new(None);

        let v1 = ScrollView::vertical(
            VStack::hetero_options(VStackOptions::default().align(HorizontalAlignment::Leading))
                .push(
                    Button::new_with_label(
                        Color::black()
                            .intrinsic(100, 100),
                        |_| println!("Clicked")
                    )
                        .offset_signal(FixedSignal::new(0.0), offset_y.signal())
                )
                .push(
                    Dropdown::new(selected.binding())
                        .option("Hello")
                        .option("World")
                )
                .push(
                    Dropdown::new(selected.binding())
                        .option("Hello")
                        .option("World")
                )
                .push(
                    FixedSignal::new((0..14).collect())
                        .sig_vmap(|x, s| {
                            Color::white()
                                .intrinsic(100, 100 + 10 * *x)
                                .cursor(Cursor::Pointer)
                        })
                        .padding(10)
                        .border(Color::black(), 1)
                )
        )
            .hoist_y_offset(offset_y.binding())
            .frame(Frame::default()
                .intrinsic(300, 300)
                    .unlimited_stretch()
                    .align(Alignment::Center)
            );

        let v2 = ScrollView::vertical(
                VStack::hetero_options(VStackOptions::default().align(HorizontalAlignment::Leading))
                    .push(
                        Button::new_with_label(
                            Color::white()
                                .intrinsic(100, 100),
                            |_| println!("Clicked")
                        )
                            .offset_signal(FixedSignal::new(0.0), offset_y.signal())
                    )
                    .push(
                        FixedSignal::new((0..14).collect())
                            .sig_vmap(|x, s| {
                                Color::black()
                                    .intrinsic(100, 100 + 10 * *x)
                                    .cursor(Cursor::Pointer)
                            })
                            .padding(10)
                            .border(Color::white(), 1)
                    )
            )
                .hoist_y_offset(offset_y.binding())
                .frame(Frame::default()
                    .intrinsic(300, 300)
                    .unlimited_stretch()
                    .align(Alignment::Center)
                );

        VStack::hetero()
            .push(v1)
            .push(v2)
            .into_view_provider(env, s)
    }

    fn size(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> (Size, Size, Size) {
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
    fn init(&mut self, invalidator: WeakInvalidator<Env>, s: MSlock) {

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
