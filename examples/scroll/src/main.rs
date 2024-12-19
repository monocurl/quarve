use quarve::prelude::*;
use quarve::view::layout::{IteratorHMap, IteratorVMap};
use quarve::view::scroll::ScrollView;

struct App;
struct MainWindow;
struct Env(StandardConstEnv, StandardVarEnv);

// mainly boilerplate
impl ApplicationProvider for App {
    // used for calculating certain paths
    fn name(&self) -> &str {
        "Scroll Example"
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
        FixedSignal::new("Scroll App".into())
    }

    fn size(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> (Size, Size, Size) {
        (
            // min
            Size::new(800.0, 400.0),
            // intrinsic
            Size::new(800.0, 400.0),
            // max
            Size::new(800.0, 400.0)
        )
    }

    fn root(&self, env: &<Env as Environment>::Const, s: MSlock) -> impl ViewProvider<Env, DownContext=()> {
        let offset = Store::new(0.0);

        let left_content = (0..10)
            .vmap(|_i, _s| RED.intrinsic(400, 100));

        let right_content = (0.. 10)
            .hmap(|_i, _s| BLUE.intrinsic(100, 400));

        hstack()
            .push(
                ScrollView::vertical_with_binding(left_content, offset.binding())
                    .intrinsic(400, 400)
            )
            .push(
                ScrollView::horizontal_with_binding(right_content, offset.binding())
                    .intrinsic(400, 400)
            )
            .background(WHITE)
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

