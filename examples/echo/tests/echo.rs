//! Runs the conformance suite against the echo extension as a child process,
//! as the nomux daemon runs it.

use std::ffi::OsString;
use std::path::Path;

use nomux_conformance::{CASES, Target};
use nomux_extension::ApiVersion;
use nomux_extension::manifest::Manifest;

const PROGRAM: &str = env!("CARGO_BIN_EXE_nomux-echo");

fn target() -> Target {
    let manifest: Manifest = include_str!("../extension.toml").parse().unwrap();
    Target {
        command: vec![OsString::from(PROGRAM)],
        manifest,
        show_stderr: false,
    }
}

#[test]
fn manifest_names_the_program_and_matches_the_api_version() {
    let manifest = target().manifest;
    let command = manifest.daemon.as_ref().unwrap().command.as_str();
    assert_eq!(
        Path::new(command).file_name(),
        Path::new(PROGRAM).file_name()
    );
    assert!(manifest.api.matches(ApiVersion::CURRENT));
}

#[test]
fn echo_extension_passes_each_conformance_case() {
    let target = target();
    let failures: Vec<String> = CASES
        .iter()
        .filter_map(|case| {
            case.run(&target)
                .err()
                .map(|reason| format!("{}: {reason}", case.name))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
