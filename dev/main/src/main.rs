use std::thread;
use std::time::Duration;
use quarve::core::{Application, Environment, launch, MSlock, slock_owner, timed_worker};
use quarve::state::{Bindable, FixedSignal, Signal, Store};
use quarve::view::{IntoViewProvider, ViewProvider};
use quarve::view::dev_views::{DebugView};
use quarve::view::layout::*;
use quarve::{hstack, vstack};
use quarve::view::modifers::{OffsetModifiable, WhenModifiable};

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

    fn root(&self, env: &<Env as Environment>::Const, s: MSlock<'_>) -> impl ViewProvider<Env, DownContext=()> {
        // let iteration = |i: f64| {
        //     s.clock_signal()
        //         .map(move |s| {
        //             let range = 0 .. (1 + (5.0 * (i + s).sin().abs()) as i32);
        //             range.into_iter().collect()
        //         }, s)
        //         .signal_vmap_options(|_i, _s| {
        //             DebugView
        //         }, |o| o.spacing(10.0))
        // };
        let store = Store::new(vec![Store::new(1), Store::new(2)]);

        let offset_y = s
            .clock_signal()
            .map(|u| ((4.0 * u).sin() * 100.0) as f32, s);
        let positive_y = offset_y
            .map(|val| *val > 0.0, s);

        hstack! {
            DebugView
                .offset(200.0, 110.0)
                .when(positive_y, |u|
                   u.offset_signal(offset_y, s.fixed_signal(0.0))
                );

            store.binding_vmap(|x, _s| {
                DebugView
            })
        }
        .into_view_provider(env, s)
    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
