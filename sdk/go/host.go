package extension

import (
	"context"
	"slices"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// Permissions are the permissions that the user approved for the extension:
// the [permissions] table of extension.toml. The host applies them. The
// extension reads them to know what it can do.
type Permissions struct {
	// FS has the directories that the extension can read and write. Each
	// item is an absolute path.
	FS []string
	// Process has the programs that the extension can start.
	Process []string
	// Network is true if the extension can open network connections.
	Network bool
	// Scopes has the scopes of host services, for example sessions:read.
	Scopes []string
}

func permissionsFromWire(permissions *wire.Permissions) Permissions {
	return Permissions{
		FS:      permissions.GetFs(),
		Process: permissions.GetProcess(),
		Network: permissions.GetNetwork(),
		Scopes:  permissions.GetScopes(),
	}
}

// Host is the nomux daemon that started the extension.
//
// Each handler gets the host. The host stays valid until the extension stops,
// and each goroutine can use it, for example to emit events when a file
// changes.
type Host struct {
	outbound    *outbound
	pending     *pendingCalls
	permissions Permissions
}

// Permissions returns the permissions that the user approved for the
// extension.
func (h *Host) Permissions() Permissions {
	return Permissions{
		FS:      slices.Clone(h.permissions.FS),
		Process: slices.Clone(h.permissions.Process),
		Network: h.permissions.Network,
		Scopes:  slices.Clone(h.permissions.Scopes),
	}
}

// Call calls a method of a host service and waits for the result.
//
// method has the form <service>.<method>, for example sessions.list. The
// method specifies the encoding of the payloads. The host fails the call with
// [CodePermissionDenied] if the extension does not have the scope of the
// service. If the host fails the call, the error is an [*Error].
//
// If ctx is done before the response arrives, Call returns ctx.Err() and
// drops the response. Call does not send Cancel to the host.
func (h *Host) Call(ctx context.Context, method string, payload []byte) ([]byte, error) {
	id, response, ok := h.pending.register()
	if !ok {
		return nil, disconnected()
	}
	request := &wire.Request{Id: id, Body: &wire.Request_Call{Call: &wire.CallRequest{Method: method, Payload: payload}}}
	if err := h.outbound.send(&wire.Envelope{Kind: &wire.Envelope_Request{Request: request}}); err != nil {
		h.pending.forget(id)
		return nil, Errorf(CodeFailed, "the call of `%s` was not sent: %v", method, err)
	}
	select {
	case answer, ok := <-response:
		if !ok {
			return nil, disconnected()
		}
		return callResult(method, answer)
	case <-ctx.Done():
		h.pending.forget(id)
		return nil, ctx.Err()
	}
}

func callResult(method string, answer *wire.Response) ([]byte, error) {
	switch result := answer.GetResult().(type) {
	case *wire.Response_Call:
		return result.Call.GetPayload(), nil
	case *wire.Response_Error:
		return nil, errorFromWire(result.Error)
	default:
		return nil, Errorf(CodeFailed, "the host did not answer `%s` with a call result", method)
	}
}

// Emit sends an event to the host. The host adds the identifier of the
// extension to the name.
func (h *Host) Emit(name string, payload []byte) error {
	event := &wire.Event{Name: name, Payload: payload}
	if err := h.outbound.send(&wire.Envelope{Kind: &wire.Envelope_Event{Event: event}}); err != nil {
		return Errorf(CodeFailed, "the event `%s` was not sent: %v", name, err)
	}
	return nil
}

func disconnected() *Error {
	return &Error{Code: CodeFailed, Message: "the host closed the connection"}
}
