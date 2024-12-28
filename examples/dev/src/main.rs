use quarve::event::EventModifiers;
use quarve::prelude::*;
use quarve::state::{Filterless, TokenStore};
use quarve::view::color_view::EmptyView;
use quarve::view::control::{Button, Dropdown};
use quarve::view::image_view::ImageView;
use quarve::view::scroll::ScrollView;
use quarve::view::text::{Text, TextField, TextModifier};

mod config;

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

fn view(s: MSlock) -> impl IVP {
    let selection = Store::new(None);
    selection.listen(|a, s| {
        println!("Changed {:?}", a);
        true
    }, s);

    let selected = TokenStore::new(Some(1));
    let current = Store::new("A".to_string());
    let current2 = Store::new("A".to_string());

    VStack::hetero_options(VStackOptions::default().align(HorizontalAlignment::Center))
        .push(
            Dropdown::new(selection.binding())
                .option("Damascus")
                .option("Solidarity")
                .intrinsic(100, 30)
        )
        .push(
            Text::from_signal(selected.map(|s| {
                format!("{:?}", *s)
            }, s))
        )
        .push(
            TextField::new(current.binding())
                .focused_if_eq(selected.binding(), 0)
                .text_backcolor(BLACK)
                .text_color(ORANGE)
        )
        .push(
            TextField::new(current2.binding())
                .focused_if_eq(selected.binding(), 1)
                .text_backcolor(WHITE)
                .text_color(BLUE)
        )
        .push(
            ScrollView::vertical(
                vstack()
                    .push(
                        text("This is a lot of text\n line 2\n line 3")
                            .max_lines(2)
                            .text_font("SignikaNegative-Regular.ttf")
                    )
                    .push(RED.intrinsic(200, 400))
                    .push(button("Click Me 3!", |_| println!("Clicked")))
                    .frame(F.intrinsic(400, 1000).unlimited_stretch().align(Alignment::Center))
            )
        )
        .push(button("Click Me 2!", |_| println!("Clicked 2")))
        // .push(
        //     ColorView::new_signal(color)
        //         .intrinsic(100, 100)
        //         .cursor(Cursor::Pointer)
        // )
        .push(
            Button::new_with_label(BLUE.intrinsic(100, 100), |s| {
                println!("Hello")
            })
        )
        .push(
            ImageView::named("rose.png")
        )
        .push(
            EmptyView.intrinsic(100, 100)
                .border(RED, 1)
        )
        .frame(
            F.intrinsic(400, 400).unlimited_stretch()
                .align(Alignment::Center)
        )
        .layer(
            Layer::default()//.border(RED, 1)
                .bg_color(PURPLE)
                .radius(4)
        )
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
        view(s)
            .into_view_provider(env, s)
    }

    fn menu(&self, env: &<Self::Environment as Environment>::Const, s: MSlock) -> WindowMenu {
        WindowMenu::standard(
            env,
            Menu::new("File")
                .push(MenuButton::new("New", "N", EventModifiers::default().set_command(), |_s| {
                    println!("Clicked menu button");
                })),
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

