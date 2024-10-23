use std::thread;
use std::time::Duration;
use quarve::core::{Application, Environment, launch, MSlock, run_main_async, slock_owner, StandardConstEnv, StandardVarEnv};
use quarve::event::EventModifiers;
use quarve::prelude::rgb;
use quarve::resource::local_storage;
use quarve::state::{Bindable, FixedSignal, SetAction, Signal, Store, StoreContainerSource, TokenStore};
use quarve::util::geo::{Alignment, HorizontalAlignment, Inset, Size};
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
use quarve::view::text::{AttributeSet, CharAttribute, Page, PageAttribute, Run, RunAttribute, Text, TextField, TextModifier, TextView, TextViewProvider, TextViewState};
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

struct AttrSet;
impl AttributeSet for AttrSet {
    type CharAttribute = CharAttribute;
    type RunAttribute = RunAttribute;
    type PageAttribute = PageAttribute;
}

struct TVProvider {

}

impl TextViewProvider<Env> for TVProvider {
    type IntrinsicAttribute = AttrSet;
    type DerivedAttribute = AttrSet;
    const PAGE_INSET: Inset = Inset {
        l: 0.0,
        r: 0.0,
        b: 0.0,
        t: 0.0,
    };

    fn init(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) {
    }

    fn tab(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) {
    }

    fn untab(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) {
    }

    fn newline(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) {
    }

    fn alt_newline(&mut self, state: &TextViewState<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) {
    }

    fn run_decoration(&self, number: impl Signal<Target=usize>, run: &Run<Self::IntrinsicAttribute, Self::DerivedAttribute>, s: MSlock) -> impl IntoViewProvider<Env, DownContext=(), UpContext=()> + 'static {
        EmptyView
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
        p.replace_range(0, 0, 0, 0, "test", s);
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
                println!("about to call");
                p.replace_range(
                    0, 1,
                    0, 2,
                    "Auxiliary\nTHIS IS SECOND LINE\n",
                    s
                );
                println!("Content {:?}", p.build_full_content(s));
            })
        });

        // let portal = Portal::new();

        // let v1 = ScrollView::vertical(
        //     VStack::hetero_options(VStackOptions::default().align(HorizontalAlignment::Leading))
        //         .push(
        //             Button::new_with_label(
        //                 Color::black()
        //                     .intrinsic(100, 100),
        //                 move |s| {
        //                     SaveFilePicker::new()
        //                         .content_types("pdf")
        //                         .run(|path, s| {
        //
        //                         });
        //                     binding.apply(SetAction::Set(None), s);
        //                 }
        //             )
        //                 .offset_signal(FixedSignal::new(0.0), offset_y.signal())
        //         )
        //         .push(
        //             Button::new("Focus", move |s| {
        //                 binding2.apply(SetAction::Set(Some(2)), s);
        //             })
        //         )
        //         .push(
        //             Dropdown::new(selected.binding())
        //                 .option("Hello")
        //                 .option("World")
        //         )
        //         .push(
        //             Text::from_signal(focused.map(|r| format!("Selected {:?}", r), s))
        //                 .text_backcolor(Color::rgb(255, 0, 2))
        //                 .text_size(24.0)
        //         )
        //         .push(
        //             TextField::new(text.binding())
        //                 .unstyled()
        //                 .focused_if_eq(focused.binding(), 2)
        //                 .max_lines(0)
        //                 .text_size(24.0)
        //                 .padding(10)
        //         )
        //         .push(
        //             TextField::new(text.binding())
        //                 .focused_if_eq(focused.binding(), 3)
        //                 .text_size(24.0)
        //         )
        //         .push(
        //             TextView::new(tv.view(), TVProvider {})
        //                 .frame(Frame::default().stretched(10000, 100))
        //                 .border(Color::white(), 1)
        //                 .padding(10.0)
        //                 .bg_color(Color::rgba(0,0,0,120))
        //         )
        //         .push(
        //             Dropdown::new(selected.binding())
        //                 .option("Hello")
        //                 .option("World")
        //         )
        //         .push(
        //             (0..14)
        //                 .vmap(|x, _s| {
        //                     Color::white()
        //                         .intrinsic(100, 100 + 10 * *x)
        //                         .cursor(Cursor::Pointer)
        //                 })
        //                 .padding(10)
        //                 .border(Color::black(), 1)
        //         )
        //         .push(
        //             Text::new(format!("Item {:?}", 1))
        //                 .intrinsic(100, 100)
        //                 .bg_color(rgb(0, 0, 0))
        //         )
        //     // offset_y.binding()
        // )
        //     .frame(Frame::default()
        //         .intrinsic(300, 300)
        //             .unlimited_stretch()
        //             .align(Alignment::Center)
        //     )
        //     .mount_undo_manager(UndoManager::new(&stores, s))
        //     .text_color(Color::white())
        //     .text_font("SignikaNegative-Regular.ttf")
        //     .underline()
        //     .bold();
        //
        // let v2 = ScrollView::vertical_with_binding(
        //         VStack::hetero_options(VStackOptions::default().align(HorizontalAlignment::Leading))
        //             .push(
        //                 Button::new_with_label(
        //                     Color::white()
        //                         .intrinsic(100, 100),
        //                     |_| println!("Clicked")
        //                 )
        //                     .offset_signal(FixedSignal::new(0.0), offset_y.signal())
        //             )
        //             .push(
        //                 (0..14).vmap(|x, s| {
        //                         Color::black()
        //                             .intrinsic(100, 100 + 10 * *x)
        //                             .cursor(Cursor::Pointer)
        //                     })
        //                     .padding(10)
        //                     .border(Color::white(), 1)
        //             ),
        //         offset_y.binding()
        //     )
        //         .frame(Frame::default()
        //             .intrinsic(300, 300)
        //             .unlimited_stretch()
        //             .align(Alignment::Center)
        //         );

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
                    .text_size(24.0)
            )
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
