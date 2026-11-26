package main

import (
	"errors"
	"fmt"
	"unicode/utf8"

	"google.golang.org/protobuf/encoding/protowire"
)

// The payloads of the echo extension, as protobuf messages. The field numbers
// are the same as in the Rust example and in conformance/README.md:
//
//	message Text { string text = 1; }
//	message Echoed { uint64 length = 1; }

// encodeText encodes the argument and the result of echo and relay.
func encodeText(text string) []byte {
	payload := protowire.AppendTag(nil, 1, protowire.BytesType)
	return protowire.AppendString(payload, text)
}

// decodeText decodes a Text. It returns an error if the bytes are not a valid
// message.
func decodeText(payload []byte) (string, error) {
	var text string
	err := readFields(payload, func(field protowire.Number, kind protowire.Type, value []byte) (int, error) {
		if field != 1 {
			return 0, nil
		}
		if kind != protowire.BytesType {
			return 0, fmt.Errorf("field 1 has the wire type %d, not %d", kind, protowire.BytesType)
		}
		bytes, length := protowire.ConsumeBytes(value)
		if length < 0 {
			return 0, protowire.ParseError(length)
		}
		if !utf8.Valid(bytes) {
			return 0, errors.New("field 1 is not valid UTF-8")
		}
		text = string(bytes)
		return length, nil
	})
	return text, err
}

// encodeEchoed encodes the payload of the event echoed.
func encodeEchoed(length uint64) []byte {
	if length == 0 {
		return nil
	}
	payload := protowire.AppendTag(nil, 1, protowire.VarintType)
	return protowire.AppendVarint(payload, length)
}

// decodeEmpty examines the argument of wait: a message without fields.
func decodeEmpty(payload []byte) error {
	return readFields(payload, func(protowire.Number, protowire.Type, []byte) (int, error) {
		return 0, nil
	})
}

// readFields reads each field of a message. read reads the value of a field
// and returns its length, or 0 to skip the field.
func readFields(payload []byte, read func(protowire.Number, protowire.Type, []byte) (int, error)) error {
	for len(payload) > 0 {
		field, kind, length := protowire.ConsumeTag(payload)
		if length < 0 {
			return protowire.ParseError(length)
		}
		payload = payload[length:]
		length, err := read(field, kind, payload)
		if err != nil {
			return err
		}
		if length == 0 {
			length = protowire.ConsumeFieldValue(field, kind, payload)
			if length < 0 {
				return protowire.ParseError(length)
			}
		}
		payload = payload[length:]
	}
	return nil
}
