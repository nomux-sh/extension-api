package extension

import (
	"context"
	"fmt"
	"io"
	"os"
	"time"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// StopTimeout is the longest time that [Extension.Serve] waits for the
// running handler after the extension stopped for a reason other than a
// shutdown request. A handler that returns when its context is done does not
// make Serve wait.
const StopTimeout = 2 * time.Second

// Handler is a method that the host can call.
//
// It gets the payload of the call and returns the payload of the result. The
// method specifies the encoding of the two payloads. ctx is cancelled when
// the host cancels the call or when the extension stops.
//
// To fail the call with a code, return an [*Error]. A different error fails
// the call with [CodeFailed] and its text, or with [CodeCancelled] if ctx is
// done. If the payload is not valid, return [CodeInvalidArgument] before the
// method does its work.
type Handler func(ctx context.Context, host *Host, payload []byte) ([]byte, error)

// Initializer runs one time, after the handshake and before the first call.
type Initializer func(ctx context.Context, host *Host) error

// Extension is the daemon part of an extension.
//
// Add the handlers, then call [Extension.Run]. Do not add a handler after Run
// started.
type Extension struct {
	identity    identity
	handlers    map[string]Handler
	initializer Initializer
	initialized bool
}

// New starts the definition of an extension.
//
// id and version are the values of id and version in extension.toml. The
// host stops an extension whose id is different from its manifest.
func New(id, version string) *Extension {
	return &Extension{
		identity: identity{id: id, version: version, api: MinimumAPIVersion()},
		handlers: make(map[string]Handler),
	}
}

// RequireAPI states the lowest extension API version that the extension
// needs. The default is [MinimumAPIVersion]. State a later version only if the
// extension uses a feature of that version, and give api in extension.toml the
// same minimum: version 0.1.1 and api = "^0.1.1".
//
// The extension refuses a host whose version does not match the caret
// requirement of version. RequireAPI panics if the SDK does not implement
// version: if the caret requirement of version does not match [APIVersion].
func (e *Extension) RequireAPI(version Version) {
	if !version.accepts(APIVersion()) {
		panic(fmt.Sprintf("extension: the SDK implements the extension API version %s, not %s", APIVersion(), version))
	}
	e.identity.api = version
}

// Handle adds a method that the host can call.
//
// The handlers run one at a time, in the sequence in which the calls arrive.
// Thus a handler does not need a lock for the state of the extension. While a
// handler runs, the extension answers health requests and applies
// cancellations.
//
// Handle panics if the extension already has a method with this name.
func (e *Extension) Handle(method string, handler Handler) {
	if _, ok := e.handlers[method]; ok {
		panic("extension: the method `" + method + "` is added twice")
	}
	e.handlers[method] = handler
}

// OnInitialize sets a function that runs one time, after the handshake and
// before the first call. If it returns an error, the host gets the error and
// no handler runs.
func (e *Extension) OnInitialize(initializer Initializer) {
	e.initializer = initializer
}

// Run serves the host on the standard input and the standard output of the
// process. It is [Extension.Serve] with [os.Stdin] and [os.Stdout].
//
// The standard output carries the protocol. Write logs to the standard error.
func (e *Extension) Run(ctx context.Context) error {
	return e.Serve(ctx, os.Stdin, os.Stdout)
}

// Serve serves a host on the given streams. Use it to test an extension in
// the process of the test.
//
// Serve returns nil after the host sent a shutdown request and got the
// response: then the process exits with the status 0. Serve returns an error
// when the extension must stop for a different reason: the error wraps
// [ErrFrame], [ErrNoHello], [ErrIncompatibleAPI] or [ErrDisconnected], or it
// is the cause of ctx. Then the process exits with a status that is not 0.
//
// Serve returns only when no handler runs: the handlers before a shutdown
// request returned before its response. When the extension stops for a
// different reason, Serve cancels the context of each handler and waits for
// the running handler for [StopTimeout] at most.
//
// When Serve returns, it closes input if input is an [io.Closer]. Thus the
// goroutine that reads input ends.
func (e *Extension) Serve(ctx context.Context, input io.Reader, output io.Writer) error {
	if closer, ok := input.(io.Closer); ok {
		defer closer.Close()
	}
	connection := newConnection(ctx, e, input, output)
	go connection.read()
	worked := make(chan struct{})
	go func() {
		defer close(worked)
		connection.work()
	}()

	select {
	case <-connection.stopped:
	case <-ctx.Done():
		connection.stop(context.Cause(ctx))
	}
	timer := time.NewTimer(StopTimeout)
	defer timer.Stop()
	select {
	case <-worked:
	case <-timer.C:
	}
	return connection.failure
}

// answer does the work of one request on the worker goroutine.
func (e *Extension) answer(ctx context.Context, host *Host, request *wire.Request) *wire.Response {
	switch body := request.GetBody().(type) {
	case *wire.Request_Initialize:
		if err := e.initialize(ctx, host); err != nil {
			return errorResponse(ctx, err)
		}
		return &wire.Response{Result: &wire.Response_Initialize{Initialize: &wire.InitializeResponse{}}}
	case *wire.Request_Call:
		payload, err := e.call(ctx, host, body.Call)
		if err != nil {
			return errorResponse(ctx, err)
		}
		return &wire.Response{Result: &wire.Response_Call{Call: &wire.CallResponse{Payload: payload}}}
	default:
		return &wire.Response{Result: &wire.Response_Shutdown{Shutdown: &wire.ShutdownResponse{}}}
	}
}

func (e *Extension) initialize(ctx context.Context, host *Host) error {
	if e.initialized {
		return Errorf(CodeFailed, "the extension is already initialized")
	}
	initializer := e.initializer
	e.initializer = nil
	if initializer != nil {
		if err := initializer(ctx, host); err != nil {
			return err
		}
	}
	e.initialized = true
	return nil
}

func (e *Extension) call(ctx context.Context, host *Host, request *wire.CallRequest) ([]byte, error) {
	if !e.initialized {
		return nil, Errorf(CodeFailed, "the extension is not initialized")
	}
	if ctx.Err() != nil {
		return nil, cancelled()
	}
	handler, ok := e.handlers[request.GetMethod()]
	if !ok {
		return nil, Errorf(CodeNotFound, "the extension has no method `%s`", request.GetMethod())
	}
	return handler(ctx, host, request.GetPayload())
}

func errorResponse(ctx context.Context, err error) *wire.Response {
	return &wire.Response{Result: &wire.Response_Error{Error: callError(ctx, err).toWire()}}
}
