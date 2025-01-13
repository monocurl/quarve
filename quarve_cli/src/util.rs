pub mod file_util {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::path::Path;
    use std::{fs, io};

    // https://stackoverflow.com/questions/26958489/how-to-copy-a-folder-recursively-in-rust
    pub(crate) fn copy_directory(src: &Path, dst: &Path) -> io::Result<()> {
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                copy_directory(&entry.path(), &dst.join(entry.file_name()))?;
            } else if ty.is_file() {
                fs::copy(entry.path(), dst.join(entry.file_name()))?;
            }
            // ignore symlink (which are encountered sometimes in QtGui.Framework)
        }
        Ok(())
    }


    pub(crate) fn append(name: &str, to: &Path, contents: &str) {
        let mut toml = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(Path::new(name).join(to))
            .expect("Could not locate project");

        writeln!(toml, "{}", contents)
            .expect("Could not write to project");
    }

    pub(crate) fn set(name: &str, path: &Path, contents: &str) {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(Path::new(name).join(path))
            .expect("Could not locate project");

        writeln!(file, "{}", contents)
            .expect("Could not write to project");
    }
}

pub mod cargo_util {
    use crate::util::file_util::{append, set};
    use serde_json::Value;
    use std::env::VarError;
    use std::fs::create_dir_all;
    use std::path::{Path, PathBuf};
    use std::process::Command as Process;

    pub(crate) fn new(name: &str) {
        let init = Process::new("cargo")
            .arg("new")
            .arg(name)
            .status().expect("Failed to execute cargo");

        if !init.success() {
            return
        }

        // for local builds
        let quarve_dep =
            if std::env::var("QUARVE_DEV") != Err(VarError::NotPresent) {
                "quarve = { path = \"../../quarve\" }\n"
            } else {
                "quarve = { version = \"0.1.0\" }\n"
            };

        append(name, Path::new("Cargo.toml"), quarve_dep);
        append(name, Path::new(".gitignore"), "quarve_target\n");
        set(name, &Path::new("src").join("main.rs"),
            include_str!("template.rs.txt"));

        let path = find_path(name);
        create_dir_all(path.join("res").join("font"))
            .expect("Unable to create resource directory");
    }

    pub(crate) fn find_path(current_dir: &str) -> PathBuf {
        let root = Process::new("cargo")
            .arg("locate-project")
            .arg("--workspace")
            .arg("--message-format")
            .arg("plain")
            .current_dir(Path::new(current_dir))
            .output()
            .expect("Failed to execute cargo");

        let str = String::from_utf8(root.stdout).expect("UTF-8 error");
        Path::new(&str).parent()
            .expect("Unexpected cargo location")
            .to_owned()
    }

    pub(crate) fn find_name(name_hint: Option<&str>) -> Option<String> {
        let meta = Process::new("cargo")
            .arg("metadata")
            .arg("--no-deps")
            .arg("--format-version=1")
            .output()
            .expect("Failed to execute cargo");

        let str = String::from_utf8(meta.stdout).expect("UTF-8 error");
        let json: Value = serde_json::from_str(&str).unwrap();
        let map = json.as_object().unwrap();

        if let Some(hint) = name_hint {
            let found = map.get("packages").unwrap()
                .as_array().unwrap().iter()
                .any(|p|
                    p.as_object().unwrap()
                        .get("targets").unwrap()
                        .as_array().unwrap().iter()
                        .any(|t| {
                            t.as_object().unwrap()
                                .get("name").unwrap()
                                .as_str().unwrap() == hint
                        })
                );

            if found {
                Some(hint.into())
            } else {
                None
            }
        } else {
            Some(map.get("packages").unwrap()
                .as_array().unwrap()[0]
                .as_object().unwrap()
                .get("targets").unwrap()
                .as_array().unwrap()[0]
                .get("name").unwrap()
                .as_str().unwrap().to_owned())
        }
    }
}
