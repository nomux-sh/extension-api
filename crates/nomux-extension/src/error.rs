//! The errors of a call and of a run.

use std::fmt;

use crate::api_version::ApiVersion;
use crate::frame::FrameError;
use crate::wire::v1::{self, ErrorCode};

/// The failure of one call.
///
/// A method of an extension returns it, and the host gets the code and the
/// message. A call of a host service returns it when the host refuses or
/// fails the call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallError {
    /// The reason for the failure.
    pub code: ErrorCode,
    /// A message in English for the user.
    pub message: String,
}

impl CallError {
    /// Makes an error with a code and a message.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Makes an error with the code [`ErrorCode::Failed`].
    pub fn failed(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Failed, message)
    }

    /// Makes the error that a method returns when it stops because
    /// [`crate::Call::is_cancelled`] is `true`.
    pub fn cancelled() -> Self {
        Self::new(ErrorCode::Cancelled, "the call was cancelled")
    }

    pub(crate) fn disconnected() -> Self {
        Self::failed("the host closed the connection")
    }
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CallError {}

impl From<v1::Error> for CallError {
    fn from(error: v1::Error) -> Self {
        // A code that is not set or not known is a failure with no other
        // reason.
        let code = match ErrorCode::try_from(error.code) {
            Ok(ErrorCode::Unspecified) | Err(_) => ErrorCode::Failed,
            Ok(code) => code,
        };
        Self::new(code, error.message)
    }
}

impl From<CallError> for v1::Error {
    fn from(error: CallError) -> Self {
        Self {
            code: error.code.into(),
            message: error.message,
        }
    }
}

/// The reason why [`crate::Extension::run`] stopped before the host sent a
/// shutdown request.
#[derive(Debug)]
pub enum RunError {
    /// A frame was not read or written.
    Frame(FrameError),
    /// The first message from the host was not `Hello`.
    NoHello,
    /// The host does not supply the extension API version that this crate
    /// needs.
    IncompatibleApi {
        /// The extension API version of the host.
        host: ApiVersion,
    },
    /// The host closed the stream and did not send a shutdown request.
    Disconnected,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => error.fmt(f),
            Self::NoHello => f.write_str("the first message from the host was not `Hello`"),
            Self::IncompatibleApi { host } => write!(
                f,
                "the host supplies the extension API version {host}, but this extension needs {}",
                crate::ApiRequirement::from(ApiVersion::CURRENT)
            ),
            Self::Disconnected => {
                f.write_str("the host closed the stream and did not send a shutdown request")
            }
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::NoHello | Self::IncompatibleApi { .. } | Self::Disconnected => None,
        }
    }
}

impl From<FrameError> for RunError {
    fn from(error: FrameError) -> Self {
        Self::Frame(error)
    }
}
