//! The daemon part of an extension: its methods and its run loop.

use std::collections::HashMap;
use std::io::{self, BufReader, Read, Write};
use std::sync::Arc;

use prost::Message;

use crate::call::Call;
use crate::cancellation::Cancellations;
use crate::error::{CallError, RunError};
use crate::frame::FrameError;
use crate::handshake;
use crate::host::{Host, Link};
use crate::outbound::Outbound;
use crate::pending_calls::PendingCalls;
use crate::reader::{self, Inbound, Work};
use crate::wire::v1::envelope::Kind;
use crate::wire::v1::{
    CallRequest, CallResponse, ErrorCode, InitializeResponse, Response, ShutdownResponse, response,
};

type Method = Box<dyn FnMut(&Call, &[u8]) -> Result<Vec<u8>, CallError>>;
type Initialize = Box<dyn FnOnce(&Host) -> Result<(), CallError>>;

/// The daemon part of an extension.
///
/// Register the methods, then call [`Extension::run`]. The example is in the
/// [crate documentation](crate).
pub struct Extension {
    id: String,
    version: String,
    methods: HashMap<String, Method>,
    initialize: Option<Initialize>,
    is_initialized: bool,
}

impl Extension {
    /// Starts the definition of an extension.
    ///
    /// `id` and `version` are the values of `id` and `version` in
    /// `extension.toml`. The host stops an extension whose `id` is different
    /// from its manifest.
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            methods: HashMap::new(),
            initialize: None,
            is_initialized: false,
        }
    }

    /// Adds a method that the host can call.
    ///
    /// The argument and the result are protobuf messages. If the payload of a
    /// call is not an encoded `Input`, the host gets
    /// [`ErrorCode::InvalidArgument`] and the function does not run.
    ///
    /// The methods run one at a time, on the thread of [`Extension::run`].
    ///
    /// # Panics
    ///
    /// Panics if the extension already has a method with this name.
    pub fn method<Input, Output, Function>(mut self, name: &str, mut function: Function) -> Self
    where
        Input: Message + Default,
        Output: Message,
        Function: FnMut(&Call, Input) -> Result<Output, CallError> + 'static,
    {
        let method: Method = Box::new(move |call, payload| {
            let input = Input::decode(payload).map_err(|error| {
                CallError::new(
                    ErrorCode::InvalidArgument,
                    format!("the payload is not valid: {error}"),
                )
            })?;
            function(call, input).map(|output| output.encode_to_vec())
        });
        let previous = self.methods.insert(name.to_owned(), method);
        assert!(previous.is_none(), "the method `{name}` is added twice");
        self
    }

    /// Sets a function that runs one time, after the handshake and before the
    /// first call. If the function fails, the host gets the error and no
    /// method runs.
    pub fn on_initialize(
        mut self,
        function: impl FnOnce(&Host) -> Result<(), CallError> + 'static,
    ) -> Self {
        self.initialize = Some(Box::new(function));
        self
    }

    /// Serves the host on the standard input and the standard output of the
    /// process, until the host sends a shutdown request.
    ///
    /// The standard output carries the protocol. Write logs to the standard
    /// error.
    pub fn run(self) -> Result<(), RunError> {
        self.serve(io::stdin(), io::stdout())
    }

    /// Serves a host on the given streams, until the host sends a shutdown
    /// request. Use it to test an extension in the process of the test.
    pub fn serve(
        mut self,
        input: impl Read + Send + 'static,
        output: impl Write + Send + 'static,
    ) -> Result<(), RunError> {
        let mut input = BufReader::new(input);
        let outbound = Outbound::new(output);
        let permissions = handshake::accept(&mut input, &outbound, &self.id, &self.version)?;
        let link = Arc::new(Link {
            outbound,
            pending: PendingCalls::new(),
            cancellations: Cancellations::new(),
        });
        let host = Host::new(Arc::clone(&link), permissions);

        for inbound in reader::spawn(input, Arc::clone(&link)) {
            let (id, work, cancelled) = match inbound {
                Inbound::Request {
                    id,
                    work,
                    cancelled,
                } => (id, work, cancelled),
                Inbound::Failed(error) => return Err(error.into()),
            };
            let is_shutdown = matches!(work, Work::Shutdown);
            let call = Call {
                host: host.clone(),
                cancelled,
            };
            let result = self.answer(work, &call);
            link.cancellations.forget(id);
            respond(&link.outbound, id, result)?;
            if is_shutdown {
                return Ok(());
            }
        }
        Err(RunError::Disconnected)
    }

    fn answer(&mut self, work: Work, call: &Call) -> Result<response::Result, CallError> {
        match work {
            Work::Initialize => self.initialize(call.host()),
            Work::Shutdown => Ok(response::Result::Shutdown(ShutdownResponse {})),
            Work::Call(request) => self.call(request, call),
        }
    }

    fn initialize(&mut self, host: &Host) -> Result<response::Result, CallError> {
        if self.is_initialized {
            return Err(CallError::failed("the extension is already initialized"));
        }
        if let Some(initialize) = self.initialize.take() {
            initialize(host)?;
        }
        self.is_initialized = true;
        Ok(response::Result::Initialize(InitializeResponse {}))
    }

    fn call(&mut self, request: CallRequest, call: &Call) -> Result<response::Result, CallError> {
        if !self.is_initialized {
            return Err(CallError::failed("the extension is not initialized"));
        }
        if call.is_cancelled() {
            return Err(CallError::cancelled());
        }
        let method = self.methods.get_mut(&request.method).ok_or_else(|| {
            CallError::new(
                ErrorCode::NotFound,
                format!("the extension has no method `{}`", request.method),
            )
        })?;
        let payload = method(call, &request.payload)?;
        Ok(response::Result::Call(CallResponse { payload }))
    }
}

fn respond(
    outbound: &Outbound,
    id: u64,
    result: Result<response::Result, CallError>,
) -> Result<(), FrameError> {
    let result = result.unwrap_or_else(|error| response::Result::Error(error.into()));
    let sent = outbound.send(Kind::Response(Response {
        id,
        result: Some(result),
    }));
    // A result that is too large is the failure of one call. The connection
    // stays open.
    let Err(error @ FrameError::TooLarge { .. }) = sent else {
        return sent;
    };
    outbound.send(Kind::Response(Response {
        id,
        result: Some(response::Result::Error(
            CallError::failed(error.to_string()).into(),
        )),
    }))
}
