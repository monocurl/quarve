use quarve::core::{clock_signal, with_app};
use quarve::event::EventModifiers;
use quarve::prelude::*;
use quarve::state::{Filterless, TokenStore};
use quarve::view::color_view::ColorView;
use quarve::view::control::Dropdown;
use quarve::view::menu::{MenuChannel, MenuReceiver, MenuSend};
use quarve::view::scroll::ScrollView;
use quarve::view::text::{Text, TextField, TextModifier};
use crate::icpc::icpc_viewer;
use crate::portal::dynamic_portal;

mod config;
mod modal;
mod portal;
mod icpc;

struct App;
struct MainWindow;
struct Env(StandardConstEnv, StandardVarEnv);

// mainly boilerplate
impl ApplicationProvider for App {
    // used for calculating certain paths
    fn name(&self) -> &str {
        "Counter"
    }

    fn will_spawn(&self, app: &quarve::core::Application, s: MSlock) {
        // init code goes here
        app.spawn_window(MainWindow, s);
    }
}

fn split(s: MSlock) -> impl IVP {
    let left = BLUE
        .padding(10)
        .frame(F
            .squished(100, 100)
            .intrinsic(150, 150)
            .stretched(200, 200)
        );

    let right = RED
        .padding(10)
        .frame(F
            .squished(100, 400)
            .intrinsic(300, 400)
            .stretched(600, 400)
        );

    HSplit::new(
        left, right
    )
}

fn wormpool(s: MSlock) -> impl IVP {
    let selection = Store::new(None);
    selection.listen(|a, _s| {
        println!("Changed {:?}", a);
        true
    }, s);

    let selected = TokenStore::new(Some(1));
    let current = Store::new("A".to_string());
    let current2 = Store::new("A".to_string());
    let color = clock_signal(s)
        .map(|u| rgb((((u * 255.0) as u64) % 255) as u8, 127, 100), s);

    let clock = clock_signal(s);
    let signal = clock
        .map(|c| {
            let amount = (10.0 * c.sin() + 10.0) as i32;
            (0..amount)
                .into_iter()
                .collect::<Vec<i32>>()
        }, s);

    let dynamic_flex = signal.sig_flexmap_options(
        |i, _s| {
            BLUE.padding(10)
                .layer(Layer::default().border(GREEN, 1).radius(3))
                .intrinsic(100 + 10 * i, 20 + 5 * i)
        },
        FlexStackOptions::default()
            .align(FlexAlign::Start)
            .cross_gap(10.0)
            .wrap()
    ).intrinsic(800, 600);

    ScrollView::vertical(
        VStack::hetero_options(VStackOptions::default().align(HorizontalAlignment::Center))
            .push(
                Dropdown::new(selection.binding())
                    .option("Damascus")
                    .option("Solidarity")
                    .intrinsic(100, 30)
                    .padding(10)
                    .border(BLUE, 1)
            )
            .push(
                Text::from_signal(selected.map(|s| {
                    format!("{:?}", *s)
                }, s))
            )
            .push(
                TextField::new(current2.binding())
                    .focused_if_eq(selected.binding(), 1)
                    .text_backcolor(WHITE)
                    .text_color(BLUE)
            )
            .push(
                TextField::new(current.binding())
                    .focused_if_eq(selected.binding(), 0)
                    .text_backcolor(BLACK)
                    .text_color(ORANGE)
            )
            .push(
                text("This is a lot of text\n line 2\n line 3")
                    .text_font("SignikaNegative-Regular.ttf")
            )
            .push(dynamic_flex)
            .push(ColorView::new_signal(color).intrinsic(200, 400))
            .push(button("Click Me 3!", |_| println!("Clicked")))
            .push(button("Click Me 2!", |_| println!("Clicked 2")))
            .push(
                RED.intrinsic(100, 100)
                    .offset(-20, 0)
                    .padding(10)
                    .layer(L.radius(25).bg_color(rgba(255, 255, 255, 100)))
            )
            .layer(
                Layer::default()//.border(RED, 1)
                    .bg_color(PURPLE)
                    .radius(4)
            )
    )
}

fn new_window(s: MSlock) {
    with_app(|app| {
        app.spawn_window(MainWindow, s)
    }, s);
}

// The main code where you specify a view hierarchy
impl WindowProvider for MainWindow {
    type Environment = Env;

    fn title(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> impl Signal<Target=String> {
        FixedSignal::new("Counter".into())
    }

    fn size(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> (Size, Size, Size) {
        (
            // min
            Size::new(400.0, 400.0),
            // intrinsic
            Size::new(800.0, 800.0),
            // max
            Size::new(2400.0, 2000.0)
        )
    }

    fn root(&self, env: &<Env as Environment>::Const, s: MSlock) -> impl ViewProvider<Env, DownContext=()> {
        icpc_viewer()
            .into_view_provider(env, s)
    }

    fn menu(&self, env: &<Self::Environment as Environment>::Const, s: MSlock) -> WindowMenu {
        // in practice, this initialization would be done
        // somewhere else (likely in environment initialization)
        // so that we can also give the channel to the views
        // but lets ignore that for sake of example
        let channel = MenuChannel::new();

        WindowMenu::standard(
            env,
            Menu::new("File")
                .push(MenuReceiver::new(&channel, "New", "N", EventModifiers::default().set_command(), s)),
            Menu::new("Edit"),
            Menu::new("View"),
            Menu::new("Help"),
            s
        )
    }

    fn is_fullscreen(&self, _env: &<Self::Environment as Environment>::Const, s: MSlock) -> impl Binding<Filterless<bool>> {
        let ret = Store::new(false);
        if config::ENABLE_FULLSCREEN_LOGGING {
            ret.listen(|val, _s| {
                println!("Fullscreen State: {}", *val);
                true
            }, s);
        }
        ret.binding()
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

// boilerplate trait alias for IntoViewProvider<Env, UpContext=(), DownContext=()>
// which is referenced a lot but tedious to type
pub(crate) trait IVP: IntoViewProvider<Env, UpContext=(), DownContext=()> { }

impl<I> IVP for I where I: IntoViewProvider<Env, UpContext=(), DownContext=()> { }


fn main() {
    quarve::core::launch(App);
}

