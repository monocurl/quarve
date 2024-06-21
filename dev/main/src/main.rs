use quarve::core::{Application, Environment, launch, MSlock};
use quarve::state::{FixedSignal, Signal};
use quarve::view::{ViewProvider, IntoViewProvider};
use quarve::view::layout::*;
use quarve::view::portal::*;
use quarve::{hstack};
use quarve::view::color_view::{EmptyView};
use quarve::view::modifers::{FrameModifiable, WhenModifiable};
use quarve::view::portal::Portal;
use quarve::view::util::Color;

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
        let enabled = s.clock_signal()
            .map(|val| *val % 2.0 > 1.0, s);
        let enabled_int = enabled.map(|u| if *u {vec![1]} else {vec![]}, s);
        let not_enabled = enabled.map(|u| !*u, s);
        let not_enabled_int = not_enabled.map(|u| if *u {vec![1]} else {vec![]}, s);
        let p = Portal::new();
        let p2 = p.clone();
        let p3 = p.clone();

        let black_box =
            Color::black().intrinsic(100, 100);

        hstack! {
            EmptyView;

            EmptyView
                .when(not_enabled, |v| {
                    v.portal_sender(&p, Color::black().intrinsic(200, 100))
                });

            not_enabled_int
                .signal_vmap(move |_, s| {
                    PortalReceiver::new(&p3)
                });
            // PortalReceiver::new(&p);
            enabled_int
                .signal_vmap(move |_, s| {
                    PortalReceiver::new(&p2)
                });

            EmptyView
                .when(enabled, |v| {
                    v.portal_sender(&p, black_box)
                });


        }
        .into_view_provider(env, s)
    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
