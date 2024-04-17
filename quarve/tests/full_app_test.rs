use quarve;

struct TestChannels {

}

impl quarve::ChannelProvider for TestChannels {

}

extern "C" {
    fn launch_window();
}

#[test]
fn full_app_test() {
    unsafe {
        launch_window();
    }
    quarve::launch(TestChannels { });
}
