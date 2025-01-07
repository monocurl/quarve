use crate::run::platform_run;
use crate::util::cargo_util::find_path;
use clap::{arg, Command};
use std::fs::remove_dir_all;
use std::ops::Deref;
use util::cargo_util::new;

mod util;
mod run;

fn run(name_hint: Option<&str>, release: bool) {
    let mut build = std::process::Command::new("cargo");
    build.arg("build");
    build.arg("--all");
    build.env("RUSTFLAGS", "--cfg quarve_managed_run");

    if release {
        build.arg("--release");
    }

    let status = build.status();

    if !status.expect("Failed to execute cargo").success() {
        return
    }

    platform_run(name_hint, release);
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
        )
        .subcommand(
            Command::new("clean")
                .about("Clear quarve target")
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
        Some(("clean", _)) => {
            let mut root = find_path(".");
            root.push("quarve_target");
            // don't worry about errors
            let _ = remove_dir_all(&root);
        }
        _ => {
            unreachable!()
        }
    }
}
