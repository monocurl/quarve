use cc;

#[cfg(target_os = "macos")]
fn build() {
    println!("cargo:rerun-if-changed=macos");

    cc::Build::new()
        .file("macos/core.m")
        .file("macos/cursor_view.m")
        .compile("backend");
}

fn main() {
    /* dependencies */
    println!("cargo:rerun-if-changed=build.rs");

    build();
}

