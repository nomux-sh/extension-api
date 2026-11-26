//! The `[adapter]` table.

use super::error::Problems;
use super::fields::Fields;
use super::name::is_name;

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

fn is_tool_name(name: &str) -> bool {
    name.len() <= 64 && is_name(name)
}
