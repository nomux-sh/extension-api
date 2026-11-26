//! The `[adapter]` table.

use super::error::Problems;
use super::fields::Fields;
use super::name::is_name;
use crate::api_version::{ApiRequirement, ApiVersion};

/// The agent tools whose hook events an extension takes: the extension is
/// the adapter of these tools.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct AdapterPart {
    /// The names of the tools, for example `testtool`. A name starts with a
    /// lower-case letter and has only lower-case letters, digits and `-`,
    /// in 64 characters at most.
    pub tools: Vec<String>,
}

impl AdapterPart {
    pub(crate) fn read(mut fields: Fields, problems: &mut Problems) -> Option<Self> {
        let tools = fields.strings_matching(
            "tools",
            is_tool_name,
            "must be a tool name: a lower-case letter, then lower-case letters, digits and `-`, \
             in 64 characters at most",
            problems,
        );
        fields.finish(problems);
        Some(Self { tools })
    }
}

/// The first extension API version that has the `[adapter]` table.
const FIRST_API_VERSION: ApiVersion = ApiVersion {
    major: 0,
    minor: 1,
    patch: 1,
};

/// Adds a problem if `api` accepts a host that does not know the table.
pub(crate) fn require_api(api: ApiRequirement, problems: &mut Problems) {
    let minimum = api.minimum();
    let first = FIRST_API_VERSION;
    if (minimum.major, minimum.minor, minimum.patch) < (first.major, first.minor, first.patch) {
        let problem = format!(
            "must be `^{first}` or later: the `[adapter]` table needs the extension API version {first}"
        );
        problems.add("extension.api".to_owned(), problem);
    }
}

fn is_tool_name(name: &str) -> bool {
    name.len() <= 64 && is_name(name)
}
