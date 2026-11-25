//! The `[daemon]` table.

use super::error::Problems;
use super::fields::Fields;
use super::path::{PACKAGE_PATH_RULE, is_package_path};

/// The daemon part of an extension package.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct DaemonPart {
    /// The program of the daemon part: a path in the package.
    pub command: String,
    /// `true` if the host starts the daemon part only when a client needs
    /// it. `false` if the host starts it together with the daemon. The
    /// default is `true`.
    pub lazy: bool,
}

impl DaemonPart {
    pub(crate) fn read(mut fields: Fields, problems: &mut Problems) -> Option<Self> {
        let command =
            fields.string_matching("command", is_package_path, PACKAGE_PATH_RULE, problems);
        let lazy = fields.boolean("lazy", true, problems);
        fields.finish(problems);
        Some(Self {
            command: command?,
            lazy,
        })
    }
}
