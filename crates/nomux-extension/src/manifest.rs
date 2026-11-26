//! The manifest of an extension package: `extension.toml`.
//!
//! [`Manifest`] is the result of a successful validation. Parse the text of
//! the file to get it:
//!
//! ```
//! use nomux_extension::ApiVersion;
//! use nomux_extension::manifest::{Manifest, ManifestError};
//!
//! let manifest: Manifest = r#"
//! [extension]
//! id = "nomux.git"
//! name = "Git"
//! version = "1.0.0"
//! api = "^0.1"
//!
//! [daemon]
//! command = "bin/nomux-git"
//!
//! [permissions]
//! fs = ["${worktree}"]
//! process = ["git"]
//! "#
//! .parse()
//! .unwrap();
//! assert_eq!(manifest.id, "nomux.git");
//! assert!(manifest.api.matches(ApiVersion::CURRENT));
//!
//! let error = "[extension]\nid = \"Git\"".parse::<Manifest>().unwrap_err();
//! let ManifestError::Fields(problems) = error else { panic!() };
//! assert_eq!(problems[0].path, "extension.id");
//! ```
//!
//! The JSON Schema of the file is `schema/extension.schema.json` in the
//! `extension-api` repository. The schema and this module describe the same
//! rules.

mod adapter;
mod daemon;
mod error;
mod fields;
mod name;
mod path;
mod permissions;
mod ui;

use std::str::FromStr;

pub use adapter::AdapterPart;
pub use daemon::DaemonPart;
pub use error::{FieldError, ManifestError};
pub use permissions::PermissionRequest;
pub use ui::UiPart;

use crate::api_version::ApiRequirement;
use error::Problems;
use fields::Fields;

/// A valid `extension.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Manifest {
    /// The identifier of the extension, for example `nomux.git`.
    pub id: String,
    /// The name of the extension for the user.
    pub name: String,
    /// The version of the extension.
    pub version: semver::Version,
    /// The extension API versions that the extension can use. The host
    /// starts the extension only if [`ApiRequirement::matches`] is `true`
    /// for the extension API version of the host.
    pub api: ApiRequirement,
    /// The daemon part, if the package has one.
    pub daemon: Option<DaemonPart>,
    /// The UI part, if the package has one.
    pub ui: Option<UiPart>,
    /// The agent tools of which the extension is the adapter, if the
    /// manifest has an `[adapter]` table.
    pub adapter: Option<AdapterPart>,
    /// The permissions that the extension asks for.
    pub permissions: PermissionRequest,
}

impl FromStr for Manifest {
    type Err = ManifestError;

    /// Reads and validates the text of an `extension.toml`. The error lists
    /// all the fields that are not valid.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let table = text
            .parse()
            .map_err(|error: toml::de::Error| ManifestError::Syntax(error.to_string()))?;
        let mut problems = Problems::default();
        let mut root = Fields::root(table);

        let identity = root
            .required_table("extension", &mut problems)
            .and_then(|fields| Identity::read(fields, &mut problems));
        let daemon = root
            .table("daemon", &mut problems)
            .and_then(|fields| DaemonPart::read(fields, &mut problems));
        let ui = root
            .table("ui", &mut problems)
            .and_then(|fields| UiPart::read(fields, &mut problems));
        let adapter = root
            .table("adapter", &mut problems)
            .and_then(|fields| AdapterPart::read(fields, &mut problems));
        let permissions = root
            .table("permissions", &mut problems)
            .map(|fields| PermissionRequest::read(fields, &mut problems))
            .unwrap_or_default();
        root.finish(&mut problems);
        if let (Some(identity), Some(_)) = (&identity, &adapter) {
            adapter::require_api(identity.api, &mut problems);
        }

        let problems = problems.into_list();
        let (Some(identity), true) = (identity, problems.is_empty()) else {
            return Err(ManifestError::Fields(problems));
        };
        Ok(Self {
            id: identity.id,
            name: identity.name,
            version: identity.version,
            api: identity.api,
            daemon,
            ui,
            adapter,
            permissions,
        })
    }
}

/// The `[extension]` table.
struct Identity {
    id: String,
    name: String,
    version: semver::Version,
    api: ApiRequirement,
}

impl Identity {
    fn read(mut fields: Fields, problems: &mut Problems) -> Option<Self> {
        let id = fields.string_matching(
            "id",
            is_extension_id,
            "must have two or more parts with `.` between them, in 64 characters at most. \
             A part starts with a lower-case letter and has only lower-case letters, digits and `-`",
            problems,
        );
        let name = fields.string_matching(
            "name",
            |name| !name.trim().is_empty() && name.chars().count() <= 64,
            "must have 1 to 64 characters",
            problems,
        );
        let version = fields.string("version", problems).and_then(|text| {
            let version = text.parse();
            if let Err(error) = &version {
                let problem = format!("is not a version of Semantic Versioning 2.0.0: {error}");
                problems.add(fields.path("version"), problem);
            }
            version.ok()
        });
        let api = fields.string("api", problems).and_then(|text| {
            let api = text.parse();
            if api.is_err() {
                let problem = "must be a caret requirement: `^1`, `^1.2` or `^1.2.3`";
                problems.add(fields.path("api"), problem);
            }
            api.ok()
        });
        fields.finish(problems);
        Some(Self {
            id: id?,
            name: name?,
            version: version?,
            api: api?,
        })
    }
}

fn is_extension_id(id: &str) -> bool {
    id.len() <= 64 && id.contains('.') && name::is_dotted_name(id)
}
