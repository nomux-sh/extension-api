//! Tests of the validator of `extension.toml`.

use nomux_extension::ApiVersion;
use nomux_extension::manifest::{Manifest, ManifestError};

/// The manifest of the configuration reference, with a scope.
const GIT: &str = r#"
[extension]
id = "nomux.git"
name = "Git"
version = "1.0.0"
api = "^1.0"

[daemon]
command = "bin/nomux-git"
lazy = true

[ui]
component = "ui/git.wasm"
slots = ["sidebar.section", "tab.kind:diff", "statusbar.item"]

[permissions]
fs = ["${worktree}"]
process = ["git"]
network = false
scopes = ["sessions:read"]
"#;

/// Changes one line of [`GIT`] and returns the problems of the result as
/// `path: problem` texts.
fn problems_with(line: &str, replacement: &str) -> Vec<String> {
    assert!(GIT.contains(line), "the manifest has no line `{line}`");
    problems_of(&GIT.replace(line, replacement))
}

fn problems_of(text: &str) -> Vec<String> {
    match text.parse::<Manifest>() {
        Err(ManifestError::Fields(problems)) => problems
            .into_iter()
            .map(|problem| format!("{}: {}", problem.path, problem.problem))
            .collect(),
        other => panic!("the manifest has no field problems: {other:?}"),
    }
}

/// Asserts that the change of one line gives one problem, at `path`.
#[track_caller]
fn assert_rejected(line: &str, replacement: &str, path: &str) {
    let problems = problems_with(line, replacement);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].starts_with(&format!("{path}: ")),
        "{problems:?}"
    );
}

#[test]
fn valid_manifest_gives_each_value() {
    let manifest: Manifest = GIT.parse().unwrap();

    assert_eq!(manifest.id, "nomux.git");
    assert_eq!(manifest.name, "Git");
    assert_eq!(manifest.version, semver::Version::new(1, 0, 0));
    assert_eq!(manifest.api.to_string(), "^1.0");
    let daemon = manifest.daemon.unwrap();
    assert_eq!(
        (daemon.command.as_str(), daemon.lazy),
        ("bin/nomux-git", true)
    );
    let ui = manifest.ui.unwrap();
    assert_eq!(ui.component, "ui/git.wasm");
    assert_eq!(
        ui.slots,
        ["sidebar.section", "tab.kind:diff", "statusbar.item"]
    );
    assert_eq!(manifest.permissions.fs, ["${worktree}"]);
    assert_eq!(manifest.permissions.process, ["git"]);
    assert!(!manifest.permissions.network);
    assert_eq!(manifest.permissions.scopes, ["sessions:read"]);
}

#[test]
fn manifest_with_only_the_extension_table_asks_for_no_permission() {
    let manifest: Manifest =
        "[extension]\nid = \"a.b\"\nname = \"B\"\nversion = \"0.1.0-beta.1\"\napi = \"^0.1\""
            .parse()
            .unwrap();

    assert_eq!(manifest.daemon, None);
    assert_eq!(manifest.ui, None);
    assert_eq!(manifest.permissions, Default::default());
    assert!(manifest.api.matches(ApiVersion {
        major: 0,
        minor: 1,
        patch: 4
    }));
}

#[test]
fn daemon_part_is_lazy_when_the_manifest_does_not_say() {
    let manifest: Manifest = GIT.replace("lazy = true\n", "").parse().unwrap();

    assert!(manifest.daemon.unwrap().lazy);
}

#[test]
fn identifier_must_have_a_publisher_and_a_name_in_lower_case() {
    for id in [
        "git",
        "Nomux.git",
        "nomux..git",
        "nomux.git.",
        "nomux.9git",
        "nomux git",
    ] {
        assert_rejected(
            "id = \"nomux.git\"",
            &format!("id = \"{id}\""),
            "extension.id",
        );
    }
}

#[test]
fn version_and_api_requirement_must_have_their_syntax() {
    assert_rejected(
        "version = \"1.0.0\"",
        "version = \"1.0\"",
        "extension.version",
    );
    for api in ["1.0", "^1.x", ">=1.0", "^1.0.0.0", "^01", "*", ""] {
        assert_rejected(
            "api = \"^1.0\"",
            &format!("api = \"{api}\""),
            "extension.api",
        );
    }
}

