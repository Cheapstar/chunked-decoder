use std::io::BufWriter;

use crate::chunked_decoder::ChunkedDecoder;

/// Decode the given buffer and write it to the underlying Writer
pub struct DecoderWriter<W: std::io::Write> {
    writer: BufWriter<W>,
    chunk_decoder: ChunkedDecoder,
}

impl<W: std::io::Write> DecoderWriter<W> {
    pub fn new(w: W) -> Self {
        Self {
            writer: BufWriter::new(w),
            chunk_decoder: ChunkedDecoder::new(),
        }
    }

    pub fn into_inner(self) -> std::io::Result<W> {
        self.writer.into_inner().map_err(|e| e.into_error())
    }
}

impl<W: std::io::Write> std::io::Write for DecoderWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.chunk_decoder
            .decode(buf)
            .map_err(|_| std::io::ErrorKind::Other)?;

        let inner_buf_len = self.chunk_decoder.processed_chunk_length();
        if inner_buf_len != 0 {
            let inner_buf = self.chunk_decoder.get_processed_chunk(None);
            self.writer.write_all(&inner_buf)?;
        }

        Ok(inner_buf_len)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        let inner_buf_len = self.chunk_decoder.processed_chunk_length();
        if inner_buf_len != 0 {
            let inner_buf = self.chunk_decoder.get_processed_chunk(None);
            self.writer.write_all(&inner_buf)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use crate::writer::DecoderWriter;

    #[test]
    fn works_correctly() {
        let payload = b"1a\r\n01234567890123456789012345\r\n0\r\n\r\n";
        let some_writer = vec![];
        let mut decoder_writer = DecoderWriter::new(some_writer);

        let expected = b"01234567890123456789012345\r\n".to_vec();

        for iter in payload.chunks(2) {
            decoder_writer.write(iter);
        }

        decoder_writer.flush();

        let final_writer = decoder_writer.into_inner().unwrap();

        assert_eq!(final_writer, expected);
    }
}
