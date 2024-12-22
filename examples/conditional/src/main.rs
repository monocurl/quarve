use quarve::prelude::*;
use quarve::view::control::Dropdown;
use quarve::view::text::{TextModifier};
use quarve::{view_match, vstack};
use quarve::state::{SetAction};

struct App;
struct MainWindow;
pub(crate) struct Env(StandardConstEnv, StandardVarEnv);

// mainly boilerplate
impl ApplicationProvider for App {
    // used for calculating certain paths
    fn name(&self) -> &str {
        "Quarve Example"
    }

    fn will_spawn(&self, app: &quarve::core::Application, s: MSlock) {
        // init code goes here
        app.spawn_window(MainWindow, s);
    }
}

fn mux_demo() -> impl IVP {
    let selection = Store::new(None);
    let selection_sig = selection.signal();

    // view match allows you to select the type of the view
    // to use dynamically (notice that the second arm has a different type than the first)
    // it takes as parameter a signal
    // an optional map on the signal
    // (necessary in this case since you can't match on an Option<String>)
    // and a set of match arms
    let mux = view_match!(selection_sig, |val: &Option<String>| val.as_ref().map(move |q| q.as_str());
        Some("Alpha") => text("alpha text").bold(),
        Some("Beta") => button("beta button", |_s| println!("clicked!")).italic(),
        _ => text("Please select an option")
    );

    // macro syntax of vstack
    vstack! {
        mux;
        Dropdown::new(selection.binding())
            .option("Alpha")
            .option("Beta");
    }
}

fn if_demo() -> impl IVP {
    let shown = Store::new(false);
    let shown_binding = shown.binding();

    hstack()
        .push(button("toggle color", move |s| {
            let curr = *shown_binding.borrow(s);
            shown_binding.apply(SetAction::Set(!curr), s);
        }))
        .push(
            // dont like this syntax that much
            // but i also think a macro would be overkill
            view_if(shown.signal(), BLUE.intrinsic(50, 50))
                .view_else(RED.intrinsic(50, 50))
        )
}

// The main code where you specify a view hierarchy
impl WindowProvider for MainWindow {
    type Environment = Env;

    fn title(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> impl Signal<Target=String> {
        FixedSignal::new("Conditionals Demo".into())
    }

    fn size(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> (Size, Size, Size) {
        (
            // min
            Size::new(400.0, 400.0),
            // intrinsic
            Size::new(400.0, 400.0),
            // max
            Size::new(400.0, 400.0)
        )
    }

    fn root(&self, env: &<Env as Environment>::Const, s: MSlock) -> impl ViewProvider<Env, DownContext=()> {
        let mux = mux_demo();
        let if_portion = if_demo();

        vstack()
            .push(mux)
            .push(if_portion)
            .text_color(BLUE)
            .frame(
                F.intrinsic(400, 400).unlimited_stretch()
            )
            .background(BLACK)
            .into_view_provider(env, s)
    }

    fn menu(&self, env: &<Self::Environment as Environment>::Const, s: MSlock) -> WindowMenu {
        WindowMenu::standard(
            env,
            Menu::new("File"),
            Menu::new("Edit"),
            Menu::new("View"),
            Menu::new("Help"),
            s
        )
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

