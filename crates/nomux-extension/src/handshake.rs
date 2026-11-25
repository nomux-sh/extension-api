//! The extension side of the handshake.

use std::io::Read;

use crate::api_version::{ApiRequirement, ApiVersion};
use crate::error::RunError;
use crate::frame;
use crate::outbound::Outbound;
use crate::wire::v1::envelope::Kind;
use crate::wire::v1::request::Body;
use crate::wire::v1::{Error, ErrorCode, Permissions, Request, Response, Welcome, response};

/// Reads `Hello`, applies the version rule and answers. The result is the
/// permissions that the host gave.
pub(crate) fn accept(
    input: &mut impl Read,
    outbound: &Outbound,
    extension_id: &str,
    extension_version: &str,
) -> Result<Permissions, RunError> {
    let envelope = frame::read_envelope(input)?.ok_or(RunError::Disconnected)?;
    let Some(Kind::Request(Request {
        id,
        body: Some(Body::Hello(hello)),
    })) = envelope.kind
    else {
        return Err(RunError::NoHello);
    };

    let host = hello.api_version.unwrap_or_default();
    let requirement = ApiRequirement::from(ApiVersion::CURRENT);
    let is_compatible = requirement.matches(host);
    let result = if is_compatible {
        response::Result::Welcome(Welcome {
            api_version: Some(ApiVersion::CURRENT),
            extension_id: extension_id.to_owned(),
            extension_version: extension_version.to_owned(),
        })
    } else {
        response::Result::Error(Error {
            code: ErrorCode::VersionIncompatible.into(),
            message: format!(
                "the extension needs the extension API version {requirement}, but the host supplies {host}"
            ),
        })
    };
    outbound.send(Kind::Response(Response {
        id,
        result: Some(result),
    }))?;

    if !is_compatible {
        return Err(RunError::IncompatibleApi { host });
    }
    Ok(hello.permissions.unwrap_or_default())
}
