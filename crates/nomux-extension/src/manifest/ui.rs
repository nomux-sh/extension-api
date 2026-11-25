//! The `[ui]` table.

use super::error::Problems;
use super::fields::Fields;
use super::name::{is_dotted_name, is_name};
use super::path::{PACKAGE_PATH_RULE, is_package_path};

/// The UI part of an extension package.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct UiPart {
    /// The WASM component of the UI part: a path in the package.
    pub component: String,
    /// The slots that the UI part fills, for example `sidebar.section` or
    /// `tab.kind:diff`. The validator examines only the syntax: the list of
    /// slots is a part of the UI API.
    pub slots: Vec<String>,
}

impl UiPart {
    pub(crate) fn read(mut fields: Fields, problems: &mut Problems) -> Option<Self> {
        let component =
            fields.string_matching("component", is_package_path, PACKAGE_PATH_RULE, problems);
        let slots = fields.strings_matching(
            "slots",
            is_slot,
            "must be a slot: names with `.` between them, then an optional `:` and a name, \
             for example `tab.kind:diff`",
            problems,
        );
        fields.finish(problems);
        Some(Self {
            component: component?,
            slots,
        })
    }
}

fn is_slot(slot: &str) -> bool {
    match slot.split_once(':') {
        Some((place, kind)) => is_dotted_name(place) && is_name(kind),
        None => is_dotted_name(slot),
    }
}
