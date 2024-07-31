use quarve::core::{Application, Environment, launch, MSlock, StandardConstEnv, StandardVarEnv};
use quarve::event::EventModifiers;
use quarve::state::{FixedSignal, Signal, Store};
use quarve::util::geo::{Alignment, HorizontalAlignment, Size};
use quarve::view::{ViewProvider, IntoViewProvider, WeakInvalidator};
use quarve::view::conditional::view_if;
use quarve::view::control::{Button, Dropdown};
use quarve::view::layout::*;
use quarve::view::menu::{Menu, MenuButton, MenuSend, WindowMenu};
use quarve::view::modifers::{Cursor, CursorModifiable, EnvironmentModifier, Frame, FrameModifiable, OffsetModifiable, PaddingModifiable};
use quarve::view::scroll::ScrollView;
use quarve::view::text::{Text, TextField, TextModifier};
use quarve::view::undo_manager::{UndoManager, UndoManagerExt};
use quarve::view::util::Color;

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
    fn will_spawn(&self, app: &Application, s: MSlock) {
        app.spawn_window(WindowProvider, s);
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
        let selected = Store::new(None);
        let text = Store::new("Velociraptor".to_owned());

        let v1 = ScrollView::vertical_with_binding(
            VStack::hetero_options(VStackOptions::default().align(HorizontalAlignment::Leading))
                .push(
                    Button::new_with_label(
                        Color::black()
                            .intrinsic(100, 100),
                        |s| {

                        }
                    )
                        .offset_signal(FixedSignal::new(0.0), offset_y.signal())
                )
                .push(
                    Dropdown::new(selected.binding())
                        .option("Hello")
                        .option("World")
                )
                .push(
                    Text::new("Test")
                        .text_backcolor(Color::rgb(255, 0, 2))
                        .text_size(24.0)
                )
                .push(
                    TextField::new(text.binding())
                        .text_size(24.0)
                )
                .push(
                    TextField::new(text.binding())
                )
                .push(
                    Dropdown::new(selected.binding())
                        .option("Hello")
                        .option("World")
                )
                .push(
                    (0..14)
                        .vmap(|x, _s| {
                            Color::white()
                                .intrinsic(100, 100 + 10 * *x)
                                .cursor(Cursor::Pointer)
                        })
                        .padding(10)
                        .border(Color::black(), 1)
                ),
            offset_y.binding()
        )
            .frame(Frame::default()
                .intrinsic(300, 300)
                    .unlimited_stretch()
                    .align(Alignment::Center)
            )
            .mount_undo_manager(UndoManager::new(&selected, s))
            .text_color(Color::white())
            .text_font("SignikaNegative-Regular.ttf")
            .underline()
            .bold();

        let v2 = ScrollView::vertical_with_binding(
                VStack::hetero_options(VStackOptions::default().align(HorizontalAlignment::Leading))
                    .push(
                        Button::new_with_label(
                            Color::white()
                                .intrinsic(100, 100),
                            |_| println!("Clicked")
                        )
                            .offset_signal(FixedSignal::new(0.0), offset_y.signal())
                    )
                    .push(
                        (0..14).vmap(|x, s| {
                                Color::black()
                                    .intrinsic(100, 100 + 10 * *x)
                                    .cursor(Cursor::Pointer)
                            })
                            .padding(10)
                            .border(Color::white(), 1)
                    ),
                offset_y.binding()
            )
                .frame(Frame::default()
                    .intrinsic(300, 300)
                    .unlimited_stretch()
                    .align(Alignment::Center)
                );

        VStack::hetero()
            .push(v1)
            .push(
                view_if(offset_y.map(|y| *y < 10.0, s),
                    v2
                        .menu_send(&env.channels.select_all_menu, |_|println!("Delete"))
                )
            )
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
