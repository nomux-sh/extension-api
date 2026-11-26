//! The methods, events and payloads that the conformance extension of each
//! language supplies. `conformance/README.md` describes them.

/// The argument and the result of `echo` and `relay`, and the payload of the
/// host service `conformance.reverse`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Text {
    /// The text.
    #[prost(string, tag = "1")]
    pub text: String,
}

/// The payload of the event `echoed`.
#[derive(Clone, PartialEq, prost::Message)]
pub struct Echoed {
    /// The number of UTF-8 bytes of the text.
    #[prost(uint64, tag = "1")]
    pub length: u64,
}

/// Returns its argument and emits `echoed`. An empty text fails.
pub const ECHO: &str = "echo";
/// Emits `started`, then runs until the host cancels it.
pub const WAIT: &str = "wait";
/// Calls the host service `conformance.reverse` and returns its result.
pub const RELAY: &str = "relay";
/// The host service that `relay` calls.
pub const REVERSE: &str = "conformance.reverse";
/// The text for which `conformance.reverse` fails.
pub const DENIED_TEXT: &str = "deny";
/// The message of the failure of `conformance.reverse`.
pub const DENIED_MESSAGE: &str = "the extension has no scope for conformance.reverse";

/// The environment variable that gives the extension API version that the
/// extension asks for, for example `0.1.1`. The extension gives it to the
/// option of its SDK. Without the variable, the extension asks for the
/// default version of its SDK.
pub const REQUESTED_API: &str = "NOMUX_CONFORMANCE_API";

/// The event of `echo`.
pub const ECHOED: &str = "echoed";
/// The event of `wait`.
pub const STARTED: &str = "started";

impl Text {
    /// Makes a text.
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}
