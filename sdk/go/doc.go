// Package extension is the nomux extension SDK for Go: the daemon part of a
// nomux extension.
//
// The daemon part of an extension is a separate process. The nomux daemon
// (the host) starts it when a client needs it and stops it when it is idle.
// The host and the extension exchange protobuf messages on the standard input
// and the standard output of the process. The extension protocol
// (spec/extension-protocol.md in the repository) specifies the messages.
//
// Make an extension with its identifier and version, add the handlers of its
// methods, then call [Extension.Run]:
//
//	func main() {
//		ext := extension.New("nomux.echo", "0.1.0")
//		ext.Handle("echo", func(ctx context.Context, host *extension.Host, payload []byte) ([]byte, error) {
//			if err := host.Emit("echoed", nil); err != nil {
//				return nil, err
//			}
//			return payload, nil
//		})
//		if err := ext.Run(context.Background()); err != nil {
//			log.Fatal(err)
//		}
//	}
//
// The standard output carries the protocol. Write logs to the standard error.
//
// The handlers run one at a time on one goroutine, in the sequence in which
// the calls arrive. A second goroutine reads the messages from the host. Thus
// the extension answers health requests and applies cancellations while a
// handler runs.
package extension
