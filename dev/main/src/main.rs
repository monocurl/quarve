use quarve::core::{Application, EnvironmentProvider, launch, MSlock};
use quarve::state::Signal;
use quarve::view::{Empty, View, ViewProvider};

struct Env;

struct ApplicationProvider;

struct WindowProvider;

impl EnvironmentProvider for Env {
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
    type Environment = Env;

    fn title(&self, s: MSlock<'_>) -> impl Signal<String> {
        s.clock_signal()
            .map(|time| format!("Time {}", time), s)
    }

    fn style(&self, _s: MSlock<'_>) {

    }

    fn tree(&self, s: MSlock<'_>) -> View<Env, impl ViewProvider<Env, LayoutContext=()>> {
        ViewProvider::make_view(Empty, s)
    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
