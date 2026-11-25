//! The nomux extension SDK for the daemon part of an extension.
//!
//! The daemon part of an extension is a separate process on the Host. The
//! nomux daemon (the host) starts the process when a client needs it and
//! stops it when it is idle. The host and the extension exchange protobuf
//! messages on the standard input and the standard output of the process.
//!
//! An extension does these steps:
//!
//! 1. It makes an [`Extension`] with the identifier and the version of its
//!    `extension.toml`.
//! 2. It adds each method with [`Extension::method`]. The argument and the
//!    result of a method are protobuf messages.
//! 3. It calls [`Extension::run`]. The function does the handshake, runs the
//!    methods that the host calls and returns when the host tells the
//!    extension to stop.
//!
//! In a method, [`Call::host`] gives the [`Host`]. With it, the extension
//! emits events ([`Host::emit`]), calls host services ([`Host::call`]) and
//! reads the permissions that the user approved ([`Host::permissions`]).
//!
//! # Example
//!
//! An extension with one method and one event. Add `prost` to the
//! dependencies of the extension, with the same version as this crate uses:
//! the `prost::Message` derive macro needs it.
//!
//! ```no_run
//! use nomux_extension::{Call, CallError, Extension, RunError};
//!
//! /// The argument and the result of the `echo` method.
//! #[derive(Clone, PartialEq, prost::Message)]
//! struct Text {
//!     #[prost(string, tag = "1")]
//!     text: String,
//! }
//!
//! /// The payload of the `echoed` event.
//! #[derive(Clone, PartialEq, prost::Message)]
//! struct Echoed {
//!     #[prost(uint64, tag = "1")]
//!     length: u64,
//! }
//!
//! fn echo(call: &Call, input: Text) -> Result<Text, CallError> {
//!     if input.text.is_empty() {
//!         return Err(CallError::failed("the text is empty"));
//!     }
//!     let length = input.text.len() as u64;
//!     call.host().emit("echoed", &Echoed { length })?;
//!     Ok(input)
//! }
//!
//! fn main() -> Result<(), RunError> {
//!     Extension::new("nomux.echo", env!("CARGO_PKG_VERSION"))
//!         .method("echo", echo)
//!         .run()
//! }
//! ```
//!
//! # Threads and the async runtime
//!
//! The crate has no async runtime. An extension starts when a client needs
//! it and stops when it is idle, thus it must be small and start fast.
//! [`Extension::run`] blocks the thread that calls it and runs the methods on
//! that thread, one at a time. Thus a method does not need a lock for the
//! state of the extension.
//!
//! One more thread reads the messages from the host. While a method runs,
//! that thread answers health requests and records cancellations. A method
//! that can run for a long time examines [`Call::is_cancelled`] between its
//! steps.
//!
//! # Rules for an extension
//!
//! - The standard output carries the protocol. Do not print to it. Write
//!   logs to the standard error.
//! - A panic in a method stops the process. The nomux daemon continues to
//!   run and reports the failure of the extension.
//!
//! # Other parts of the crate
//!
//! - [`wire::v1`]: the messages of the protocol. `prost` generates these
//!   types from the schemas of the `extension-api` repository.
//! - [`frame`]: the framing. A host uses it to speak with an extension.
//! - `manifest` (feature `manifest`): the types and the validator of
//!   `extension.toml`. An extension does not need it.
//!
//! Versions earlier than 1.0.0 are pre-releases: interfaces can change
//! between minor versions.

mod api_version;
mod call;
mod cancellation;
mod error;
mod extension;
pub mod frame;
mod handshake;
mod host;
#[cfg(feature = "manifest")]
pub mod manifest;
mod outbound;
mod pending_calls;
mod reader;

pub use api_version::{ApiRequirement, ApiVersion, InvalidApiRequirement};
pub use call::Call;
pub use error::{CallError, RunError};
pub use extension::Extension;
pub use host::Host;
pub use wire::v1::{ErrorCode, Permissions};

/// Compiles the example in the crate README as a documentation test.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

/// The messages of the nomux extension protocol.
pub mod wire {
    /// Package `nomux.extension.v1`.
    ///
    /// `buf generate` writes the included file. Do not edit the file. The
    /// file is included with `include!`, thus `rustfmt` does not format it.
    pub mod v1 {
        include!("generated/nomux.extension.v1.rs");
    }
}
