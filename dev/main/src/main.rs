use quarve::core::{Application, Environment, launch, MSlock, timed_worker};
use quarve::state::{Binding, FixedSignal, Signal, Store, WithCapacitor};
use quarve::state::capacitor::{ConstantTimeCapacitor};
use quarve::state::SetAction::Set;
use quarve::util::Vector;
use quarve::view::{View, ViewProvider};
use quarve::view::dev_views::{DebugView, Layout};

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

    fn title(&self, _s: MSlock<'_>) -> impl Signal<String> {
        // s.clock_signal()
        //     .map(|time| format!("Time {}", time), s)
        FixedSignal::new("Hello".to_owned())
    }

    fn style(&self, _s: MSlock<'_>) {

    }

    fn tree(&self, env: &Env, s: MSlock<'_>) -> View<Env, impl ViewProvider<Env, LayoutContext=()>> {
        let l0 = DebugView.into_view(s);
        let l1 = DebugView.into_view(s);

        let store = Store::new(Vector::from_array([0.0, 0.0]));
        let capacitated =
            store.with_capacitor(ConstantTimeCapacitor::new(0.5), s);
            // store.with_capacitor(SmoothCapacitor::ease_in_out(3.5), s);

        store.apply([Set(100.0), Set(10.0)], s);

        Layout(l0, l1, capacitated)
            .into_view(s)
    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
