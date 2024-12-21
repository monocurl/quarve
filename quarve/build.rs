use cc;

#[cfg(all(target_os="macos", not(quarve_backend_qt)))]
fn build() {
    println!("cargo:rerun-if-changed=macos");

    cc::Build::new()
        .file("macos/core.m")
        .file("macos/cursor_view.m")
        .file("macos/image_view.m")
        .file("macos/layer_view.m")
        .file("macos/layout_view.m")
        .file("macos/scroll_view.m")
        .file("macos/button_view.m")
        .file("macos/dropdown_view.m")
        .file("macos/menu.m")
        .file("macos/message_box.m")
        .file("macos/file_picker.m")
        .file("macos/text.m")
        .file("macos/path.m")
        .compile("backend");
}

#[cfg(any(not(target_os="macos"), quarve_backend_qt))]
fn build() {
    println!("cargo:rerun-if-changed=qt");

    cc::Build::new()
        .file("qt/core.m")
        .file("qt/cursor_view.m")
        .file("qt/image_view.m")
        .file("qt/layer_view.m")
        .file("qt/layout_view.m")
        .file("qt/scroll_view.m")
        .file("qt/button_view.m")
        .file("qt/dropdown_view.m")
        .file("qt/menu.m")
        .file("qt/message_box.m")
        .file("qt/file_picker.m")
        .file("qt/text.m")
        .file("qt/path.m")
        .compile("backend");
}

fn main() {
    /* dependencies */
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=inc");

    build();
}

