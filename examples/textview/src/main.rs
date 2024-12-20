use std::sync::mpsc::channel;
use std::thread;
use quarve::core::slock_owner;
use quarve::prelude::*;
use quarve::state::{StoreContainerSource, StoreContainerView};
use quarve::view::color_view::EmptyView;
use quarve::view::text::{AttributeSet, CharAttribute, Page, PageAttribute, Run, RunAttribute, Text, TextModifier, TextView, TextViewProvider, TextViewState, ToCharAttribute};
use quarve::view::undo_manager::{UndoManager, UndoManagerExt};

struct App;
struct MainWindow;
struct Env(StandardConstEnv, StandardVarEnv);


// text views work an attribute model
// each run (or line) as well as character has an associated
// set of attributes. There are both derived attributes
// that you can think of as attributes that are determined
// solely for the text content, and intrinsic attributes
// which you can think of as attributes that may vary
// for even the same text content. In this demo, we will
// be building a parser that implements markdown bold
// highlighting. Since the boldness is determined exactly
// from the text content, we only need derived attributes
struct Intrinsic;
impl AttributeSet for Intrinsic {
    type CharAttribute = CharAttribute;
    type RunAttribute = RunAttribute;
    type PageAttribute = PageAttribute;
}

struct Derived;
impl AttributeSet for Derived {
    type CharAttribute = Attribute;
    type RunAttribute = RunAttribute;
    type PageAttribute = PageAttribute;
}

#[derive(Default, Clone, PartialEq)]
struct Attribute {
    bold: bool
}

impl ToCharAttribute for Attribute {
    fn to_char_attribute(&self) -> impl AsRef<CharAttribute> {
        CharAttribute {
            bold: Some(self.bold),
            italic: None,
            underline: None,
            strikethrough: None,
            back_color: None,
            fore_color: Some(if self.bold {
                RED
            } else {
                WHITE
            }),
        }
    }
}

struct TVP;
impl TextViewProvider<Env> for TVP {
    type IntrinsicAttribute = Intrinsic;
    type DerivedAttribute = Derived;
    const PAGE_INSET: Inset = Inset {
        l: 40.0,
        r: 10.0,
        b: 10.0,
        t: 10.0,
    };

    fn init(&mut self, state: StoreContainerView<TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>>, s: MSlock) {
        // see multithread example for more information about how multithreading works
        // here, whenever there is a change to state, we request a rerender of all lines
        let (notifier, receiver) = channel();

        // initial request
        notifier.send(()).unwrap();

        state.subtree_general_listener(move |_s| {
            notifier.send(()).unwrap();
            true
        }, s);

        let state = state.clone();
        // worker thread that applies formatting
        thread::spawn(move || {
            while receiver.recv().is_ok() {
                let slock = slock_owner();
                let s = slock.marker();

                let page = state.page(0, s);
                let mut bold = false;
                for run in page.runs(s).iter() {
                    // in efficient implementation character by character,
                    // but simple for example
                    let c = run.content(s);

                    for (i, c) in c.char_indices() {
                        let curr_bold = if bold && c == '*' {
                            bold = false;
                            true
                        }
                        else if !bold && c == '*' {
                            bold = true;
                            true
                        }
                        else {
                            bold
                        };

                        run.set_char_derived(Attribute {
                            bold: curr_bold,
                        }, i..(i + c.len_utf8()), s);
                    }
                }
            }
        });
    }

    fn run_decoration(&self, number: impl Signal<Target=usize>, _run: &Run<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> impl IntoViewProvider<Env, DownContext=()> {
        // line number for every single run
        Text::from_signal(number.map(|u| (u + 1).to_string(), s))
            .frame(
                F.intrinsic(100, 20)
                    .unlimited_height()
                    .align(Alignment::TopTrailing)
            )
            .offset(-100, 0)
            .text_color(GRAY)
    }

    // no background or foreground
    fn page_background(&self, _page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, _s: MSlock) -> impl IntoViewProvider<Env, DownContext=()> {
        EmptyView
    }

    // useful for creating attractions near the cursor
    fn page_foreground(&self, _page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, _cursor: impl Signal<Target=Option<Point>>, _s: MSlock) -> impl IntoViewProvider<Env, DownContext=()> {
        EmptyView
    }
}

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

// The main code where you specify a view hierarchy
impl WindowProvider for MainWindow {
    type Environment = Env;

    fn title(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> impl Signal<Target=String> {
        FixedSignal::new("Text View".into())
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
        // store container source is basically just Arc
        let state = StoreContainerSource::new(TextViewState::new());
        let content = Page::new(s);
        content.replace_range(0, 0, 0, 0, "Type with *asterisks* to \nbold certain text", s);
        state.insert_page(content, 0, s);

        // add undo support
        let undo_manager = UndoManager::new(&state, s);

        let provider = TVP;

        TextView::new(state.view(), provider)
            .frame(
                F.intrinsic(400, 400).unlimited_stretch()
            )
            .background(BLACK)
            .mount_undo_manager(undo_manager)
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

fn main() {
    quarve::core::launch(App);
}

