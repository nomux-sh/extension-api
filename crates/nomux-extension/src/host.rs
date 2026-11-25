//! The host, as an extension sees it.

use std::sync::Arc;

use prost::Message;

use crate::cancellation::Cancellations;
use crate::error::CallError;
use crate::outbound::Outbound;
use crate::pending_calls::PendingCalls;
use crate::wire::v1::envelope::Kind;
use crate::wire::v1::request::Body;
use crate::wire::v1::{CallRequest, ErrorCode, Event, Permissions, Request, response};

/// The state of one connection that the threads of the extension share.
pub(crate) struct Link {
    pub(crate) outbound: Outbound,
    pub(crate) pending: PendingCalls,
    pub(crate) cancellations: Cancellations,
}

/// The nomux daemon that started the extension.
///
/// A method gets the host from [`crate::Call::host`]. A clone is cheap, and a
/// clone can move to a different thread, for example to emit events from a
/// thread that watches files.
#[derive(Clone)]
pub struct Host {
    link: Arc<Link>,
    permissions: Arc<Permissions>,
}

impl Host {
    pub(crate) fn new(link: Arc<Link>, permissions: Permissions) -> Self {
        Self {
            link,
            permissions: Arc::new(permissions),
        }
    }

    /// The permissions that the user approved for the extension.
    pub fn permissions(&self) -> &Permissions {
        &self.permissions
    }

    /// Calls a method of a host service and waits for the result.
    ///
    /// `method` has the form `<service>.<method>`, for example
    /// `sessions.list`. The host fails the call with
    /// [`ErrorCode::PermissionDenied`] if the extension does not have the
    /// scope of the service.
    pub fn call<Input, Output>(&self, method: &str, input: &Input) -> Result<Output, CallError>
    where
        Input: Message,
        Output: Message + Default,
    {
        let (id, reply) = self
            .link
            .pending
            .register()
            .ok_or_else(CallError::disconnected)?;
        let body = Body::Call(CallRequest {
            method: method.to_owned(),
            payload: input.encode_to_vec(),
        });
        let request = Request {
            id,
            body: Some(body),
        };
        if let Err(error) = self.link.outbound.send(Kind::Request(request)) {
            self.link.pending.forget(id);
            return Err(CallError::failed(format!(
                "the call of `{method}` was not sent: {error}"
            )));
        }
        let response = reply.recv().map_err(|_| CallError::disconnected())?;
        match response.result {
            Some(response::Result::Call(reply)) => Output::decode(reply.payload.as_slice())
                .map_err(|error| {
                    CallError::new(
                        ErrorCode::InvalidArgument,
                        format!("the result of `{method}` is not valid: {error}"),
                    )
                }),
            Some(response::Result::Error(error)) => Err(error.into()),
            _ => Err(CallError::failed(format!(
                "the host did not answer `{method}` with a call result"
            ))),
        }
    }

    /// Sends an event to the host.
    ///
    /// The host adds the identifier of the extension to the name.
    pub fn emit(&self, name: &str, payload: &impl Message) -> Result<(), CallError> {
        let event = Event {
            name: name.to_owned(),
            payload: payload.encode_to_vec(),
        };
        self.link
            .outbound
            .send(Kind::Event(event))
            .map_err(|error| CallError::failed(format!("the event `{name}` was not sent: {error}")))
    }
}
