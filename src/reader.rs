use std::io::{BufRead, BufReader};

use crate::chunked_decoder::ChunkedDecoder;

/// Read from underlying reader and parse it and then push it to the specified buffer
pub struct DecoderReader<R: std::io::Read> {
    reader: BufReader<R>,
    chunk_decoder: ChunkedDecoder,
}

impl<R: std::io::Read> DecoderReader<R> {
    pub fn new(r: R) -> Self {
        Self {
            reader: BufReader::new(r),
            chunk_decoder: ChunkedDecoder::new(),
        }
    }
}

impl<R: std::io::Read> std::io::Read for DecoderReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let inner_buf = self.reader.fill_buf()?;

        // No data left to read
        if inner_buf.len() == 0 && self.chunk_decoder.processed_chunk_length() == 0 {
            return Ok(0);
        }

        let inner_buf_len = inner_buf.len();
        self.chunk_decoder
            .decode(inner_buf)
            .map_err(|_| std::io::ErrorKind::Other)?;

        let processed_chunk = self.chunk_decoder.get_processed_chunk(Some(buf.len()));

        // consume the read bytes
        self.reader.consume(inner_buf_len);

        if processed_chunk.len() != 0 {
            buf.copy_from_slice(&processed_chunk);
        }

        Ok(inner_buf_len)
    }
}

#[cfg(test)]
mod test {
    use std::io::Read;

    use crate::reader::DecoderReader;

    #[test]
    fn test_working_correctly() {
        let payload = b"1a\r\n01234567890123456789012345\r\n0\r\n\r\n";
        let mut decoderReader = DecoderReader::new(payload.as_ref());

        let expected = b"01234567890123456789012345\r\n".to_vec();
        let mut first_part = vec![0u8; 2];
        let mut final_part = vec![];

        for i in 0..14 {
            decoderReader.read(&mut first_part);
            final_part.extend_from_slice(&first_part);
        }

        assert_eq!(final_part, expected);
    }
}
