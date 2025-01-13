use quarve::prelude::*;
use quarve::state::{Binding, Filterless, Store};
use quarve::view::control::Dropdown;
use quarve::view::scroll::ScrollView;
use quarve::view::text::{TextField, TextModifier};
use crate::IVP;

fn divider() -> impl IVP {
    PURPLE
        .frame(F.intrinsic(1,1).unlimited_width())
}

pub fn icpc_viewer() -> impl IVP {
    // let contest_type = Store::new(None);
    // let url = Store::new("".to_string());

    vstack()
        // .push(
        //     text("ICPC Live Scoreboard")
        //         .text_size(36)
        // )
        // .push(selector(contest_type.binding(), url.binding()))
        .push(divider())
        .push(scoreboard(1))
        .frame(F.unlimited_stretch())
        .text_color(WHITE)
        .bg_color(BLACK)
}

fn selector(
    contest_type: impl Binding<Filterless<Option<String>>> + Clone,
    url: impl Binding<Filterless<String>> + Clone,
) -> impl IVP {

    hstack()
        .push(
            text("Contest Type")
                .bold()
        )
        .push(
            Dropdown::new(contest_type.clone())
                .option("NAC")
                .option("CERC")
                .intrinsic(100, 22)
        )
        .push(
            text("Scoreboard URL:")
                .bold()
        )
        .push(
            TextField::new(url)
                .unstyled()
                .padding(2)
                .layer(L.border(LIGHT_GRAY, 1).radius(2))
                .intrinsic(300, 28)
        )
        .padding(5)
}

fn scoreboard(count: usize) -> impl IVP {
    // timer controls
    let elapsed_seconds = Store::new(0);

    let controls = hstack();

    // problem headers
    let problems = (0..count)
        .into_iter()
        .hmap(|i, s| {
            text(&"ABCDEFGHIJKLMNOPQRSTUVWXYZ"[*i ..*i + 1])
                .bold()
                .padding(1)
                // .layer(L.radius(2).border())
        })
        .border(RED, 1);

    vstack()
        .push(controls)
        .push(
            problems
                .offset(100, 0)
        )
        .push(
            ScrollView::vertical(
                vstack()
            )
        )
}
