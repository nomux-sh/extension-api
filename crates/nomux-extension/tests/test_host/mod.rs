//! A minimal host for tests. It speaks the extension protocol with an
//! extension on two streams. The real host is the nomux daemon.

// Each test crate that includes this module uses a different part of it.
#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::io::{BufReader, Read, Write};

use nomux_extension::wire::v1::envelope::Kind;
use nomux_extension::wire::v1::request::Body;
use nomux_extension::wire::v1::{
    CallRequest, CallResponse, Envelope, Error, ErrorCode, Event, Hello, InitializeRequest,
    InitializeResponse, Permissions, Request, Response, response,
};
use nomux_extension::{ApiVersion, frame};
use prost::Message;

pub struct TestHost<FromExtension: Read, ToExtension: Write> {
    input: BufReader<FromExtension>,
    output: ToExtension,
    last_id: u64,
    responses: HashMap<u64, response::Result>,
    events: VecDeque<Event>,
    /// Answers the calls of host services. The default refuses each call.
    pub service: Box<dyn FnMut(CallRequest) -> response::Result>,
}

impl<FromExtension: Read, ToExtension: Write> TestHost<FromExtension, ToExtension> {
    pub fn new(from_extension: FromExtension, to_extension: ToExtension) -> Self {
        Self {
            input: BufReader::new(from_extension),
            output: to_extension,
            last_id: 0,
            responses: HashMap::new(),
            events: VecDeque::new(),
            service: Box::new(|request| {
                response::Result::Error(Error {
                    code: ErrorCode::NotFound.into(),
                    message: format!("the host has no method `{}`", request.method),
                })
            }),
        }
    }

    /// The stream to the extension, for bytes that are not a valid frame.
    pub fn output(&mut self) -> &mut ToExtension {
        &mut self.output
    }

    pub fn send(&mut self, kind: Kind) {
        frame::write_envelope(&mut self.output, &Envelope { kind: Some(kind) }).unwrap_or_else(
            |error| panic!("the message to the extension was not written: {error}"),
        );
    }

    /// Sends a request and does not wait for the response.
    pub fn start_request(&mut self, body: Body) -> u64 {
        self.last_id += 1;
        self.send(Kind::Request(Request {
            id: self.last_id,
            body: Some(body),
        }));
        self.last_id
    }

    pub fn wait_response(&mut self, id: u64) -> response::Result {
        loop {
            if let Some(result) = self.responses.remove(&id) {
                return result;
            }
            self.read();
        }
    }

    pub fn request(&mut self, body: Body) -> response::Result {
        let id = self.start_request(body);
        self.wait_response(id)
    }

    pub fn next_event(&mut self) -> Event {
        loop {
            if let Some(event) = self.events.pop_front() {
                return event;
            }
            self.read();
        }
    }

    /// The number of events that were read and not taken yet.
    pub fn pending_events(&self) -> usize {
        self.events.len()
    }

    pub fn hello(&mut self, api_version: ApiVersion, permissions: Permissions) -> response::Result {
        self.request(Body::Hello(Hello {
            api_version: Some(api_version),
            extension_id: String::new(),
            permissions: Some(permissions),
        }))
    }

    /// Does the handshake and the initialization.
    pub fn start(&mut self) {
        let welcome = self.hello(ApiVersion::CURRENT, Permissions::default());
        assert!(
            matches!(welcome, response::Result::Welcome(_)),
            "{welcome:?}"
        );
        let initialized = self.request(Body::Initialize(InitializeRequest {}));
        assert_eq!(
            initialized,
            response::Result::Initialize(InitializeResponse {})
        );
    }

    pub fn start_call(&mut self, method: &str, input: &impl Message) -> u64 {
        self.start_request(Body::Call(CallRequest {
            method: method.to_owned(),
            payload: input.encode_to_vec(),
        }))
    }

    pub fn wait_call<Reply: Message + Default>(&mut self, id: u64) -> Result<Reply, Error> {
        match self.wait_response(id) {
            response::Result::Call(reply) => Ok(Reply::decode(reply.payload.as_slice())
                .unwrap_or_else(|error| panic!("the result of the call is not valid: {error}"))),
            response::Result::Error(error) => Err(error),
            other => panic!("the response is not a call result: {other:?}"),
        }
    }

    pub fn call<Reply: Message + Default>(
        &mut self,
        method: &str,
        input: &impl Message,
    ) -> Result<Reply, Error> {
        let id = self.start_call(method, input);
        self.wait_call(id)
    }

    /// Reads one message from the extension and files it.
    fn read(&mut self) {
        let envelope = frame::read_envelope(&mut self.input)
            .unwrap_or_else(|error| panic!("the extension sent a frame that is not valid: {error}"))
            .expect("the extension closed its output");
        match envelope.kind {
            Some(Kind::Response(Response {
                id,
                result: Some(result),
            })) => {
                self.responses.insert(id, result);
            }
            Some(Kind::Event(event)) => self.events.push_back(event),
            Some(Kind::Request(Request {
                id,
                body: Some(Body::Call(call)),
            })) => {
                let result = (self.service)(call);
                self.send(Kind::Response(Response {
                    id,
                    result: Some(result),
                }));
            }
            other => panic!("the extension sent a message that is not valid: {other:?}"),
        }
    }
}

/// The result of a host service that succeeds.
pub fn call_result(output: &impl Message) -> response::Result {
    response::Result::Call(CallResponse {
        payload: output.encode_to_vec(),
    })
}
