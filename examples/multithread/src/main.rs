use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;
use quarve::core::slock_owner;
use quarve::prelude::*;
use quarve::state::SetAction;
use quarve::view::text::{Text, TextField, TextModifier};

struct App;
struct MainWindow;
struct Env(StandardConstEnv, StandardVarEnv);

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
        FixedSignal::new("Multithreaded Demo".into())
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
        // one of quarve's innovations
        // is the ability for having effortless multithreaded apps
        // here we simulate a long running worker task that is scheduled by a button

        let answer = Store::new(0);
        let answer_binding = answer.binding();

        let (sender, receiver) = channel();
        thread::spawn(move || {
            while let Some((a, b)) = receiver.recv().ok() {
                // perform (simulated) work to calculate result
                thread::sleep(Duration::from_secs(4));

                let result = a + b;
                // we now have the result, and can update the answer
                // all state changes require the state lock (slock), which we can acquire
                // try to acquire the state lock as late as possible to avoid stalls
                let s = slock_owner();
                answer_binding.apply(SetAction::Set(result), s.marker());
            }
        });

        // for simplicity, we will assume that the string is correctly written with only digits
        let a_text = Store::new("0".into());
        let a = TextField::new(a_text.binding())
            .intrinsic(100, 30)
            .padding(5)
            .border(BLACK, 1);

        let b_text = Store::new("0".into());
        let b = TextField::new(b_text.binding())
            .intrinsic(100, 30)
            .padding(5)
            .border(BLACK, 1);

        vstack()
            .push(
                hstack()
                    .push(a)
                    .push(b)
            )
            .push(button("Calculate Sum", move |s| {
                let Ok(a) = a_text.borrow(s).parse::<i32>() else {
                    return;
                };
                let Ok(b) = b_text.borrow(s).parse::<i32>() else {
                    return;
                };

                sender.send((a, b))
                    .expect("Unable to send job to worker");
            }).text_color(BLUE))
            .push(Text::from_signal(answer.map(|u| format!("Answer: {:?}", *u), s)))
            .frame(
                F.intrinsic(400, 400).unlimited_stretch()
            )
            .background(WHITE)
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

