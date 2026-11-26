package extension

import (
	"context"
	"errors"
	"fmt"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// ErrorCode is the reason for a failed request.
type ErrorCode int32

// The error codes of the extension protocol.
const (
	// CodeFailed: the request failed, and no other code gives the reason.
	CodeFailed = ErrorCode(wire.ErrorCode_ERROR_CODE_FAILED)
	// CodeNotFound: the method or the item that the request names does not
	// exist.
	CodeNotFound = ErrorCode(wire.ErrorCode_ERROR_CODE_NOT_FOUND)
	// CodeTimeout: the receiver did not complete the request in the
	// permitted time.
	CodeTimeout = ErrorCode(wire.ErrorCode_ERROR_CODE_TIMEOUT)
	// CodePermissionDenied: the extension does not have the permission that
	// the request needs.
	CodePermissionDenied = ErrorCode(wire.ErrorCode_ERROR_CODE_PERMISSION_DENIED)
	// CodeVersionIncompatible: the host does not supply the extension API
	// version that the extension needs. Only the reply to Hello uses it.
	CodeVersionIncompatible = ErrorCode(wire.ErrorCode_ERROR_CODE_VERSION_INCOMPATIBLE)
	// CodeInvalidArgument: the receiver cannot read the payload of the
	// request.
	CodeInvalidArgument = ErrorCode(wire.ErrorCode_ERROR_CODE_INVALID_ARGUMENT)
	// CodeCancelled: the receiver stopped the request because the sender
	// sent Cancel.
	CodeCancelled = ErrorCode(wire.ErrorCode_ERROR_CODE_CANCELLED)
)

// String returns the name of the code in the schema, for example
// ERROR_CODE_FAILED.
func (c ErrorCode) String() string {
	return wire.ErrorCode(c).String()
}

// Error is the failure of one call.
//
// A handler returns it to fail the call with a code. [Host.Call] returns it
// when the host refuses or fails a call.
type Error struct {
	// Code is the reason for the failure.
	Code ErrorCode
	// Message is English text for the user. A program must not parse it.
	Message string
}

// Errorf makes an [Error] with a code and a formatted message.
func Errorf(code ErrorCode, format string, args ...any) *Error {
	return &Error{Code: code, Message: fmt.Sprintf(format, args...)}
}

// Error returns the message.
func (e *Error) Error() string {
	return e.Message
}

// The reasons why [Extension.Run] and [Extension.Serve] stop before the host
// sends a shutdown request. The returned error wraps one of them.
var (
	// ErrFrame: a frame is not valid, or the stream failed.
	ErrFrame = errors.New("the frame is not valid")
	// ErrNoHello: the first message from the host was not Hello.
	ErrNoHello = errors.New("the first message from the host was not Hello")
	// ErrIncompatibleAPI: the host does not supply a compatible extension
	// API version.
	ErrIncompatibleAPI = errors.New("the host does not supply a compatible extension API version")
	// ErrDisconnected: the host closed the stream and did not send a
	// shutdown request.
	ErrDisconnected = errors.New("the host closed the stream and did not send a shutdown request")
)

// callError returns the error that the host gets for an error of a handler.
// ctx is the context of the call.
func callError(ctx context.Context, err error) *Error {
	if failure, ok := errors.AsType[*Error](err); ok {
		return failure
	}
	if ctx.Err() != nil {
		return cancelled()
	}
	return &Error{Code: CodeFailed, Message: err.Error()}
}

func cancelled() *Error {
	return &Error{Code: CodeCancelled, Message: "the call was cancelled"}
}

// errorFromWire makes the error of a wire message. A code that is not set or
// not known is a failure with no other reason.
func errorFromWire(failure *wire.Error) *Error {
	code := ErrorCode(failure.GetCode())
	if code < CodeFailed || code > CodeCancelled {
		code = CodeFailed
	}
	return &Error{Code: code, Message: failure.GetMessage()}
}

func (e *Error) toWire() *wire.Error {
	return &wire.Error{Code: wire.ErrorCode(e.Code), Message: e.Message}
}
