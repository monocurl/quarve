use std::os::macos::raw::stat;
use quarve::core::clock_signal;
use quarve::prelude::*;
use quarve::view::layout::{FlexAlign, FlexContext, FlexStack, FlexStackOptions, FlexSubview, IteratorFlexMap, SignalFlexMap, VecLayoutProvider};
use quarve::vstack;

struct App;
struct MainWindow;
struct Env(StandardConstEnv, StandardVarEnv);

// mainly boilerplate
impl ApplicationProvider for App {
    // used for calculating certain paths
    fn name(&self) -> &str {
        "Quarve Example"
    }

    fn will_spawn(&self, app: &quarve::core::Application, s: MSlock) {
        // init code goes here
        app.spawn_window(MainWindow, s);
    }
}

// The main code where you specify a view hierarchy
impl WindowProvider for MainWindow {
    type Environment = Env;

    fn title(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> impl Signal<Target=String> {
        FixedSignal::new("Flexbox Demo".into())
    }

    fn size(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> (Size, Size, Size) {
        (
            // min
            Size::new(600.0, 400.0),
            // intrinsic
            Size::new(800.0, 800.0),
            // max
            Size::new(2400.0, 2000.0)
        )
    }

    fn root(&self, env: &<Env as Environment>::Const, s: MSlock) -> impl ViewProvider<Env, DownContext=()> {
        let static_flex = FlexStack::hetero_options(FlexStackOptions::default().gap(0.0))
            .push(
                text("first")
                    .frame(F.intrinsic(200, 30).squished(40, 30).unlimited_stretch())
                    .border(RED, 1)
                    .flex(FlexContext::default().grow(1.0))
            )
            .push(
                text("second block of text")
                    .frame(F.intrinsic(100, 30).unlimited_stretch())
                    .border(BLUE, 1)
                    .flex(FlexContext::default().grow(3.0))
            )
            .push(
                text("final")
                    .frame(F.intrinsic(300, 30).unlimited_stretch())
                    .border(BLACK, 1)
                    .flex(FlexContext::default().grow(0.5))
            );

        let clock = clock_signal(s);
        let signal = clock
            .map(|c| {
                let amount = (10.0 * c.sin() + 10.0) as i32;
                (0..amount)
                    .into_iter()
                    .collect::<Vec<i32>>()
            }, s);

        let dynamic_flex = signal.sig_flexmap_options(
            |i, _s| {
                BLUE.padding(10)
                    .layer(Layer::default().border(GREEN, 1).radius(3))
                    .intrinsic(100 + 10 * i, 20 + 5 * i)
            },
            FlexStackOptions::default()
                .align(FlexAlign::Start)
                .cross_gap(10.0)
                .wrap()
        ).intrinsic(800, 600);

        vstack()
            .push(static_flex)
            .push(dynamic_flex)
            .background(WHITE)
            .frame(F.unlimited_stretch())
            .into_view_provider(env, s)
    }

    fn menu(&self, env: &<Self::Environment as Environment>::Const, s: MSlock) -> WindowMenu {
        WindowMenu::standard(
            env,
            Menu::new("File"),
            Menu::new("Edit"),
            Menu::new("View"),
            Menu::new("Help"),
            s
        )
    }
}

// Boilerplate; mainly necessary for advanced projects
impl Environment for Env {
    type Const = StandardConstEnv;
    type Variable = StandardVarEnv;

    fn root_environment() -> Self {
        Env(StandardConstEnv::new(), StandardVarEnv::new())
    }

    fn const_env(&self) -> &Self::Const {
        &self.0
    }

    fn variable_env(&self) -> &Self::Variable { &self.1 }

    fn variable_env_mut(&mut self) -> &mut Self::Variable { &mut self.1 }
}


fn main() {
    quarve::core::launch(App);
}

