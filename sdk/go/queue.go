package extension

import (
	"context"
	"sync"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// job is a request whose work runs on the worker goroutine: an
// initialization, a shutdown or a call.
type job struct {
	request *wire.Request
	ctx     context.Context
	cancel  context.CancelFunc
}

// queue holds the jobs in the sequence of arrival. It also cancels a job that
// waits or runs when the host sends Cancel.
type queue struct {
	mutex sync.Mutex
	jobs  []*job
	// active has each job that waits or runs, by request id.
	active map[uint64]*job
	closed bool
	// added gets a value when a job is added.
	added chan struct{}
	// done is closed by close.
	done chan struct{}
}

func newQueue() *queue {
	return &queue{
		active: make(map[uint64]*job),
		added:  make(chan struct{}, 1),
		done:   make(chan struct{}),
	}
}

// push adds a job. Its context is a child of parent.
func (q *queue) push(parent context.Context, request *wire.Request) {
	ctx, cancel := context.WithCancel(parent)
	added := &job{request: request, ctx: ctx, cancel: cancel}
	q.mutex.Lock()
	if q.closed {
		q.mutex.Unlock()
		cancel()
		return
	}
	q.jobs = append(q.jobs, added)
	q.active[request.GetId()] = added
	q.mutex.Unlock()
	select {
	case q.added <- struct{}{}:
	default:
	}
}

// pop waits for the next job. It returns nil after close.
func (q *queue) pop() *job {
	for {
		q.mutex.Lock()
		if q.closed {
			q.mutex.Unlock()
			return nil
		}
		if len(q.jobs) > 0 {
			next := q.jobs[0]
			q.jobs[0] = nil
			q.jobs = q.jobs[1:]
			q.mutex.Unlock()
			return next
		}
		q.mutex.Unlock()
		select {
		case <-q.added:
		case <-q.done:
		}
	}
}

// cancel cancels the context of the job with this request id. An id that is
// not active has no effect.
func (q *queue) cancel(id uint64) {
	q.mutex.Lock()
	cancelled := q.active[id]
	q.mutex.Unlock()
	if cancelled != nil {
		cancelled.cancel()
	}
}

// finish releases a job after its work returned.
func (q *queue) finish(finished *job) {
	finished.cancel()
	q.mutex.Lock()
	defer q.mutex.Unlock()
	if q.active[finished.request.GetId()] == finished {
		delete(q.active, finished.request.GetId())
	}
}

// close cancels each job and makes pop return nil.
func (q *queue) close() {
	q.mutex.Lock()
	if q.closed {
		q.mutex.Unlock()
		return
	}
	q.closed = true
	active := q.active
	q.active = nil
	q.jobs = nil
	q.mutex.Unlock()
	for _, cancelled := range active {
		cancelled.cancel()
	}
	close(q.done)
}
