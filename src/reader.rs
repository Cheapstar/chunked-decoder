use std::vec;

use crate::chunked_decoder::{ChunkedDecoder, ChunkedDecoderState};

/// Decodes a chunked body while reading from the underlying reader.
pub struct Decoder<R: std::io::Read> {
    reader: R,
    chunk_decoder: ChunkedDecoder,
}

impl<R: std::io::Read> Decoder<R> {
    pub fn new(r: R) -> Self {
        Self {
            reader: r,
            chunk_decoder: ChunkedDecoder::new(),
        }
    }
}

impl<R: std::io::Read> std::io::Read for Decoder<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // initialize a new_buf
        let mut new_buf = vec![0u8; buf.len()];

        let mut ret_size = 0;
        while ret_size == 0 {
            if self.chunk_decoder.get_state() == &ChunkedDecoderState::Done {
                break;
            }

            // read the required number of bytes
            let returned_bytes_size = self.reader.read(&mut new_buf)?;

            if returned_bytes_size == 0 && self.chunk_decoder.processed_chunk_length() == 0 {
                // No data left to read
                return Ok(0);
            }

            self.chunk_decoder
                .decode(&new_buf[..returned_bytes_size])
                .map_err(|_| std::io::ErrorKind::Other)?;

            let processed_chunk = self.chunk_decoder.get_processed_chunk(Some(buf.len()));

            ret_size = processed_chunk.len();
            if processed_chunk.len() != 0 {
                buf[..ret_size].copy_from_slice(&processed_chunk);
            }
        }

        Ok(ret_size)
    }
}

/// The former name of [`Decoder`].
#[deprecated(note = "use reader::Decoder instead")]
pub type DecoderReader<R> = Decoder<R>;

#[cfg(test)]
mod test {
    use std::io::Read;

    use crate::reader::Decoder;

    #[test]
    fn test_working_correctly() {
        let payload = b"1a\r\n01234567890123456789012345\r\n0\r\n\r\n";
        let mut decoder_reader = Decoder::new(payload.as_ref());

        let expected = b"01234567890123456789012345\r\n".to_vec();
        let mut first_part = vec![0u8; 2];
        let mut final_part = vec![];

        let mut ret_size = 2;
        while ret_size != 0 {
            let r = decoder_reader.read(&mut first_part);
            ret_size = r.unwrap();
            if ret_size != 0 {
                final_part.extend_from_slice(&first_part[..ret_size]);
            }
        }

        assert_eq!(final_part, expected);
    }
}
