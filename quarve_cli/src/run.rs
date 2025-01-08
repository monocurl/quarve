pub(crate) use run::*;

#[cfg(target_os = "macos")]
mod run {
    use std::fs::remove_dir_all;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    use crate::util::cargo_util::{find_name, find_path};
    use crate::util::file_util::copy_directory;

    const QT_FRAMEWORKS: [&'static str; 4] = ["QtGui", "QtCore", "QtWidgets", "QtDBus"];

    pub(crate) fn platform_run(name_hint: Option<&str>, release: bool) {
        let root = find_path(".");
        let Some(name) = find_name(name_hint) else {
            eprintln!("Could not find binary named '{}'", name_hint.unwrap());
            return
        };

        let mut source = root.clone();
        source.push("target");
        if release {
            source.push("release");
        } else {
            source.push("debug");
        }
        source.push(&name);

        let mut quarve_target = root.clone();
        quarve_target.push("quarve_target");

        quarve_target.push(format!("{}.app", name));
        let _ = remove_dir_all(&quarve_target);

        quarve_target.push("Contents");
        attach_binary(&name, &source, &mut quarve_target);
        attach_resources(root, &mut quarve_target);
        attach_info_plist(&name, &mut quarve_target);
        quarve_target.pop();

        let binary = quarve_target
            .join("Contents")
            .join("MacOS")
            .join(name);
        attach_qt(release, &binary, &mut quarve_target);

        /* run app */
        if !Command::new("open")
            .arg(quarve_target)
            .status()
            .expect("Unable to open application")
            .success() {
            return
        }
    }

    // quarve target expected to be at root .app directory
    fn attach_qt(release: bool, binary: &Path, quarve_target: &mut PathBuf) {
        /* qt, if provided */
        // TODO this should only attach based
        // on the qt_backend feature not on the env var
        if let Ok(qt_path) = std::env::var("QUARVE_BACKEND_PATH") {
            if release {
                // copy relevant frameworks
                let mut src_path = PathBuf::from(qt_path.clone());
                src_path.push("lib");

                quarve_target.push("Contents");
                quarve_target.push("Frameworks");
                for framework in QT_FRAMEWORKS {
                    let name =  framework.to_string() + ".framework";
                    src_path.push(&name);
                    quarve_target.push(&name);

                    copy_directory(&src_path, quarve_target)
                        .expect("Unable to copy qt frameworks");

                    quarve_target.pop();
                    src_path.pop();
                }
                quarve_target.pop();
                quarve_target.pop();

                assert!(Command::new("install_name_tool")
                            .arg("-add_rpath")
                            .arg("@executable_path/../Frameworks")
                            .arg(&binary)
                            .stderr(Stdio::null())
                            .spawn().expect("Unable to add Qt dependencies")
                            .wait().expect("Unable to add Qt dependencies").success(),
                        "Unable to add Qt dependencies");
            } else {
                // use install name tool to add dependency
                // if failed, it generally means that it already has
                // the path and we can ignore
                // FIXME improve htis

                let _ = Command::new("install_name_tool")
                    .arg("-add_rpath")
                    .arg(qt_path)
                    .arg(&binary)
                    .stderr(Stdio::null())
                    .spawn().expect("Unable to add Qt dependencies")
                    .wait().expect("Unable to add Qt dependencies");
            }
        }
    }

    fn attach_binary(name: &String, source: &PathBuf, quarve_target: &mut PathBuf) {
        /* Binary */
        quarve_target.push("MacOS");
        std::fs::create_dir_all(&quarve_target).unwrap();
        {
            quarve_target.push(&name);
            std::fs::copy(source, &quarve_target).unwrap();
            quarve_target.pop();
        }
        quarve_target.pop();
    }

    fn attach_resources(root: PathBuf, quarve_target: &mut PathBuf) {
        /* Assets */
        quarve_target.push("Resources");
        std::fs::create_dir_all(&quarve_target).unwrap();

        let mut source = root.clone();
        source.push("res");
        std::fs::create_dir_all(&source).unwrap();
        copy_directory(&source, &quarve_target).unwrap();

        quarve_target.pop();
    }

    fn attach_info_plist(name: &String, quarve_target: &mut PathBuf) {
        /* Info.plist */
        quarve_target.push("Info.plist");
        let contents = format!("
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple Computer//DTD PLIST 1.0//EN\" \"https://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
    <key>CFBundleExecutable</key>
    <string>{name}</string>
    <key>CFBundleGetInfoString</key>
    <string>{name}</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
</dict>
</plist>
", name=&name);

        std::fs::write(&quarve_target, contents).expect("Error writing Info.plist");
        quarve_target.pop();
    }
}

#[cfg(target_os = "linux")]
mod run {
    pub(crate) fn platform_run(name_hint: Option<&str>, release: bool) {}
}

#[cfg(target_os = "windows")]
mod run {
    use std::fs;
    use std::fs::{create_dir_all, remove_dir_all};
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::Command;

    use crate::util::cargo_util::{find_name, find_path};
    use crate::util::file_util::copy_directory;

    const QT_FRAMEWORKS: [&'static str; 3] = ["Qt6Gui", "Qt6Core", "Qt6Widgets"];
    const QT_PLUGINS: [&'static str; 1] = ["qwindows"];
    const WINDOWS_CREATE_NO_WINDOW: u32 = 0x08000000;


    pub(crate) fn platform_run(name_hint: Option<&str>, release: bool) {
        let root = find_path(".");
        let Some(name) = find_name(name_hint) else {
            eprintln!("Could not find binary named '{}'", name_hint.unwrap());
            return
        };

        let mut source = root.clone();
        source.push("target");
        if release {
            source.push("release");
        } else {
            source.push("debug");
        }
        source.push(name.clone() + ".exe");

        let mut quarve_target = root.clone();
        quarve_target.push("quarve_target");

        quarve_target.push(&name);
        let _ = remove_dir_all(&quarve_target);
        create_dir_all(&quarve_target).expect("Unable to create output directory");

        attach_binary(&name, &mut source, &mut quarve_target);
        attach_resources(root, &mut quarve_target);

        // also package qt dll's
        attach_qt(&mut quarve_target);

        // launch
        Command::new(quarve_target.join(&name))
            .creation_flags(WINDOWS_CREATE_NO_WINDOW)
            .spawn()
            .expect("Unable to launch application");
    }

    fn attach_qt(quarve_target: &mut PathBuf) {
        let base_path = std::env::var("QUARVE_BACKEND_PATH").expect("Please set the QUARVE_BACKEND_PATH to the Qt installation");
        let mut base_path = PathBuf::from(base_path);
        base_path.push("bin");

        for framework in QT_FRAMEWORKS {
            let name = framework.to_string() + ".dll";
            base_path.push(&name);
            quarve_target.push(&name);

            fs::copy(&base_path, &quarve_target)
                .expect("Unable to copy Qt dlls");

            base_path.pop();
            quarve_target.pop();
        }

        base_path.pop();
        base_path.push("plugins");
        base_path.push("platforms");
        quarve_target.push("platforms");
        create_dir_all(&quarve_target).unwrap();
        for plugin in QT_PLUGINS {
            let name = plugin.to_string() + ".dll";
            base_path.push(&name);
            quarve_target.push(&name);

            fs::copy(&base_path, &quarve_target)
                .expect("Unable to copy Qt dlls");

            base_path.pop();
            quarve_target.pop();
        }
        base_path.pop();
        quarve_target.pop();

    }

    fn attach_binary(name: &str, source: &mut PathBuf, quarve_target: &mut PathBuf) {
        quarve_target.push(format!("{}.exe", name));
        fs::copy(&source, &quarve_target)
            .expect("Unable to copy binary");
        quarve_target.pop();
    }

    fn attach_resources(root: PathBuf, quarve_target: &mut PathBuf) {
        /* Assets */
        quarve_target.push("res");
        create_dir_all(&quarve_target).unwrap();

        let mut source = root.clone();
        source.push("res");
        create_dir_all(&source).unwrap();
        copy_directory(&source, &quarve_target).unwrap();

        quarve_target.pop();
    }
}

