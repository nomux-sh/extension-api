package extension

import (
	"bufio"
	"errors"
	"fmt"
	"io"

	"google.golang.org/protobuf/encoding/protowire"
	"google.golang.org/protobuf/proto"

	"github.com/nomux-sh/extension-api/sdk/go/internal/wire"
)

// MaxEnvelopeLength is the largest number of bytes of one encoded envelope. A
// peer that reads a larger length stops: it cannot find the start of the
// subsequent frame.
const MaxEnvelopeLength = 4 * 1024 * 1024

// maxLengthBytes is the largest number of bytes of the length of a frame.
const maxLengthBytes = 5

// tooLargeError is the error of an envelope that is larger than
// MaxEnvelopeLength.
type tooLargeError struct {
	length uint64
}

func (e *tooLargeError) Error() string {
	return fmt.Sprintf("the envelope has %d bytes, but the limit is %d bytes", e.length, MaxEnvelopeLength)
}

// readEnvelope reads one frame. It returns io.EOF if the stream ends before
// the first byte of a length.
func readEnvelope(input *bufio.Reader) (*wire.Envelope, error) {
	length, err := readLength(input)
	if err != nil {
		return nil, err
	}
	body := make([]byte, length)
	if _, err := io.ReadFull(input, body); err != nil {
		return nil, streamError(err)
	}
	envelope := &wire.Envelope{}
	if err := proto.Unmarshal(body, envelope); err != nil {
		return nil, fmt.Errorf("%w: the bytes are not an envelope: %v", ErrFrame, err)
	}
	return envelope, nil
}

func readLength(input *bufio.Reader) (uint64, error) {
	var length uint64
	for count := range maxLengthBytes {
		next, err := input.ReadByte()
		if err != nil {
			if count == 0 && errors.Is(err, io.EOF) {
				return 0, io.EOF
			}
			return 0, streamError(err)
		}
		length |= uint64(next&0x7f) << (7 * count)
		if next&0x80 == 0 {
			if length > MaxEnvelopeLength {
				return 0, fmt.Errorf("%w: %w", ErrFrame, &tooLargeError{length: length})
			}
			return length, nil
		}
	}
	return 0, fmt.Errorf("%w: the length of the frame has more than %d bytes", ErrFrame, maxLengthBytes)
}

func streamError(err error) error {
	if errors.Is(err, io.EOF) {
		return fmt.Errorf("%w: the stream ended in the middle of a frame", ErrFrame)
	}
	return fmt.Errorf("%w: the stream failed: %w", ErrFrame, err)
}

// encodeFrame encodes one envelope as a frame: the varint length, then the
// envelope. It returns a *tooLargeError if the envelope is larger than
// MaxEnvelopeLength.
func encodeFrame(envelope *wire.Envelope) ([]byte, error) {
	length := proto.Size(envelope)
	if length > MaxEnvelopeLength {
		return nil, &tooLargeError{length: uint64(length)}
	}
	frame := protowire.AppendVarint(make([]byte, 0, length+maxLengthBytes), uint64(length))
	return proto.MarshalOptions{UseCachedSize: true}.MarshalAppend(frame, envelope)
}
