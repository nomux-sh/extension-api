//! The framing of the extension protocol.
//!
//! The host and the extension exchange [`Envelope`] messages on a byte
//! stream. Each message is the length of the encoded envelope as a protobuf
//! varint, then the encoded envelope. The specification is in
//! `spec/extension-protocol.md`.
//!
//! An extension does not call these functions: [`crate::Extension`] reads and
//! writes the frames. A host uses them to speak with an extension.

use std::fmt;
use std::io::{self, Read, Write};

use prost::Message;

use crate::wire::v1::Envelope;

/// The largest number of bytes of one encoded [`Envelope`].
///
/// A peer that reads a larger length stops: it cannot find the start of the
/// subsequent frame.
pub const MAX_ENVELOPE_LENGTH: usize = 4 * 1024 * 1024;

/// The largest number of bytes of a length that is not larger than
/// [`MAX_ENVELOPE_LENGTH`], with room for a length that is too large.
const MAX_LENGTH_BYTES: u32 = 5;

/// The reason why a frame was not read or written.
#[derive(Debug)]
pub enum FrameError {
    /// The stream failed. The kind is `UnexpectedEof` when the stream ended
    /// in the middle of a frame.
    Io(io::Error),
    /// The envelope is larger than [`MAX_ENVELOPE_LENGTH`].
    TooLarge {
        /// The number of bytes of the encoded envelope.
        length: u64,
    },
    /// The length of the frame is not a varint of 5 bytes at most.
    InvalidLength,
    /// The bytes of the frame are not an encoded envelope.
    Decode(prost::DecodeError),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "the stream failed: {error}"),
            Self::TooLarge { length } => write!(
                f,
                "the envelope has {length} bytes, but the limit is {MAX_ENVELOPE_LENGTH} bytes"
            ),
            Self::InvalidLength => f.write_str("the length of the frame is not valid"),
            Self::Decode(error) => write!(f, "the frame is not an envelope: {error}"),
        }
    }
}

impl std::error::Error for FrameError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::TooLarge { .. } | Self::InvalidLength => None,
        }
    }
}

impl From<io::Error> for FrameError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Reads one envelope.
///
/// The result is `None` when the stream ended before the first byte of a
/// frame: the other peer closed the stream between two messages.
///
/// The function reads the length one byte at a time. Give it a buffered
/// reader.
pub fn read_envelope(reader: &mut impl Read) -> Result<Option<Envelope>, FrameError> {
    let Some(length) = read_length(reader)? else {
        return Ok(None);
    };
    if length > MAX_ENVELOPE_LENGTH as u64 {
        return Err(FrameError::TooLarge { length });
    }
    let mut body = vec![0; length as usize];
    reader.read_exact(&mut body)?;
    Envelope::decode(body.as_slice())
        .map(Some)
        .map_err(FrameError::Decode)
}

fn read_length(reader: &mut impl Read) -> Result<Option<u64>, FrameError> {
    let mut length = 0;
    for index in 0..MAX_LENGTH_BYTES {
        let mut byte = [0];
        match reader.read_exact(&mut byte) {
            Ok(()) => {}
            Err(error) if index == 0 && error.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        }
        length |= u64::from(byte[0] & 0x7f) << (7 * index);
        if byte[0] & 0x80 == 0 {
            return Ok(Some(length));
        }
    }
    Err(FrameError::InvalidLength)
}

/// Writes one envelope and flushes the stream.
pub fn write_envelope(writer: &mut impl Write, envelope: &Envelope) -> Result<(), FrameError> {
    let length = envelope.encoded_len();
    if length > MAX_ENVELOPE_LENGTH {
        return Err(FrameError::TooLarge {
            length: length as u64,
        });
    }
    writer.write_all(&envelope.encode_length_delimited_to_vec())?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::v1::envelope::Kind;
    use crate::wire::v1::{Cancel, Event};

    fn cancel(id: u64) -> Envelope {
        Envelope {
            kind: Some(Kind::Cancel(Cancel { id })),
        }
    }

    fn event_with(payload_length: usize) -> Envelope {
        Envelope {
            kind: Some(Kind::Event(Event {
                name: String::new(),
                payload: vec![0; payload_length],
            })),
        }
    }

    /// An envelope whose encoded form has exactly `length` bytes.
    fn envelope_of(length: usize) -> Envelope {
        let tags_and_lengths = event_with(length).encoded_len() - length;
        let envelope = event_with(length - tags_and_lengths);
        assert_eq!(envelope.encoded_len(), length);
        envelope
    }

    fn encode(envelope: &Envelope) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_envelope(&mut bytes, envelope).unwrap();
        bytes
    }

    #[test]
    fn stream_end_between_two_frames_is_not_an_error() {
        let mut bytes = encode(&cancel(1));
        bytes.extend(encode(&cancel(300)));
        let mut reader = bytes.as_slice();

        assert_eq!(read_envelope(&mut reader).unwrap(), Some(cancel(1)));
        assert_eq!(read_envelope(&mut reader).unwrap(), Some(cancel(300)));
        assert_eq!(read_envelope(&mut reader).unwrap(), None);
    }

    #[test]
    fn stream_end_in_a_frame_is_an_error() {
        let bytes = encode(&envelope_of(300));
        let in_the_length = &bytes[..1];
        let in_the_body = &bytes[..bytes.len() - 1];

        for mut cut in [in_the_length, in_the_body] {
            let error = read_envelope(&mut cut).unwrap_err();
            assert!(
                matches!(&error, FrameError::Io(error) if error.kind() == io::ErrorKind::UnexpectedEof),
                "{error:?}"
            );
        }
    }

    #[test]
    fn envelope_of_the_maximum_length_is_written_and_read() {
        let envelope = envelope_of(MAX_ENVELOPE_LENGTH);

        let bytes = encode(&envelope);

        assert_eq!(
            read_envelope(&mut bytes.as_slice()).unwrap(),
            Some(envelope)
        );
    }

    #[test]
    fn envelope_larger_than_the_maximum_is_not_written() {
        let mut bytes = Vec::new();

        let error = write_envelope(&mut bytes, &envelope_of(MAX_ENVELOPE_LENGTH + 1)).unwrap_err();

        assert!(
            matches!(error, FrameError::TooLarge { length } if length == MAX_ENVELOPE_LENGTH as u64 + 1)
        );
        assert!(bytes.is_empty());
    }

    #[test]
    fn length_larger_than_the_maximum_is_refused_before_the_body_is_read() {
        let mut length = Vec::new();
        prost::encoding::encode_varint(MAX_ENVELOPE_LENGTH as u64 + 1, &mut length);

        let error = read_envelope(&mut length.as_slice()).unwrap_err();

        assert!(
            matches!(error, FrameError::TooLarge { length } if length == MAX_ENVELOPE_LENGTH as u64 + 1)
        );
    }

    #[test]
    fn length_of_more_than_five_bytes_is_not_valid() {
        let mut bytes: &[u8] = &[0x80, 0x80, 0x80, 0x80, 0x80, 0x00];

        let error = read_envelope(&mut bytes).unwrap_err();

        assert!(matches!(error, FrameError::InvalidLength), "{error:?}");
    }

    #[test]
    fn bytes_that_are_not_an_envelope_are_a_decode_error() {
        // A frame of one byte: a field tag without its value.
        let mut bytes: &[u8] = &[0x01, 0x0a];

        let error = read_envelope(&mut bytes).unwrap_err();

        assert!(matches!(error, FrameError::Decode(_)), "{error:?}");
    }
}
