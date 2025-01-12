use crate::IVP;
use quarve::prelude::*;
use quarve::state::NumericAction;
use quarve::view::color_view::EmptyView;
use quarve::view::image_view::ImageView;
use quarve::view::portal::{Portal, PortalReceiver, PortalSendable};
use quarve::view::text::TextField;
use quarve::view::util::Color;

fn profile(name: &str, image: &str, color: Color) -> impl IVP {
    hstack()
        .push(
            ImageView::new(&image)
                .intrinsic(30, 30)
                .padding(5)
        )
        .push(text(name))
        .padding(5)
        .layer(L.bg_color(color).radius(4))
}

pub fn basic_portal() -> impl IVP {
    // create the communication channel
    let p = Portal::new();

    ivp_using(|env, s| {

    })

    vstack()
        .push(
            // mount the contents on this portal
            // in this example the sender and receiver
            // are in the same function so there's little benefit
            // but in theory they can be very far apart
            PortalReceiver::new(&p)
        )
        .push(
            RED.intrinsic(100, 100)
        )
        .push(
            // send a blue view as the content
            // Note that this sender is active only
            // whenever this view is shown
            // not important in this example
            // but sometimes it can be useful
            GREEN
                .intrinsic(100, 100)
                .portal_send(&p, BLUE.intrinsic(100, 100))
        )
}

pub fn dynamic_portal(s: MSlock) -> impl IVP {
    let p = Portal::new();

    let counter = Store::new(0);
    // imagine more complex conditions for yourself
    let left = counter.map(|c| *c % 2 == 1, s);
    let right = counter.map(|c| *c % 2 == 0, s);

    let text = Store::new("".to_string());

    hstack()
        .push(
            view_if(left, PortalReceiver::new(&p))
                .view_else(BLACK.intrinsic(100, 30))
        )
        .push(
            button("switch", move |s| {
                counter.apply(NumericAction::Incr(1), s)
            })
        )
        .push(
            view_if(right, PortalReceiver::new(&p))
                .view_else(BLACK.intrinsic(100, 30))
        )
        .push(
            // empty view is nice for portal sending
            // since it will never have children
            EmptyView
                .portal_send(&p, TextField::new(text.binding())
                    .border(RED, 1)
                    .intrinsic(100, 30)
                )
        )
}