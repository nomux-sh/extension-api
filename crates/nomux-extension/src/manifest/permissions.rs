//! The `[permissions]` table.

use super::error::Problems;
use super::fields::Fields;
use super::name::is_name;
use super::path::has_plain_parts;

/// The variable that the host replaces with the directory of the worktree.
const WORKTREE: &str = "${worktree}";

/// The permissions that an extension asks for. The user approves them at
/// installation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PermissionRequest {
    /// The directories that the extension reads and writes. An item is an
    /// absolute path, or it starts with `${worktree}`.
    pub fs: Vec<String>,
    /// The programs that the extension starts. An item is a program name
    /// without a directory.
    pub process: Vec<String>,
    /// `true` if the extension opens network connections. The default is
    /// `false`.
    pub network: bool,
    /// The scopes of host services that the extension uses, for example
    /// `sessions:read`. The validator examines only the syntax: the list of
    /// scopes is a part of the host services.
    pub scopes: Vec<String>,
}

impl PermissionRequest {
    pub(crate) fn read(mut fields: Fields, problems: &mut Problems) -> Self {
        let fs = fields.strings_matching(
            "fs",
            is_fs_path,
            "must be an absolute path or start with `${worktree}`, \
             without `.` or `..` parts and without other variables",
            problems,
        );
        let process = fields.strings_matching(
            "process",
            is_program_name,
            "must be a program name without a directory and without spaces",
            problems,
        );
        let network = fields.boolean("network", false, problems);
        let scopes = fields.strings_matching(
            "scopes",
            is_scope,
            "must be a scope: a service name, `:` and an access name, for example `sessions:read`",
            problems,
        );
        fields.finish(problems);
        Self {
            fs,
            process,
            network,
            scopes,
        }
    }
}

fn is_fs_path(path: &str) -> bool {
    if path == "/" || path == WORKTREE {
        return true;
    }
    let below_worktree = path.strip_prefix(WORKTREE).unwrap_or(path);
    !below_worktree.contains("${")
        && below_worktree
            .strip_prefix('/')
            .is_some_and(has_plain_parts)
}

fn is_program_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(|character: char| "/\\".contains(character) || character.is_whitespace())
}

fn is_scope(scope: &str) -> bool {
    scope
        .split_once(':')
        .is_some_and(|(service, access)| is_name(service) && is_name(access))
}
