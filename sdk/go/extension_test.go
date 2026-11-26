package extension

import (
	"bufio"
	"context"
	"errors"
	"io"
	"reflect"
	"testing"
	"testing/iotest"
	"time"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// The cases of the conformance suite run against examples/echo-go. These
// tests examine what the contract of the suite cannot reach.

// testHost speaks the protocol with an extension that Serve runs in the
// process of the test.
type testHost struct {
	t        *testing.T
	input    *io.PipeWriter
	output   *bufio.Reader
	lastID   uint64
	served   chan error
	received []*wire.Envelope
}

// serve starts ext. wrap can change the stream to the extension.
func serve(t *testing.T, ext *Extension, wrap func(io.Reader) io.Reader) *testHost {
	t.Helper()
	inputReader, inputWriter := io.Pipe()
	outputReader, outputWriter := io.Pipe()
	host := &testHost{t: t, input: inputWriter, output: bufio.NewReader(outputReader), served: make(chan error, 1)}
	go func() {
		host.served <- ext.Serve(context.Background(), wrap(inputReader), outputWriter)
		outputWriter.Close()
	}()
	t.Cleanup(func() {
		inputWriter.Close()
		outputReader.Close()
		inputReader.Close()
	})
	return host
}

func unchanged(input io.Reader) io.Reader { return input }

func (h *testHost) send(kind *wire.Envelope) {
	h.t.Helper()
	frame, err := encodeFrame(kind)
	if err != nil {
		h.t.Fatalf("the envelope was not encoded: %v", err)
	}
	if _, err := h.input.Write(frame); err != nil {
		h.t.Fatalf("the frame was not written: %v", err)
	}
}

func (h *testHost) startRequest(request *wire.Request) uint64 {
	h.t.Helper()
	h.lastID++
	request.Id = h.lastID
	h.send(&wire.Envelope{Kind: &wire.Envelope_Request{Request: request}})
	return h.lastID
}

func (h *testHost) startCall(method string, payload []byte) uint64 {
	h.t.Helper()
	return h.startRequest(&wire.Request{Body: &wire.Request_Call{Call: &wire.CallRequest{Method: method, Payload: payload}}})
}

// next reads the next envelope that the extension sent.
func (h *testHost) next() *wire.Envelope {
	h.t.Helper()
	envelope, err := readEnvelope(h.output)
	if err != nil {
		h.t.Fatalf("no envelope from the extension: %v", err)
	}
	return envelope
}

// response reads until the response to id and keeps the other envelopes.
func (h *testHost) response(id uint64) *wire.Response {
	h.t.Helper()
	for {
		envelope := h.next()
		if response := envelope.GetResponse(); response != nil && response.GetId() == id {
			return response
		}
		h.received = append(h.received, envelope)
	}
}

func (h *testHost) start(permissions *wire.Permissions) {
	h.t.Helper()
	hello := &wire.Hello{ApiVersion: APIVersion().toWire(), ExtensionId: "test.go", Permissions: permissions}
	if h.response(h.startRequest(&wire.Request{Body: &wire.Request_Hello{Hello: hello}})).GetWelcome() == nil {
		h.t.Fatal("Hello did not get Welcome")
	}
	initialized := h.response(h.startRequest(&wire.Request{Body: &wire.Request_Initialize{Initialize: &wire.InitializeRequest{}}}))
	if initialized.GetInitialize() == nil {
		h.t.Fatalf("InitializeRequest got %v", initialized)
	}
}

func expectCode(t *testing.T, response *wire.Response, code ErrorCode) {
	t.Helper()
	if got := ErrorCode(response.GetError().GetCode()); got != code {
		t.Fatalf("the response is %v, not an error with %s", response, code)
	}
}

func echoHandler(_ context.Context, _ *Host, payload []byte) ([]byte, error) {
	return payload, nil
}

func TestFramesThatArriveOneByteAtATime(t *testing.T) {
	ext := New("test.go", "1.0.0")
	ext.Handle("echo", echoHandler)
	host := serve(t, ext, iotest.OneByteReader)
	host.start(nil)

	result := host.response(host.startCall("echo", []byte("payload")))
	if got := string(result.GetCall().GetPayload()); got != "payload" {
		t.Fatalf("echo returned %q", got)
	}
}

func TestResultLargerThanTheLimitFailsOnlyItsCall(t *testing.T) {
	ext := New("test.go", "1.0.0")
	ext.Handle("large", func(context.Context, *Host, []byte) ([]byte, error) {
		return make([]byte, MaxEnvelopeLength), nil
	})
	ext.Handle("echo", echoHandler)
	host := serve(t, ext, unchanged)
	host.start(nil)

	expectCode(t, host.response(host.startCall("large", nil)), CodeFailed)
	if host.response(host.startCall("echo", []byte("after"))).GetCall() == nil {
		t.Fatal("the call after the large result failed")
	}
}

func TestCallThatIsCancelledBeforeItsTurnDoesNotRun(t *testing.T) {
	ext := New("test.go", "1.0.0")
	ext.Handle("block", func(ctx context.Context, host *Host, _ []byte) ([]byte, error) {
		if err := host.Emit("started", nil); err != nil {
			return nil, err
		}
		<-ctx.Done()
		return nil, ctx.Err()
	})
	ran := false
	ext.Handle("mark", func(context.Context, *Host, []byte) ([]byte, error) {
		ran = true
		return nil, nil
	})
	host := serve(t, ext, unchanged)
	host.start(nil)

	blocked := host.startCall("block", nil)
	if host.next().GetEvent().GetName() != "started" {
		t.Fatal("block did not emit started")
	}
	queued := host.startCall("mark", nil)
	host.send(&wire.Envelope{Kind: &wire.Envelope_Cancel{Cancel: &wire.Cancel{Id: queued}}})
	host.send(&wire.Envelope{Kind: &wire.Envelope_Cancel{Cancel: &wire.Cancel{Id: blocked}}})

	expectCode(t, host.response(blocked), CodeCancelled)
	expectCode(t, host.response(queued), CodeCancelled)
	if ran {
		t.Fatal("the call that was cancelled in the queue ran")
	}
}

func TestNoHandlerRunsWhenTheInitializationFailed(t *testing.T) {
	ext := New("test.go", "1.0.0")
	ext.OnInitialize(func(context.Context, *Host) error {
		return Errorf(CodePermissionDenied, "no access")
	})
	ext.Handle("echo", echoHandler)
	host := serve(t, ext, unchanged)
	hello := &wire.Hello{ApiVersion: APIVersion().toWire()}
	host.response(host.startRequest(&wire.Request{Body: &wire.Request_Hello{Hello: hello}}))

	failed := host.response(host.startRequest(&wire.Request{Body: &wire.Request_Initialize{Initialize: &wire.InitializeRequest{}}}))
	expectCode(t, failed, CodePermissionDenied)
	if failed.GetError().GetMessage() != "no access" {
		t.Fatalf("the message of the failure is %q", failed.GetError().GetMessage())
	}
	expectCode(t, host.response(host.startCall("echo", nil)), CodeFailed)
}

func TestPermissionsOfTheHandshakeReachTheExtension(t *testing.T) {
	ext := New("test.go", "1.0.0")
	got := make(chan Permissions, 1)
	ext.OnInitialize(func(_ context.Context, host *Host) error {
		got <- host.Permissions()
		return nil
	})
	host := serve(t, ext, unchanged)
	host.start(&wire.Permissions{Fs: []string{"/w"}, Process: []string{"git"}, Network: true, Scopes: []string{"sessions:read"}})

	want := Permissions{FS: []string{"/w"}, Process: []string{"git"}, Network: true, Scopes: []string{"sessions:read"}}
	if permissions := <-got; !reflect.DeepEqual(permissions, want) {
		t.Fatalf("the permissions are %+v, not %+v", permissions, want)
	}
}

func TestCallOfAHostServiceEndsWithTheContextOfTheHandler(t *testing.T) {
	ext := New("test.go", "1.0.0")
	ext.Handle("relay", func(ctx context.Context, host *Host, payload []byte) ([]byte, error) {
		return host.Call(ctx, "service.method", payload)
	})
	host := serve(t, ext, unchanged)
	host.start(nil)

	relay := host.startCall("relay", nil)
	if host.next().GetRequest().GetCall().GetMethod() != "service.method" {
		t.Fatal("relay did not call the host service")
	}
	host.send(&wire.Envelope{Kind: &wire.Envelope_Cancel{Cancel: &wire.Cancel{Id: relay}}})
	expectCode(t, host.response(relay), CodeCancelled)
}

func TestServeWaitsForTheRunningHandlerAfterTheStop(t *testing.T) {
	for _, test := range []struct {
		name         string
		obeysContext bool
		minimum      time.Duration
		maximum      time.Duration
	}{
		{"a handler that returns when its context is done", true, 0, StopTimeout / 2},
		{"a handler that does not return", false, StopTimeout, 2 * StopTimeout},
	} {
		t.Run(test.name, func(t *testing.T) {
			release := make(chan struct{})
			t.Cleanup(func() { close(release) })
			ext := New("test.go", "1.0.0")
			ext.Handle("block", func(ctx context.Context, host *Host, _ []byte) ([]byte, error) {
				if err := host.Emit("started", nil); err != nil {
					return nil, err
				}
				if test.obeysContext {
					<-ctx.Done()
				} else {
					<-release
				}
				return nil, nil
			})
			host := serve(t, ext, unchanged)
			host.start(nil)
			host.startCall("block", nil)
			host.next()

			stopped := time.Now()
			host.input.Close()
			err := <-host.served
			elapsed := time.Since(stopped)
			if !errors.Is(err, ErrDisconnected) {
				t.Fatalf("Serve returned %v, not ErrDisconnected", err)
			}
			if elapsed < test.minimum || elapsed > test.maximum {
				t.Fatalf("Serve returned after %v, not between %v and %v", elapsed, test.minimum, test.maximum)
			}
		})
	}
}