#[test]
fn command_and_component_must_stay_in_the_package() {
    for command in [
        "/usr/bin/git",
        "../git",
        "bin/../../git",
        "bin//git",
        "bin\\\\git",
        "",
    ] {
        assert_rejected(
            "command = \"bin/nomux-git\"",
            &format!("command = \"{command}\""),
            "daemon.command",
        );
    }
    assert_rejected(
        "component = \"ui/git.wasm\"",
        "component = \"/ui/git.wasm\"",
        "ui.component",
    );
}

#[test]
fn file_system_permission_must_be_absolute_or_below_the_worktree() {
    for path in [
        "src",
        "~/src",
        "${home}/src",
        "${worktree}/../other",
        "${worktree}src",
        "/var/${worktree}",
        "/var/./lib",
        "",
    ] {
        assert_rejected(
            "fs = [\"${worktree}\"]",
            &format!("fs = [\"${{worktree}}\", \"{path}\"]"),
            "permissions.fs[1]",
        );
    }

    let accepted = GIT.replace(
        "fs = [\"${worktree}\"]",
        "fs = [\"/\", \"/var/lib\", \"${worktree}/.git\"]",
    );
    assert_eq!(
        accepted.parse::<Manifest>().unwrap().permissions.fs.len(),
        3
    );
}

#[test]
fn process_scope_and_slot_items_must_have_their_syntax() {
    assert_rejected(
        "process = [\"git\"]",
        "process = [\"git\", \"/usr/bin/git\"]",
        "permissions.process[1]",
    );
    for scope in [
        "sessions",
        "sessions:",
        ":read",
        "Sessions:read",
        "sessions:read:all",
    ] {
        assert_rejected(
            "scopes = [\"sessions:read\"]",
            &format!("scopes = [\"{scope}\"]"),
            "permissions.scopes[0]",
        );
    }
    for slot in ["Sidebar", "tab.kind:", "tab..kind", "tab.kind:diff:more"] {
        assert_rejected(
            "slots = [\"sidebar.section\", \"tab.kind:diff\", \"statusbar.item\"]",
            &format!("slots = [\"sidebar.section\", \"{slot}\"]"),
            "ui.slots[1]",
        );
    }
}

#[test]
fn value_of_a_different_type_is_reported_at_its_path() {
    assert_rejected("network = false", "network = \"no\"", "permissions.network");
    assert_rejected(
        "process = [\"git\"]",
        "process = \"git\"",
        "permissions.process",
    );
    assert_rejected(
        "process = [\"git\"]",
        "process = [\"git\", 7]",
        "permissions.process[1]",
    );
    assert_rejected("name = \"Git\"", "name = 7", "extension.name");
    assert_eq!(problems_of("extension = 1"), ["extension: must be a table"]);
}

#[test]
fn key_that_is_not_known_is_reported_at_its_path() {
    assert_rejected("lazy = true", "lazzy = true", "daemon.lazzy");
    assert_eq!(
        problems_of(&format!("{GIT}\n[theme]\nname = \"x\"")),
        ["theme: is not a known key"]
    );
}

#[test]
fn all_problems_of_a_manifest_are_reported_together() {
    let problems = problems_of(
        "[extension]\nid = \"git\"\nversion = \"1.0.0\"\n\n[daemon]\nlazy = 1\n\n[permissions]\nfs = [\"src\"]",
    );

    let paths: Vec<&str> = problems
        .iter()
        .map(|problem| problem.split(':').next().unwrap())
        .collect();
    assert_eq!(
        paths,
        [
            "extension.id",
            "extension.name",
            "extension.api",
            "daemon.command",
            "daemon.lazy",
            "permissions.fs[0]"
        ]
    );
    assert_eq!(problems[1], "extension.name: is missing");
}

#[test]
fn manifest_without_the_extension_table_is_rejected() {
    assert_eq!(
        problems_of("[daemon]\ncommand = \"bin/x\""),
        ["extension: is missing"]
    );
}

#[test]
fn text_that_is_not_toml_is_a_syntax_error_with_the_line() {
    let error = "[extension]\nid = ".parse::<Manifest>().unwrap_err();

    let ManifestError::Syntax(message) = error else {
        panic!("the error is not a syntax error: {error:?}");
    };
    assert!(message.contains("line 2"), "{message}");
}
