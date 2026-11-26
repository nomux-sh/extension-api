package extension

import (
	"fmt"
	"io"
	"sync"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// outbound is the stream to the host. Each goroutine can send.
type outbound struct {
	mutex  sync.Mutex
	output io.Writer
	// failure is the first error of the stream. After it, nothing is
	// written.
	failure error
}

// send writes one envelope in one call of Write. It returns a *tooLargeError
// if the envelope is larger than the limit; then nothing is written and the
// stream stays usable.
func (o *outbound) send(envelope *wire.Envelope) error {
	frame, err := encodeFrame(envelope)
	if err != nil {
		return err
	}
	o.mutex.Lock()
	defer o.mutex.Unlock()
	if o.failure != nil {
		return o.failure
	}
	if _, err := o.output.Write(frame); err != nil {
		o.failure = fmt.Errorf("%w: the stream failed: %w", ErrFrame, err)
	}
	return o.failure
}

func (o *outbound) respond(id uint64, result *wire.Response) error {
	result.Id = id
	return o.send(&wire.Envelope{Kind: &wire.Envelope_Response{Response: result}})
}
