use quarve::core::{Application, Environment, launch, MSlock, timed_worker};
use quarve::state::{Bindable, Binding, FixedSignal, NumericAction, Signal, Store, WithCapacitor};
use quarve::state::capacitor::{ConstantSpeedCapacitor, SmoothCapacitor};
use quarve::view::{View, ViewProvider};
use quarve::view::layout::{DebugView, Layout};

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

    fn tree(&self, _env: &Env, s: MSlock<'_>) -> View<Env, impl ViewProvider<Env, LayoutContext=()>> {
        let l0 = ViewProvider::make_view(DebugView, s);
        let l1 = ViewProvider::make_view(DebugView, s);

        let pos = s.clock_signal()
            .map(|s| {
                let t = *s as f32 % 1.0;
                let u = 3.0 * t * t - 2.0 * t * t * t;
                u * std::f32::consts::PI * 2.0
            }, s);

        let store = Store::new(0.0);
        let signal = store.signal();
        let capacitated = store.with_capacitor(ConstantSpeedCapacitor::new(1.0), s);

        let mut counter = 0;
        timed_worker(move |d, s| {
            if (d.as_secs_f64() > counter as f64) {
                counter += 1;
                store.apply(NumericAction::Incr(1.0), s);
            }
            true
        });

        ViewProvider::make_view(Layout(l0, l1, capacitated), s)
    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
