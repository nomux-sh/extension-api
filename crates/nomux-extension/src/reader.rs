//! The thread that reads the messages from the host.
//!
//! A second thread is necessary because a method can run for a long time on
//! the thread of [`crate::Extension::run`]. During that time, this thread
//! answers health requests, records cancellations and gives the responses of
//! host services to the method that waits for them.

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::error::CallError;
use crate::frame::{self, FrameError};
use crate::host::Link;
use crate::wire::v1::envelope::Kind;
use crate::wire::v1::request::Body;
use crate::wire::v1::{CallRequest, HealthResponse, Request, Response, response};

/// The work of a request that the thread of the extension does.
pub(crate) enum Work {
    Initialize,
    Shutdown,
    Call(CallRequest),
}

pub(crate) enum Inbound {
    Request {
        id: u64,
        work: Work,
        cancelled: Arc<AtomicBool>,
    },
    Failed(FrameError),
}

/// Starts the thread. The receiver ends when the stream from the host ends.
pub(crate) fn spawn(mut input: impl Read + Send + 'static, link: Arc<Link>) -> Receiver<Inbound> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        if let Err(error) = read_all(&mut input, &link, &sender) {
            // The receiver is gone only after the extension stopped.
            let _ = sender.send(Inbound::Failed(error));
        }
        link.pending.close();
    });
    receiver
}

fn read_all(
    input: &mut impl Read,
    link: &Link,
    sender: &Sender<Inbound>,
) -> Result<(), FrameError> {
    while let Some(envelope) = frame::read_envelope(input)? {
        match envelope.kind {
            Some(Kind::Request(request)) => route(request, link, sender)?,
            Some(Kind::Response(response)) => link.pending.resolve(response),
            Some(Kind::Cancel(cancel)) => link.cancellations.cancel(cancel.id),
            // The host sends no events. A kind that this version does not
            // know comes from a newer host and is not an error.
            Some(Kind::Event(_)) | None => {}
        }
    }
    Ok(())
}

fn route(request: Request, link: &Link, sender: &Sender<Inbound>) -> Result<(), FrameError> {
    let work = match request.body {
        Some(Body::Initialize(_)) => Work::Initialize,
        Some(Body::Shutdown(_)) => Work::Shutdown,
        Some(Body::Call(call)) => Work::Call(call),
        Some(Body::Health(_)) => {
            let result = response::Result::Health(HealthResponse {});
            return respond(link, request.id, result);
        }
        Some(Body::Hello(_)) => {
            let error = CallError::failed("the handshake is already complete");
            return respond(link, request.id, response::Result::Error(error.into()));
        }
        None => {
            let error = CallError::failed("the extension does not know this request");
            return respond(link, request.id, response::Result::Error(error.into()));
        }
    };
    let cancelled = link.cancellations.track(request.id);
    // The receiver is gone only after the extension stopped.
    let _ = sender.send(Inbound::Request {
        id: request.id,
        work,
        cancelled,
    });
    Ok(())
}

fn respond(link: &Link, id: u64, result: response::Result) -> Result<(), FrameError> {
    link.outbound.send(Kind::Response(Response {
        id,
        result: Some(result),
    }))
}
