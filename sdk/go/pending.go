package extension

import (
	"sync"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// pendingCalls are the calls of host services that have no response yet.
type pendingCalls struct {
	mutex  sync.Mutex
	lastID uint64
	// waiting is nil after close.
	waiting map[uint64]chan *wire.Response
}

func newPendingCalls() *pendingCalls {
	return &pendingCalls{waiting: make(map[uint64]chan *wire.Response)}
}

// register reserves a request number. The channel gets the response, or it
// is closed when the connection stops. ok is false after close.
func (p *pendingCalls) register() (id uint64, response <-chan *wire.Response, ok bool) {
	p.mutex.Lock()
	defer p.mutex.Unlock()
	if p.waiting == nil {
		return 0, nil, false
	}
	p.lastID++
	channel := make(chan *wire.Response, 1)
	p.waiting[p.lastID] = channel
	return p.lastID, channel, true
}

func (p *pendingCalls) forget(id uint64) {
	p.mutex.Lock()
	defer p.mutex.Unlock()
	delete(p.waiting, id)
}

// resolve gives a response to the call that waits for it. A response with an
// id that no call waits for is dropped.
func (p *pendingCalls) resolve(response *wire.Response) {
	p.mutex.Lock()
	defer p.mutex.Unlock()
	channel, ok := p.waiting[response.GetId()]
	if !ok {
		return
	}
	delete(p.waiting, response.GetId())
	channel <- response
}

// close fails each call that waits and each subsequent call.
func (p *pendingCalls) close() {
	p.mutex.Lock()
	defer p.mutex.Unlock()
	for _, channel := range p.waiting {
		close(channel)
	}
	p.waiting = nil
}
