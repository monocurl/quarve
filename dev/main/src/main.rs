use quarve::core::{Application, Environment, launch, MSlock, timed_worker};
use quarve::state::{FixedSignal, Signal};
use quarve::view::{IntoViewProvider, View, ViewProvider};
use quarve::view::dev_views::{DebugView};
use quarve::view::layout::*;
use quarve::vstack;

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
    fn will_spawn(&self, app: &Application, s: MSlock<'_>) {
        app.spawn_window(WindowProvider, s);
    }
}

impl quarve::core::WindowProvider for WindowProvider {
    type Env = Env;

    fn title(&self, _s: MSlock<'_>) -> impl Signal<String> {
        // s.clock_signal()
        //     .map(|time| format!("Time {}", time), s)
        FixedSignal::new("Hello".to_owned())
    }

    fn style(&self, _s: MSlock<'_>) {

    }

    fn tree(&self, env: &Env, s: MSlock<'_>) -> View<Env, impl ViewProvider<Env, DownContext=()>> {
        let items = s.clock_signal()
            .map(|s| {
                let range = 0 .. ((5.0 * s.sin().abs()) as i32);
                range.into_iter().collect()
            }, s);

        vstack! {
            DebugView;
            DebugView;
            items.signal_vmap(|i, s| {
                DebugView
            });
        }
            .into_view_provider(env.const_env(), s)
            .into_view(s)

        //     .into_view_provider(env.const_env(), s)
        //     .into_view(s)
    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
