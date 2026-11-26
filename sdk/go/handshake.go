package extension

import (
	"fmt"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// identity is the identifier and the version of the extension.
type identity struct {
	id      string
	version string
}

// answerHello reads the first message of the host and applies the version
// rule. response is nil if the first message is not Hello: then the
// extension sends nothing. err is not nil if the extension must stop.
func answerHello(envelope *wire.Envelope, extension identity) (response *wire.Response, permissions Permissions, err error) {
	request := envelope.GetRequest()
	hello := request.GetHello()
	if hello == nil {
		return nil, Permissions{}, ErrNoHello
	}
	host := versionFromWire(hello.GetApiVersion())
	if !APIVersion().accepts(host) {
		message := fmt.Sprintf("the extension needs the extension API version ^%s, but the host supplies %s", APIVersion(), host)
		refusal := &Error{Code: CodeVersionIncompatible, Message: message}
		response := &wire.Response{Id: request.GetId(), Result: &wire.Response_Error{Error: refusal.toWire()}}
		return response, Permissions{}, fmt.Errorf("%w: %s", ErrIncompatibleAPI, message)
	}
	welcome := &wire.Welcome{
		ApiVersion:       APIVersion().toWire(),
		ExtensionId:      extension.id,
		ExtensionVersion: extension.version,
	}
	response = &wire.Response{Id: request.GetId(), Result: &wire.Response_Welcome{Welcome: welcome}}
	return response, permissionsFromWire(hello.GetPermissions()), nil
}
