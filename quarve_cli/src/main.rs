mod util;

use std::env::VarError;
use std::fs::OpenOptions;
use std::io::Write;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::{Command as Process};
use clap::{Command, arg};
use serde_json::Value;
use crate::util::copy_directory;

fn append(name: &str, to: &str, contents: &str) {
    let mut toml = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(format!("{}/{}", name, to))
        .expect("Could not locate project");

    writeln!(toml, "{}", contents)
        .expect("Could not write to project");
}
fn new(name: &str) {
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
        }
        else {
            "quarve = { version = \"0.1.0\" }\n"
        };

    append(name, "Cargo.toml", quarve_dep);

    append(name, ".gitignore", "quarve_target\n")
}

fn find_path() -> PathBuf {
    let root = Process::new("cargo")
        .arg("locate-project")
        .arg("--workspace")
        .arg("--message-format")
        .arg("plain")
        .output()
        .expect("Failed to execute cargo");

    let str = String::from_utf8(root.stdout).expect("UTF-8 error");
    Path::new(&str).parent()
        .expect("Unexpected cargo location")
        .to_owned()
}

fn find_name(name_hint: Option<&str>) -> Option<String> {
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
        }
        else {
            None
        }
    }
    else {
        Some(map.get("packages").unwrap()
            .as_array().unwrap()[0]
            .as_object().unwrap()
            .get("targets").unwrap()
            .as_array().unwrap()[0]
            .get("name").unwrap()
            .as_str().unwrap().to_owned())
    }
}

#[cfg(target_os = "macos")]
fn platform_run(name_hint: Option<&str>, release: bool)  {
    let root = find_path();
    let Some(name) = find_name(name_hint) else {
        eprintln!("Could not find binary named '{}'", name_hint.unwrap());
        return
    };

    let mut source = root.clone();
    if release {
        source.push("target/release/");
    }
    else {
        source.push("target/debug/");
    }
    source.push(&name);

    let mut quarve_target = root.clone();
    quarve_target.push("quarve_target");

    quarve_target.push(format!("{}.app", name));
    quarve_target.push("Contents");

    /* Binary */
    {
        quarve_target.push("MacOS");
        std::fs::create_dir_all(&quarve_target).unwrap();
        {
            quarve_target.push(&name);
            std::fs::copy(source, &quarve_target).unwrap();
            quarve_target.pop();
        }
        quarve_target.pop();
    }

    /* Assets */
    {
        quarve_target.push("Resources");
        if Path::exists(&quarve_target) {
            std::fs::remove_dir_all(&quarve_target).unwrap();
        }
        std::fs::create_dir_all(&quarve_target).unwrap();

        let mut source = root.clone();
        source.push("res");
        copy_directory(&source, &quarve_target).unwrap();

        quarve_target.pop();
    }

    /* Info.plist */
    {
        quarve_target.push("Info.plist");
        let contents = format!("
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple Computer//DTD PLIST 1.0//EN\" \"https://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
    <key>CFBundleExecutable</key>
    <string>{}</string>
    <key>CFBundleGetInfoString</key>
    <string>{}</string>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
</dict>
</plist>
", &name, &name, &name);

        std::fs::write(&quarve_target, contents).expect("Error writing Info.plist");
        quarve_target.pop();
    }

    quarve_target.pop();

    /* run app */
    if !Process::new("open")
        .arg(quarve_target)
        .status()
        .expect("Unable to open application")
        .success() {
        return
    }
}

#[cfg(target_os = "linux")]
fn platform_run(name_hint: Option<&str>, release: bool) {

}

#[cfg(target_os = "windows")]
fn platform_run(name_hint: Option<&str>, release: bool) {

}

fn run(name_hint: Option<&str>, release: bool) {
    let mut build = Process::new("cargo")
        .arg("build")
        .arg("--all")
        .env("RUSTFLAGS", "--cfg quarve_managed_run");

    if release {
        build = build.arg("--release");
    }

    let status = build.status();

    if !status.expect("Failed to execute cargo").success() {
        return
    }

    platform_run(name_hint);
}

#[cfg(target_os = "macos")]
fn platform_deploy() -> Result<(), ()> {
    /* copy binary */
    Ok(())
}

#[cfg(target_os = "linux")]
fn platform_deploy() -> Result<()> {

}

#[cfg(target_os = "windows")]
fn platform_deploy() -> Result<()> {

}

fn deploy(name_hint: Option<&str>) {
    run(name_hint, true)
}

fn main() {
    let c = Command::new("quarve")
        .about("Utilities for running and deploying quarve applications.")
        .subcommand_required(true)
        .subcommand(
            Command::new("new")
                .about("Create a new quarve project")
                .arg(arg!(<NAME> "The name of the project to create"))
                .arg_required_else_help(true)
        )
        .subcommand(
            Command::new("run")
                .arg(arg!(-n --name <NAME> "Explicitly specifies the name of the app to run"))
                .about("Run an existing quarve project for development")
        )
        .subcommand(
            Command::new("deploy")
                .arg(arg!(-n --name <NAME> "Explicitly specifies the name of the app to deploy"))
                .about("Build a quarve project for release")
        );

    match c.get_matches().subcommand() {
        Some(("new", submatches)) => {
            let name = submatches.get_one::<String>("NAME")
                .expect("name is required");

            new(name)
        },
        Some(("run", submatches)) => {
            run(submatches.get_one::<String>("name")
                .map(|s| s.deref()),
                false
            )
        },
        Some(("deploy", submatches)) => {
            deploy(submatches.get_one::<String>("name")
                    .map(|s| s.deref())
            )
        },
        _ => {
            unreachable!()
        }
    }
}
