use quarve::core::{Application, Environment, launch, MSlock};
use quarve::state::{FixedSignal, Signal, Store};
use quarve::view::{IntoViewProvider, ViewProvider};
use quarve::view::dev_views::{DebugView};
use quarve::view::layout::*;
use quarve::{hstack};
use quarve::util::geo;
use quarve::view::modifers::{ForeBackModifiable, FrameModifiable, LayerModifiable, OffsetModifiable, PaddingModifiable, WhenModifiable};
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
            .map(|u| ((4.0 * u).sin().abs() * 100.0) as f32, s);
        let positive_y = offset_y
            .map(|val| *val > 40.0, s);
        let positive_y_ind = positive_y.map(|val| {
            if *val {
                Color::new(100, 0, 0)
            }
            else {
                Color::transparent()
            }
        }, s);
        hstack! {
            // DebugView
            //     .padding(10.0)
            //     .when(positive_y, |u|
            //        u.layer(|l| {
            //             l.bg_color(Color::new(100, 0, 0))
            //              .border_color(Color::black())
            //              .radius(40.0)
            //              .border_width(3.0)
            //              .opacity(0.5)
            //         })
            //         // .offset_signal(s.fixed_signal(0.0), offset_y)
            //     );
            //     // .offset(200.0, 110.0);
            //
            store.binding_vmap(move |_x, _s| {
                DebugView
                    .padding(20.0)
                    .frame(|f| f.intrinsic(200.0, 200.0))
                    .padding_edge(100.0, geo::edge::LEFT)
                    .layer(|l|
                        l.border(Color::black(), 1.0)
                    )
                    .padding(10.0)
                    .layer(|l| l.border_color(Color::black()).border_width(1.0))
            })
        }
        .into_view_provider(env, s)
    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
