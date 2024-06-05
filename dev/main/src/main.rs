use quarve::core::{Application, Environment, launch, MSlock};
use quarve::state::{FixedSignal, Signal};
use quarve::view::{Empty, Layout, View, ViewProvider};

struct Env;

struct ApplicationProvider;

struct WindowProvider;

impl Environment for Env {
    fn root_environment() -> Self {
        Env
    }
}

impl quarve::core::ApplicationProvider for ApplicationProvider {
    fn will_spawn(&self, app: &Application, s: MSlock<'_>) {
        app.spawn_window(WindowProvider, s);
    }
}

impl quarve::core::WindowProvider for WindowProvider {
    type Env = Env;

    fn title(&self, s: MSlock<'_>) -> impl Signal<String> {
        // s.clock_signal()
        //     .map(|time| format!("Time {}", time), s)
        FixedSignal::new("Hello".to_owned())
    }

    fn style(&self, _s: MSlock<'_>) {

    }

    fn tree(&self, s: MSlock<'_>) -> View<Env, impl ViewProvider<Env, LayoutContext=()>> {
        let l0 = ViewProvider::make_view(Empty, s);
        let l1 = ViewProvider::make_view(Empty, s);
        let pos = s.clock_signal()
            .map(|s| (*s * 20.0 % 100.0) as f32, s);

        ViewProvider::make_view(Layout(l0, l1, pos), s)
    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
