use quarve;

extern "C" {
    fn launch_window();
}


fn main() {
    unsafe {
        launch_window();
    }
}