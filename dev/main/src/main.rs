use std::thread;
use std::time::Duration;
use quarve::core::{Application, Environment, launch, MSlock, slock_owner, timed_worker};
use quarve::state::{Bindable, Binding, FixedSignal, Signal, Store, VecActionBasis};
use quarve::view::{IntoViewProvider, View, ViewProvider};
use quarve::view::dev_views::{DebugView};
use quarve::view::layout::*;
use quarve::{hstack, vstack};

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
        let store = Store::new(vec![Store::new(1)]);
        let binding = store.binding();
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(5));
            println!("Applying");
            let s = slock_owner();
            binding.apply(VecActionBasis::Insert(Store::new(1), 1), s.marker());
        });
        let items = s.clock_signal()
            .map(|s| {
                let range = 0 .. ((5.0 * s.sin().abs()) as i32);
                range.into_iter().collect()
            }, s);
        // let items = FixedSignal::new(vec![1, 2, 3, 4]);

        vstack! {
            DebugView;
            items.signal_vmap(|i, s| {
                DebugView
            });
            hstack! {
                DebugView;
                DebugView;
            };
            DebugView;
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
