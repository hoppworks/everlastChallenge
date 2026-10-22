use super::{MAX_FRAME_BYTES, ProtocolError, VERSION};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{self, Read, Write};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<T> {
    version: u16,
    payload: T,
}

fn read_error(error: io::Error) -> ProtocolError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        ProtocolError::Truncated
    } else {
        ProtocolError::Io(error)
    }
}

pub(super) fn read<T: DeserializeOwned>(
    reader: &mut impl Read,
) -> Result<Option<T>, ProtocolError> {
    let mut prefix = [0; 4];
    loop {
        match reader.read(&mut prefix[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(ProtocolError::Io(error)),
        }
    }
    reader.read_exact(&mut prefix[1..]).map_err(read_error)?;
    let size = u32::from_be_bytes(prefix) as usize;
    if size > MAX_FRAME_BYTES {
        return Err(ProtocolError::TooLarge);
    }
    if size == 0 {
        return Err(ProtocolError::Malformed);
    }
    let mut bytes = vec![0; size];
    reader.read_exact(&mut bytes).map_err(read_error)?;
    // Typed deserialization rejects duplicate/unknown fields. Do not deserialize
    // via Value, which would silently collapse duplicate JSON object members.
    let envelope: Envelope<T> =
        serde_json::from_slice(&bytes).map_err(|_| ProtocolError::Malformed)?;
    if envelope.version != VERSION {
        return Err(ProtocolError::Version);
    }
    Ok(Some(envelope.payload))
}

struct BoundedBuffer {
    bytes: Vec<u8>,
    overflow: bool,
}

impl Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|size| size > MAX_FRAME_BYTES)
        {
            self.overflow = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame too large",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn write<T: Serialize>(
    writer: &mut impl Write,
    payload: &T,
) -> Result<(), ProtocolError> {
    let mut buffer = BoundedBuffer {
        bytes: Vec::new(),
        overflow: false,
    };
    if serde_json::to_writer(
        &mut buffer,
        &Envelope {
            version: VERSION,
            payload,
        },
    )
    .is_err()
    {
        return Err(if buffer.overflow {
            ProtocolError::TooLarge
        } else {
            ProtocolError::Malformed
        });
    }
    let length = u32::try_from(buffer.bytes.len()).map_err(|_| ProtocolError::TooLarge)?;
    // Serialize completely before touching the pipe. An oversized or malformed
    // local message cannot leave a valid-looking partial prefix in the stream.
    writer
        .write_all(&length.to_be_bytes())
        .map_err(ProtocolError::Io)?;
    writer.write_all(&buffer.bytes).map_err(ProtocolError::Io)?;
    writer.flush().map_err(ProtocolError::Io)
}
