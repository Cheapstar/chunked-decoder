use crate::chunked_decoder::ChunkedDecoder;

/// Decodes chunked data written to it and forwards the result to the underlying writer.
pub struct Decoder<W: std::io::Write> {
    writer: W,
    chunk_decoder: ChunkedDecoder,
}

impl<W: std::io::Write> Decoder<W> {
    pub fn new(w: W) -> Self {
        Self {
            writer: w,
            chunk_decoder: ChunkedDecoder::new(),
        }
    }
}

impl<W: std::io::Write> std::io::Write for Decoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.chunk_decoder
            .decode(buf)
            .map_err(|_| std::io::ErrorKind::Other)?;

        let inner_buf_len = self.chunk_decoder.processed_chunk_length();
        if inner_buf_len != 0 {
            let inner_buf = self.chunk_decoder.get_processed_chunk(None);
            self.writer.write_all(&inner_buf)?;
        }

        Ok(buf.len())
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

/// The former name of [`Decoder`].
#[deprecated(since = "0.2.2", note = "use writer::Decoder instead")]
pub type DecoderWriter<W> = Decoder<W>;

#[cfg(test)]
mod test {
    use std::io::Write;

    use crate::writer::Decoder;

    #[test]
    fn works_correctly() {
        let payload = b"1a\r\n012345678901234567890123452\r\n0\r\n\r\n";
        let mut some_writer = vec![];
        let mut decoder_writer = Decoder::new(&mut some_writer);

        let expected = b"012345678901234567890123452\r\n".to_vec();

        for iter in payload.chunks(2) {
            decoder_writer.write(iter);
        }

        decoder_writer.flush();

        assert_eq!(some_writer, expected);
    }
}
