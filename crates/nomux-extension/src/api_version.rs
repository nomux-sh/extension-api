//! The extension API version and the version rule.
//!
//! The rule is specified in `spec/extension-protocol.md`.

use std::fmt;
use std::str::FromStr;

pub use crate::wire::v1::ApiVersion;

impl ApiVersion {
    /// The extension API version that this crate implements. It is equal to
    /// the version of the crate.
    pub const CURRENT: Self = Self {
        major: 0,
        minor: 1,
        patch: 0,
    };
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The range of extension API versions that an extension can use.
///
/// The text form is a caret requirement: `^1`, `^1.2` or `^1.2.3`. It is the
/// value of `api` in `extension.toml`. The range has the same meaning as a
/// caret requirement of Cargo:
///
/// ```
/// use nomux_extension::{ApiRequirement, ApiVersion};
///
/// let requirement: ApiRequirement = "^1.2".parse().unwrap();
/// let version = |major, minor, patch| ApiVersion { major, minor, patch };
/// assert!(requirement.matches(version(1, 2, 0)));
/// assert!(requirement.matches(version(1, 9, 4)));
/// assert!(!requirement.matches(version(1, 1, 9)));
/// assert!(!requirement.matches(version(2, 0, 0)));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApiRequirement {
    major: u32,
    minor: Option<u32>,
    patch: Option<u32>,
}

impl ApiRequirement {
    /// Returns `true` if an extension with this requirement can use a host
    /// that supplies `version`.
    pub fn matches(&self, version: ApiVersion) -> bool {
        let minimum = (self.major, self.minor.unwrap_or(0), self.patch.unwrap_or(0));
        // The first part that is not 0 sets the limit. Before version 1.0.0,
        // a minor version can have breaking changes.
        let limit = match (self.major, self.minor, self.patch) {
            (0, None, _) => (1, 0, 0),
            (0, Some(0), None) => (0, 1, 0),
            (0, Some(0), Some(patch)) => (0, 0, patch.saturating_add(1)),
            (0, Some(minor), _) => (0, minor.saturating_add(1), 0),
            (major, _, _) => (major.saturating_add(1), 0, 0),
        };
        let version = (version.major, version.minor, version.patch);
        minimum <= version && version < limit
    }
}

/// The requirement of an extension that was built for `version`.
impl From<ApiVersion> for ApiRequirement {
    fn from(version: ApiVersion) -> Self {
        Self {
            major: version.major,
            minor: Some(version.minor),
            patch: Some(version.patch),
        }
    }
}

impl fmt::Display for ApiRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "^{}", self.major)?;
        for part in [self.minor, self.patch].into_iter().flatten() {
            write!(f, ".{part}")?;
        }
        Ok(())
    }
}

/// The text is not a caret requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidApiRequirement;

impl fmt::Display for InvalidApiRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an API requirement has the form `^1`, `^1.2` or `^1.2.3`")
    }
}

impl std::error::Error for InvalidApiRequirement {}

impl FromStr for ApiRequirement {
    type Err = InvalidApiRequirement;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let numbers = text.strip_prefix('^').ok_or(InvalidApiRequirement)?;
        let mut parts = numbers.split('.').map(parse_part);
        let major = parts.next().ok_or(InvalidApiRequirement)??;
        let minor = parts.next().transpose()?;
        let patch = parts.next().transpose()?;
        if parts.next().is_some() {
            return Err(InvalidApiRequirement);
        }
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

fn parse_part(part: &str) -> Result<u32, InvalidApiRequirement> {
    let is_canonical = part == "0" || !part.starts_with('0');
    if !is_canonical || !part.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(InvalidApiRequirement);
    }
    part.parse().map_err(|_| InvalidApiRequirement)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(major: u32, minor: u32, patch: u32) -> ApiVersion {
        ApiVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn current_version_is_the_version_of_the_crate() {
        assert_eq!(ApiVersion::CURRENT.to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn requirement_matches_from_its_version_to_the_next_breaking_version() {
        // Requirement, the smallest version that matches, the first version
        // after the range.
        let ranges = [
            ("^1", version(1, 0, 0), version(2, 0, 0)),
            ("^1.2", version(1, 2, 0), version(2, 0, 0)),
            ("^1.2.3", version(1, 2, 3), version(2, 0, 0)),
            ("^0", version(0, 0, 0), version(1, 0, 0)),
            ("^0.1", version(0, 1, 0), version(0, 2, 0)),
            ("^0.1.2", version(0, 1, 2), version(0, 2, 0)),
            ("^0.0", version(0, 0, 0), version(0, 1, 0)),
            ("^0.0.3", version(0, 0, 3), version(0, 0, 4)),
        ];
        for (text, first, limit) in ranges {
            let requirement: ApiRequirement = text.parse().unwrap();
            let before_first = match first {
                ApiVersion {
                    major,
                    minor: 0,
                    patch: 0,
                } => major.checked_sub(1).map(|major| version(major, 9, 9)),
                ApiVersion {
                    major,
                    minor,
                    patch: 0,
                } => Some(version(major, minor - 1, 9)),
                ApiVersion {
                    major,
                    minor,
                    patch,
                } => Some(version(major, minor, patch - 1)),
            };
            let before_limit = match limit {
                ApiVersion {
                    major,
                    minor: 0,
                    patch: 0,
                } => version(major - 1, 9, 9),
                ApiVersion {
                    major,
                    minor,
                    patch: 0,
                } => version(major, minor - 1, 9),
                ApiVersion {
                    major,
                    minor,
                    patch,
                } => version(major, minor, patch - 1),
            };

            assert!(requirement.matches(first), "{text} and {first}");
            assert!(
                requirement.matches(before_limit),
                "{text} and {before_limit}"
            );
            assert!(!requirement.matches(limit), "{text} and {limit}");
            if let Some(before_first) = before_first {
                assert!(
                    !requirement.matches(before_first),
                    "{text} and {before_first}"
                );
            }
            assert_eq!(requirement.to_string(), text);
        }
    }

    #[test]
    fn requirement_of_a_built_extension_starts_at_its_version() {
        let requirement = ApiRequirement::from(version(1, 2, 3));

        assert_eq!(requirement.to_string(), "^1.2.3");
        assert!(!requirement.matches(version(1, 2, 2)));
        assert!(requirement.matches(version(1, 3, 0)));
    }

    #[test]
    fn text_that_is_not_a_caret_requirement_is_refused() {
        for text in [
            "",
            "^",
            "1.2",
            "^1.",
            "^.1",
            "^1.2.3.4",
            "^01",
            "^1.x",
            "^-1",
            "^+1",
            "^ 1",
            "^4294967296",
            "~1.2",
            ">=1",
        ] {
            assert_eq!(
                text.parse::<ApiRequirement>(),
                Err(InvalidApiRequirement),
                "{text:?}"
            );
        }
    }
}
