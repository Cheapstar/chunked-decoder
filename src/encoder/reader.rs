use std::{io::Read, vec};

/// Read the chunk from underlying reader and encode it
pub struct Encoder<R: Read> {
    r: R,
}

impl<R: Read> Encoder<R> {
    pub fn new(r: R) -> Self {
        Self { r: r }
    }
}

impl<R: Read> Read for Encoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // minimum of 1 bytes will be taken by hexadecimal string chunk_size plus 2 more for crlf and 2 more at the end of chunk
        if buf.len() < 6 {
            return Ok(0);
        }

        // We will determine the maximum chunk_size that can be filled in the buffer before processing here.
        // Algo :
        //  n = buf.len()
        //  - i = bytes to represent chunk_size in hexadecimal = 1
        // loop -
        //  - l = chunk_size = n - i
        //  - check if l can be represent in i bytes in hexadecimal string
        //  - true mean

        let n = buf.len() - 4;
        let mut i = 1;
        loop {
            let l = n - i;
            let chunk_size_hex = if l < 10 {
                format!("{:x}", l)
            } else {
                format!("{:01x}", l)
            };

            let chunk_size_str_bytes = chunk_size_hex.bytes().len();

            if chunk_size_str_bytes == i {
                break;
            }

            i += 1;
        }

        let req_chunk_size = n - i;
        let mut chunk = vec![0u8; n - i];

        // current required bytes to read from underlying source
        // remaining bytes to read from remaining source
        let mut req_size = req_chunk_size;

        // getting the chunk
        while req_size > 0 {
            let returned_bytes = self.r.read(&mut chunk[(req_chunk_size - req_size)..])?;

            if returned_bytes == 0 {
                break;
            }
            req_size -= returned_bytes;
        }

        // chunk processing
        let chunk_size = req_chunk_size - req_size;
        let chunk_size_hex = if chunk_size < 10 {
            format!("{:x}", chunk_size)
        } else {
            format!("{:02x}", chunk_size)
        };
        let chunk_size_len = chunk_size_hex.as_bytes().len();

        buf[0..chunk_size_len].copy_from_slice(chunk_size_hex.as_bytes());
        buf[chunk_size_len..chunk_size_len + 2].copy_from_slice(b"\r\n");
        buf[chunk_size_len + 2..chunk_size_len + 2 + chunk_size]
            .copy_from_slice(&chunk[0..chunk_size]);
        buf[chunk_size_len + 2 + chunk_size..chunk_size_len + 2 + chunk_size + 2]
            .copy_from_slice(b"\r\n");

        Ok(chunk_size + 4 + chunk_size_len)
    }
}

#[cfg(test)]
mod test {
    use std::io::Read;

    use crate::encoder;

    #[test]
    fn works_correctly() {
        let payload = b"1234567890";
        let expected = b"0a\r\n1234567890\r\n";

        let mut encoder = encoder::reader::Encoder::new(&payload[..]);

        let mut buf = vec![0u8; 16];
        let read_bytes = encoder.read(&mut buf).unwrap();

        assert_eq!(&buf[0..read_bytes], expected);
    }
}
