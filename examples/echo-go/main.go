// The echo extension in Go: a small example of the daemon part of a nomux
// extension. It is also the conformance extension of the Go SDK: it has the
// methods that conformance/README.md describes.
package main

import (
	"context"
	"log"
	"time"

	extension "github.com/nomux-sh/extension-api/sdk/go"
)

// waitLimit is the time after which wait stops without a cancellation.
const waitLimit = 5 * time.Second

func echo(_ context.Context, host *extension.Host, payload []byte) ([]byte, error) {
	text, err := decodeText(payload)
	if err != nil {
		return nil, extension.Errorf(extension.CodeInvalidArgument, "the payload is not a Text: %v", err)
	}
	if text == "" {
		return nil, extension.Errorf(extension.CodeFailed, "the text is empty")
	}
	if err := host.Emit("echoed", encodeEchoed(uint64(len(text)))); err != nil {
		return nil, err
	}
	return encodeText(text), nil
}

func wait(ctx context.Context, host *extension.Host, payload []byte) ([]byte, error) {
	if err := decodeEmpty(payload); err != nil {
		return nil, extension.Errorf(extension.CodeInvalidArgument, "the payload is not empty: %v", err)
	}
	if err := host.Emit("started", nil); err != nil {
		return nil, err
	}
	select {
	case <-ctx.Done():
		return nil, ctx.Err()
	case <-time.After(waitLimit):
		return nil, nil
	}
}

func relay(ctx context.Context, host *extension.Host, payload []byte) ([]byte, error) {
	if _, err := decodeText(payload); err != nil {
		return nil, extension.Errorf(extension.CodeInvalidArgument, "the payload is not a Text: %v", err)
	}
	return host.Call(ctx, "conformance.reverse", payload)
}

func main() {
	ext := extension.New("nomux.echo-go", "0.1.0")
	ext.Handle("echo", echo)
	ext.Handle("wait", wait)
	ext.Handle("relay", relay)
	if err := ext.Run(context.Background()); err != nil {
		log.Fatal(err)
	}
}
