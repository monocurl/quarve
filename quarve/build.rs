use cc;

fn main() {
    /* dependencies */
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=macos");

    cc::Build::new()
        .file("macos/backend.m")
        .compile("backend");

    /* linker arguments */
    println!("cargo:rustc-link-arg=-framework");
    println!("cargo:rustc-link-arg=Foundation");
    println!("cargo:rustc-link-arg=-framework");
    println!("cargo:rustc-link-arg=AppKit");
}
