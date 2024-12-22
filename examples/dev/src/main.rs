use quarve::prelude::*;
use quarve::prelude::Alignment::Center;
use quarve::state::NumericAction;
use quarve::view::text::{Text, TextModifier};

struct App;
struct MainWindow;
struct Env(StandardConstEnv, StandardVarEnv);

// mainly boilerplate
impl ApplicationProvider for App {
    // used for calculating certain paths
    fn name(&self) -> &str {
        "Counter"
    }

    fn will_spawn(&self, app: &quarve::core::Application, s: MSlock) {
        // init code goes here
        app.spawn_window(MainWindow, s);
    }
}

fn counter(s: MSlock) -> impl IVP {
    // state
    let count = Store::new(0);
    let count_binding = count.binding();
    let count_label = count.map(|c| format!("Count: {:?}", c), s);

    // views
    let button = button("Increment", move |s| {
        count_binding.apply(NumericAction::Incr(1), s);
    });
    let display = Text::from_signal(count_label);

    vstack()
        .push(button.bold())
        .push(display)
        .text_color(BLACK)
        .frame(F.intrinsic(400, 400).align(Center).unlimited_stretch())
        .background(WHITE)
}

// The main code where you specify a view hierarchy
impl WindowProvider for MainWindow {
    type Environment = Env;

    fn title(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> impl Signal<Target=String> {
        FixedSignal::new("Counter".into())
    }

    fn size(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> (Size, Size, Size) {
        (
            // min
            Size::new(400.0, 400.0),
            // intrinsic
            Size::new(800.0, 800.0),
            // max
            Size::new(2400.0, 2000.0)
        )
    }

    fn root(&self, env: &<Env as Environment>::Const, s: MSlock) -> impl ViewProvider<Env, DownContext=()> {
        counter(s)
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

// boilerplate trait alias for IntoViewProvider<Env, UpContext=(), DownContext=()>
// which is referenced a lot but tedious to type
pub(crate) trait IVP: IntoViewProvider<Env, UpContext=(), DownContext=()> { }

impl<I> IVP for I where I: IntoViewProvider<Env, UpContext=(), DownContext=()> { }


fn main() {
    quarve::core::launch(App);
}

