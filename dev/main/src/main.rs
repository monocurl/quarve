use std::thread;
use std::time::Duration;
use quarve::core::{Application, Environment, launch, MSlock, run_main_async, slock_owner, StandardConstEnv, StandardVarEnv};
use quarve::event::EventModifiers;
use quarve::prelude::rgb;
use quarve::resource::{local_storage, Resource};
use quarve::state::{Bindable, FixedSignal, JoinedSignal, SetAction, Signal, Store, StoreContainerSource, TokenStore};
use quarve::util::geo::{Alignment, HorizontalAlignment, Inset, ScreenUnit, Size};
use quarve::view::{ViewProvider, IntoViewProvider, WeakInvalidator};
use quarve::view::color_view::EmptyView;
use quarve::state::Binding;
use quarve::view::conditional::view_if;
use quarve::view::control::{Button, Dropdown};
use quarve::view::layout::*;
use quarve::view::menu::{Menu, MenuButton, MenuSend, WindowMenu};
use quarve::view::modal::{OpenFilePicker, SaveFilePicker};
use quarve::view::modifers::{Cursor, CursorModifiable, EnvironmentModifier, ForeBackModifiable, Frame, FrameModifiable, OffsetModifiable, PaddingModifiable};
use quarve::view::portal::{Portal, PortalReceiver, PortalSendable};
use quarve::view::scroll::ScrollView;
use quarve::view::text::{AttributeSet, CharAttribute, Indentation, Justification, Page, PageAttribute, Run, RunAttribute, Text, TextField, TextModifier, TextView, TextViewProvider, TextViewState, ToCharAttribute};
use quarve::view::undo_manager::{UndoManager, UndoManagerExt};
use quarve::view::util::Color;
use quarve_derive::StoreContainer;

struct Env(StandardConstEnv, StandardVarEnv);

struct ApplicationProvider;

struct WindowProvider;

impl Environment for Env {
    type Const = StandardConstEnv;
    type Variable = StandardVarEnv;

    fn root_environment() -> Self {
        Env(StandardConstEnv::new(), StandardVarEnv::new())
    }

    fn const_env(&self) -> &Self::Const {
        &self.0
    }

    fn variable_env(&self) -> &Self::Variable {
        &self.1
    }

    fn variable_env_mut(&mut self) -> &mut Self::Variable {
        &mut self.1
    }
}

impl quarve::core::ApplicationProvider for ApplicationProvider {
    fn name(&self) -> &str {
        "quarve_dev"
    }

    fn will_spawn(&self, app: &Application, s: MSlock) {
        app.spawn_window(WindowProvider, s);
    }
}

#[derive(StoreContainer)]
struct Stores {
    selected: Store<Option<String>>,
    text: Store<String>
}

#[derive(PartialEq, Clone, Default)]
struct FakeChar {
    bold: bool,
}

impl ToCharAttribute for FakeChar {
    fn to_char_attribute(&self) -> impl AsRef<CharAttribute> {
        CharAttribute {
            bold: Some(self.bold),
            italic: None,
            underline: None,
            strikethrough: None,
            back_color: Some(Color::black()),
            fore_color: Some(Color::rgb(255, 0, 0)),
        }
    }
}

struct AttrSet;
impl AttributeSet for AttrSet {
    type CharAttribute = FakeChar;
    type RunAttribute = RunAttribute;
    type PageAttribute = PageAttribute;
}

struct TVProvider {

}

impl TextViewProvider<Env> for TVProvider {
    type IntrinsicAttribute = AttrSet;
    type DerivedAttribute = AttrSet;
    const PAGE_INSET: Inset = Inset {
        l: 20.0,
        r: 10.0,
        b: 10.0,
        t: 0.0,
    };

    // fn font() -> Option<Resource> {
    //     Some(Resource::named("font/SignikaNegative-Regular.ttf"))
    // }

    fn font_size() -> ScreenUnit {
        16.
    }

    fn init(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) {

    }

