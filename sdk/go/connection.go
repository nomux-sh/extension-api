package extension

import (
	"bufio"
	"context"
	"errors"
	"io"
	"sync"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// connection is one connection to the host, from Hello to the end.
//
// The reader goroutine reads the frames as they arrive. It answers health
// requests, applies cancellations and gives the responses of host services to
// the calls that wait for them at once. The worker goroutine does the work of
// the other requests, one request at a time, in the sequence of arrival.
type connection struct {
	extension *Extension
	ctx       context.Context
	input     *bufio.Reader
	outbound  *outbound
	pending   *pendingCalls
	queue     *queue
	// host is set by the reader goroutine before the first job. The worker
	// goroutine reads it only after it got a job.
	host *Host

	stopOnce sync.Once
	// stopped is closed when the connection stops.
	stopped chan struct{}
	// failure is the reason for the stop, nil after a shutdown request. It
	// is set before stopped is closed.
	failure error
}

func newConnection(ctx context.Context, extension *Extension, input io.Reader, output io.Writer) *connection {
	return &connection{
		extension: extension,
		ctx:       ctx,
		input:     bufio.NewReader(input),
		outbound:  &outbound{output: output},
		pending:   newPendingCalls(),
		queue:     newQueue(),
		stopped:   make(chan struct{}),
	}
}

// read is the reader goroutine. It returns when the input ends or fails, or
// at the first frame after the stop.
func (c *connection) read() {
	for {
		envelope, err := readEnvelope(c.input)
		if errors.Is(err, io.EOF) {
			err = ErrDisconnected
		}
		if err != nil {
			c.stop(err)
			return
		}
		if c.isStopped() {
			return
		}
		if c.host == nil {
			c.handshake(envelope)
			continue
		}
		c.route(envelope)
	}
}

func (c *connection) handshake(envelope *wire.Envelope) {
	response, permissions, err := answerHello(envelope, c.extension.identity)
	if response != nil {
		if sent := c.outbound.send(&wire.Envelope{Kind: &wire.Envelope_Response{Response: response}}); sent != nil {
			c.stop(sent)
			return
		}
	}
	if err != nil {
		c.stop(err)
		return
	}
	c.host = &Host{outbound: c.outbound, pending: c.pending, permissions: permissions}
}

func (c *connection) route(envelope *wire.Envelope) {
	switch kind := envelope.GetKind().(type) {
	case *wire.Envelope_Request:
		c.routeRequest(kind.Request)
	case *wire.Envelope_Response:
		c.pending.resolve(kind.Response)
	case *wire.Envelope_Cancel:
		c.queue.cancel(kind.Cancel.GetId())
	default:
		// The host sends no events. A kind that this version does not know
		// comes from a newer host and is not an error.
	}
}

func (c *connection) routeRequest(request *wire.Request) {
	switch request.GetBody().(type) {
	case *wire.Request_Health:
		c.respond(request.GetId(), &wire.Response{Result: &wire.Response_Health{Health: &wire.HealthResponse{}}})
	case *wire.Request_Hello:
		c.respond(request.GetId(), failure("the handshake is already complete"))
	case *wire.Request_Initialize, *wire.Request_Shutdown, *wire.Request_Call:
		c.queue.push(c.ctx, request)
	default:
		c.respond(request.GetId(), failure("the extension does not know this request"))
	}
}

// work is the worker goroutine. It returns when the connection stops.
func (c *connection) work() {
	for {
		next := c.queue.pop()
		if next == nil {
			return
		}
		result := c.extension.answer(next.ctx, c.host, next.request)
		c.queue.finish(next)
		if c.isStopped() {
			return
		}
		c.respond(next.request.GetId(), result)
		if next.request.GetShutdown() != nil {
			c.stop(nil)
			return
		}
	}
}

// respond sends a response. A result that is too large is the failure of one
// request: the connection stays open. A failed stream stops the connection.
func (c *connection) respond(id uint64, result *wire.Response) {
	err := c.outbound.respond(id, result)
	if tooLarge, ok := errors.AsType[*tooLargeError](err); ok {
		err = c.outbound.respond(id, failure(tooLarge.Error()))
	}
	if err != nil {
		c.stop(err)
	}
}

// stop stops the connection one time. err is nil after a shutdown request.
// The context of each job is cancelled, and each call of a host service that
// waits fails.
func (c *connection) stop(err error) {
	c.stopOnce.Do(func() {
		c.failure = err
		c.queue.close()
		c.pending.close()
		close(c.stopped)
	})
}

func (c *connection) isStopped() bool {
	select {
	case <-c.stopped:
		return true
	default:
		return false
	}
}

func failure(message string) *wire.Response {
	return &wire.Response{Result: &wire.Response_Error{Error: &wire.Error{Code: wire.ErrorCode_ERROR_CODE_FAILED, Message: message}}}
}
