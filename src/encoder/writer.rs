use std::{io::Write, vec};

/// Read the chunk from underlying reader and encode it
pub struct Encoder<W: Write> {
    w: W,
}

impl<W: Write> Encoder<W> {
    pub fn new(w: W) -> Self {
        Self { w: w }
    }
}

impl<W: Write> Write for Encoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // processing the chunk
        let chunk_size = buf.len();
        let chunk_size_hex = if chunk_size < 10 {
            format!("{:x}", chunk_size)
        } else {
            format!("{:02x}", chunk_size)
        };

        let chunk_size_bytes = chunk_size_hex.as_bytes().len();

        let mut processed_buf = vec![];

        processed_buf.extend_from_slice(chunk_size_hex.as_bytes());
        processed_buf.extend_from_slice(b"\r\n");
        processed_buf.extend_from_slice(buf);
        processed_buf.extend_from_slice(b"\r\n");

        self.w.write_all(&processed_buf)?;

        Ok(chunk_size)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::{Read, Write};

    use crate::encoder;

    #[test]
    fn works_correctly() {
        let payload = b"1234567890";
        let expected = b"0a\r\n1234567890\r\n";

        let mut result = vec![];
        let mut encoder = encoder::writer::Encoder::new(&mut result);

        let read_bytes = encoder.write(payload).unwrap();

        assert_eq!(&result, expected);
    }
}