    fn tab(&mut self, state: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> bool {
        false
    }

    fn run_decoration(&self, number: impl Signal<Target=usize>, run: &Run<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> impl IntoViewProvider<Env, DownContext=(), UpContext=()> + 'static {
        // Text::new("Test")
        let sig =
            JoinedSignal::join(&number, &run.content_signal(), s)
                .map(|(n, r)| format!("{}", n), s);
        Text::from_signal(sig)
            .text_color(Color::white())
            .frame(Frame::default().align(Alignment::Trailing).intrinsic(100., 20))
            .offset(-100, 0)
    }


    fn page_background(&self, page: &Page<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> impl IntoViewProvider<Env, DownContext=()> + 'static {
        EmptyView
    }
}

impl quarve::core::WindowProvider for WindowProvider {
    type Env = Env;

    fn title(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> impl Signal<Target=String> {
        // s.clock_signal()
        //     .map(|time| format!("Time {}", time), s)
        FixedSignal::new("Hello".to_owned())
    }

    fn size(&self, _env: &<Env as Environment>::Const, _s: MSlock) -> (Size, Size, Size) {
        (
            Size::new(400.0, 400.0),
            Size::new(400.0, 400.0),
            Size::new(800.0, 1000.0)
        )
    }

    fn root(&self, env: &<Env as Environment>::Const, s: MSlock) -> impl ViewProvider<Env, DownContext=()> {
        let offset_y = Store::new(0.0);
        let stores = Stores {
            selected: Store::new(None),
            text: Store::new("Elide".to_string())
        };
        let selected = &stores.selected;
        let text = &stores.text;
        let focused = TokenStore::new(Some(2));
        let binding = focused.binding();
        let binding2 = focused.binding();

        let tv = StoreContainerSource::new(TextViewState::new());
        let p = Page::new(s);
        p.replace_range(0, 0, 0, 0, "test\nnew", s);
        tv.insert_page(p, 0, s.to_general_slock());

        let tv2 = tv.view();
        {
            let p = tv2.page(0, s);
            // p.replace_range(
            //     0, 0,
            //     0, 0,
            //     "HELLO WORLD\nTHIS IS SECOND LINE\n",
            //     s
            // )
        }
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(3));

            run_main_async(move |s| {
                let p = tv2.page(0, s);
                p.replace_range(
                    0, 1,
                    0, 2,
                    "Auxiliary\nTHIS IS SECOND LINE\n",
                    s
                );
                p.run(1, s)
                    .set_char_intrinsic(FakeChar {
                        bold: true,
                    }, 2..5, s);
                p.run(1, s)
                    .set_intrinsic(RunAttribute {
                        justification: Some(Justification::Center),
                        indentation: Some(Indentation {
                            leading: 10.0,
                            trailing: 20.0,
                        }),
                    }, s);
            })
        });

        VStack::hetero()
            .push(
                TextView::new(tv.view(), TVProvider {})
                    .frame(Frame::default()
                        .intrinsic(100, 400)
                        .stretched(10000, 400)
                    )
                    .border(Color::white(), 1)

            )
            .push(
                TextField::new(text.binding())
                    // .focused_if_eq(focused.binding(), 3)
                    .text_font("SignikaNegative-Regular.ttf")
                    .text_size(24.0)
            )
            .mount_undo_manager(UndoManager::new(&tv, s))
        //     // .push(v1)
        //     // .push(v2)
            .into_view_provider(env, s)
    }

    fn menu(&self, env: &<Self::Env as Environment>::Const, s: MSlock) -> WindowMenu {
        WindowMenu::standard(
            env,
            Menu::new("File")
                .push(MenuButton::new("Test", "", EventModifiers::new(), |_| println!("Test Called")))
            ,
            Menu::new("Edit") ,
            Menu::new("View"),
            Menu::new("Help"),
            s
        )
    }
}

struct EnvModifier {

}

impl EnvironmentModifier<Env> for EnvModifier {
    fn init(&mut self, invalidator: WeakInvalidator<Env>, s: MSlock) {

    }

    fn push_environment(&mut self, env: &mut <Env as Environment>::Variable, s: MSlock) {

    }

    fn pop_environment(&mut self, env: &mut <Env as Environment>::Variable, s: MSlock) {

    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
