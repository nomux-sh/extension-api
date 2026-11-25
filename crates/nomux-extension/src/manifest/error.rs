//! The errors of a manifest.

use std::fmt;

/// One field of the manifest that is not valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldError {
    /// The path of the field, for example `extension.id` or
    /// `permissions.fs[0]`.
    pub path: String,
    /// The rule that the field does not obey. The text completes the
    /// sentence that starts with the path.
    pub problem: String,
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "`{}` {}", self.path, self.problem)
    }
}

/// The reason why a text is not a valid manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    /// The text is not TOML. The message has the line and the column.
    Syntax(String),
    /// The fields that are not valid, in the sequence of the tables
    /// `extension`, `daemon`, `ui` and `permissions`.
    Fields(Vec<FieldError>),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(message) => write!(f, "the manifest is not TOML: {message}"),
            Self::Fields(problems) => {
                f.write_str("the manifest is not valid:")?;
                for problem in problems {
                    write!(f, "\n  {problem}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ManifestError {}

/// Collects the fields that are not valid during one validation.
#[derive(Default)]
pub(crate) struct Problems(Vec<FieldError>);

impl Problems {
    pub(crate) fn add(&mut self, path: String, problem: impl Into<String>) {
        self.0.push(FieldError {
            path,
            problem: problem.into(),
        });
    }

    pub(crate) fn into_list(self) -> Vec<FieldError> {
        self.0
    }
}
