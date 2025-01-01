#[cfg(any(not(target_os="macos"), feature = "qt_backend"))]
use std::{path::PathBuf};
#[cfg(all(target_os = "macos", feature = "qt_backend"))]
use std::process::{Command, Stdio};

use cc;

#[cfg(all(target_os="macos", not(feature = "qt_backend")))]
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

    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=UniformTypeIdentifiers");
}

#[cfg(any(not(target_os="macos"), feature = "qt_backend"))]
fn build() {
    println!("cargo:rerun-if-changed=qt");

    let qt_path = PathBuf::from(std::env::var("QUARVE_BACKEND_PATH")
        .expect("The QUARVE_BACKEND_PATH should be set to the location of Qt's libraries"));
    let qt_frameworks = ["QtGui", "QtCore", "QtWidgets", "QtDBus"];

    let mut build = cc::Build::new();

    build
        .cpp(true)
        .file("qt/core.cpp")
        .file("qt/cursor_view.cpp")
        .file("qt/image_view.cpp")
        .file("qt/layer_view.cpp")
        .file("qt/layout_view.cpp")
        .file("qt/scroll_view.cpp")
        .file("qt/button_view.cpp")
        .file("qt/dropdown_view.cpp")
        .file("qt/menu.cpp")
        .file("qt/message_box.cpp")
        .file("qt/file_picker.cpp")
        .file("qt/text.cpp")
        .file("qt/path.cpp");

    // include qt directories
    #[cfg(target_os = "macos")]
    {
        let path = Command::new("xcrun")
            .arg("--show-sdk-path")
            .stdout(Stdio::piped())
            .output()
            .expect("Unable to launch xcrun");
        let isysroot = String::from_utf8(path.stdout).unwrap();
        build.flag("-isysroot");
        // ignore trailing new line
        build.flag(&isysroot[0..isysroot.len() - 1]);

        // add the framework
        build.flag("-F");
        build.flag(&qt_path.join("lib").to_str().unwrap());

        // set c++standard
        build.flag("--std=c++17");

        for framework in qt_frameworks {
            let headers =
                qt_path.join(format!("lib/{}.framework/Headers", framework));
            build.include(&headers);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let headers = qt_path.join("include");
        build.include(&headers);

        for framework in qt_frameworks {
            let headers =
                qt_path.join(format!("include\\{}", framework));
            build.include(&headers);
        }

        let headers = qt_path.join("mkspecs\\win32-msvc");
        build.include(&headers);

        // use c++17
        build.flag("-permissive-");
        build.flag("-std:c++17");
        build.flag("-Zc:__cplusplus");
    }

    build.compile("backend");

    // link to framework/libraries
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-search=framework={}", qt_path.join("lib").to_str().expect("Invalid backend path"));

        for framework in qt_frameworks {
            println!("cargo:rustc-link-lib=framework={}", framework);
        }
    }

    #[cfg(target_os = "windows")]
    {
        let qt_libs = ["Qt6Widgets", "Qt6Gui", "Qt6Core"];

        println!("cargo:rustc-link-search={}", qt_path.join("lib").to_str().expect("Invalid backend path"));

        for lib in qt_libs {
            println!("cargo:rustc-link-lib={}", lib);
        }
    }
}


fn main() {
    /* dependencies */
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=inc");
    println!("cargo:rerun-if-env-changed=QUARVE_BACKEND_PATH");

    build();
}
