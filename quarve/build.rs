use cc;

#[cfg(target_os="macos")]
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

#[cfg(not(target_os="macos"))]
fn build() {

}

fn main() {
    /* dependencies */
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=inc");

    println!("cargo::rustc-check-cfg=cfg(quarve_managed_run)");

    build();
}

