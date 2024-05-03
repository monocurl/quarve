use quarve;
use quarve::prelude::*;

struct Channels {

}

impl ChannelProvider for Channels {

}

struct ApplicationProvider {

}

struct WindowProvider {

}

impl quarve::prelude::ApplicationProvider for ApplicationProvider {
    type ApplicationChannels = Channels;

    fn channels(&self) -> Self::ApplicationChannels {
        Channels {

        }
    }

    fn will_spawn(&self, app: AppHandle<Self>, s: &Slock<MainThreadMarker>) {
        app.spawn_window(WindowProvider {

        }, s);
    }
}

impl quarve::prelude::WindowProvider for WindowProvider {
    type WindowChannels = Channels;

    fn channels(&self) -> Self::WindowChannels {
        Channels {

        }
    }

    fn title(&self, s: &Slock<MainThreadMarker>) {
        todo!()
    }

    fn style(&self, s: &Slock<MainThreadMarker>) {
        todo!()
    }

    fn menu_bar(&self, s: &Slock<MainThreadMarker>) {
        todo!()
    }

    fn tree(&self, s: &Slock<MainThreadMarker>) {
        todo!()
    }

    fn can_close(&self, s: &Slock<MainThreadMarker>) -> bool {
        true
    }
}

fn main() {
    launch(ApplicationProvider {

    })
}
