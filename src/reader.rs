use std::vec;

use crate::decoder::{ChunkedDecoder, ChunkedDecoderState};

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

        // Only get the required length
        let processed_chunk = self
            .chunk_decoder
            .get_processed_chunk(Some(buf.len() - ret_size));

        if processed_chunk.len() != 0 {
            buf[ret_size..ret_size + processed_chunk.len()].copy_from_slice(&processed_chunk);
        }
        ret_size += processed_chunk.len();

        while ret_size < buf.len() {
            if self.chunk_decoder.get_state() == &ChunkedDecoderState::Done {
                break;
            }

            // read the required number of bytes from underlying source
            let returned_bytes_size = self.reader.read(&mut new_buf)?;

            if returned_bytes_size == 0 && self.chunk_decoder.processed_chunk_length() == 0 {
                // No data left to read
                return Ok(0);
            }

            // Decode the returned bytes
            self.chunk_decoder
                .decode(&new_buf[..returned_bytes_size])
                .map_err(|_| std::io::ErrorKind::Other)?;

            // Only get the required length
            let processed_chunk = self
                .chunk_decoder
                .get_processed_chunk(Some(buf.len() - ret_size));

            if processed_chunk.len() != 0 {
                buf[ret_size..ret_size + processed_chunk.len()].copy_from_slice(&processed_chunk);
            }
            ret_size += processed_chunk.len();
        }

        Ok(ret_size)
    }
}

/// The former name of [`Decoder`].
#[deprecated(note = "use reader::Decoder instead")]
pub type DecoderReader<R> = Decoder<R>;

#[cfg(test)]
mod test {
    use std::io::{self, Read};

    use crate::reader::Decoder;
    #[allow(deprecated)]
    use crate::reader::DecoderReader;

    /// Helper custom reader that yields at most `chunk_size` bytes per read call.
    struct ChunkedInputReader<'a> {
        data: &'a [u8],
        chunk_size: usize,
        position: usize,
    }

    impl<'a> ChunkedInputReader<'a> {
        fn new(data: &'a [u8], chunk_size: usize) -> Self {
            Self {
                data,
                chunk_size,
                position: 0,
            }
        }
    }

    impl<'a> Read for ChunkedInputReader<'a> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.position >= self.data.len() {
                return Ok(0);
            }
            let remaining = self.data.len() - self.position;
            let to_read = remaining.min(self.chunk_size).min(buf.len());
            buf[..to_read].copy_from_slice(&self.data[self.position..self.position + to_read]);
            self.position += to_read;
            Ok(to_read)
        }
    }

    /// Helper custom reader that yields an error on demand.
    struct FailingReader<'a> {
        data: &'a [u8],
        position: usize,
        fail_at: usize,
        error_kind: io::ErrorKind,
    }

    impl<'a> FailingReader<'a> {
        fn new(data: &'a [u8], fail_at: usize, error_kind: io::ErrorKind) -> Self {
            Self {
                data,
                position: 0,
                fail_at,
                error_kind,
            }
        }
    }

    impl<'a> Read for FailingReader<'a> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.position >= self.fail_at {
                return Err(io::Error::new(self.error_kind, "simulated reader error"));
            }
            let remaining = self.fail_at - self.position;
            let to_read = remaining.min(buf.len());
            buf[..to_read].copy_from_slice(&self.data[self.position..self.position + to_read]);
            self.position += to_read;
            Ok(to_read)
        }
    }

    #[test]
    fn test_working_correctly() {
        let payload = b"1b\r\n012345678901234567890123452\r\n0\r\n\r\n";
        let mut decoder_reader = Decoder::new(payload.as_ref());

        let expected = b"012345678901234567890123452".to_vec();
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

    #[test]
    fn test_read_to_end_single_chunk() {
        let payload = b"5\r\nhello\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"hello");
    }

    #[test]
    fn test_read_to_end_multiple_chunks() {
        let payload = b"7\r\nMozilla\r\n9\r\nDeveloper\r\n7\r\nNetwork\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"MozillaDeveloperNetwork");
    }

    #[test]
    fn test_read_to_string() {
        let payload = b"b\r\nhello world\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut s = String::new();
        decoder.read_to_string(&mut s).unwrap();
        assert_eq!(s, "hello world");
    }

    #[test]
    fn test_empty_body() {
        let payload = b"0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut out = Vec::new();
        let bytes_read = decoder.read_to_end(&mut out).unwrap();
        assert_eq!(bytes_read, 0);
        assert_eq!(out, b"");
    }

    #[test]
    fn test_zero_length_buffer() {
        let payload = b"5\r\nhello\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());

        // Zero-length buffer read must return Ok(0)
        let mut empty = [];
        let n = decoder.read(&mut empty).unwrap();
        assert_eq!(n, 0);

        // Subsequent reads should continue normally
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"hello");
    }

    #[test]
    fn test_read_byte_by_byte() {
        let payload = b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut out = Vec::new();
        let mut one_byte = [0u8; 1];

        loop {
            let n = decoder.read(&mut one_byte).unwrap();
            if n == 0 {
                break;
            }
            out.push(one_byte[0]);
        }

        assert_eq!(out, b"Wikipedia");
    }

    #[test]
    fn test_various_buffer_sizes() {
        let payload = b"7\r\nMozilla\r\n9\r\nDeveloper\r\n7\r\nNetwork\r\n0\r\n\r\n";
        let expected = b"MozillaDeveloperNetwork";

        for buf_size in [1, 2, 3, 4, 5, 7, 8, 11, 16, 32, 64, 128] {
            let mut decoder = Decoder::new(payload.as_ref());
            let mut out = Vec::new();
            let mut buf = vec![0u8; buf_size];

            loop {
                let n = decoder.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                out.extend_from_slice(&buf[..n]);
            }

            assert_eq!(out, expected, "Failed with read buffer size: {}", buf_size);
        }
    }

    #[test]
    fn test_buffer_larger_than_payload() {
        let payload = b"5\r\nhello\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut large_buf = vec![0u8; 8192];

        let n = decoder.read(&mut large_buf).unwrap();
        assert_eq!(&large_buf[..n], b"hello");

        // Next read should return 0 (EOF)
        let n2 = decoder.read(&mut large_buf).unwrap();
        assert_eq!(n2, 0);
    }

    #[test]
    fn test_exact_buffer_size() {
        let payload = b"5\r\nhello\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        // Expected output length is 7 ("hello\r\n")
        let mut exact_buf = vec![0u8; 5];

        let n = decoder.read(&mut exact_buf).unwrap();
        assert_eq!(n, 5);
        assert_eq!(&exact_buf[..n], b"hello");

        let n2 = decoder.read(&mut exact_buf).unwrap();
        assert_eq!(n2, 0);
    }

    #[test]
    fn test_chunk_with_extensions() {
        let payload = b"5;foo=bar;baz=1\r\nhello\r\n0;flag\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"hello");
    }

    #[test]
    fn test_case_insensitive_hex_lengths() {
        let payload = b"a\r\n0123456789\r\nB\r\n0123456789A\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"01234567890123456789A");
    }

    #[test]
    fn test_binary_data_containing_crlf() {
        let data: &[u8] = b"ab\rcd\nef\r\ngh\0ij\xff";
        let mut payload = Vec::new();
        payload.extend_from_slice(format!("{:x}\r\n", data.len()).as_bytes());
        payload.extend_from_slice(data);
        payload.extend_from_slice(b"\r\n0\r\n\r\n");

        let mut decoder = Decoder::new(payload.as_slice());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();

        let mut expected = data.to_vec();
        assert_eq!(out, expected);
    }

    #[test]
    fn test_large_chunk() {
        let data = vec![b'Z'; 65_536];
        let mut payload = Vec::new();
        payload.extend_from_slice(format!("{:x}\r\n", data.len()).as_bytes());
        payload.extend_from_slice(&data);
        payload.extend_from_slice(b"\r\n0\r\n\r\n");

        let mut decoder = Decoder::new(payload.as_slice());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();

        let mut expected = data;
        assert_eq!(out, expected);
    }

    #[test]
    fn test_many_small_chunks() {
        let mut payload = Vec::new();
        let mut expected = Vec::new();

        for i in 0..40 {
            let chunk = format!("part-{}", i);
            payload.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
            payload.extend_from_slice(chunk.as_bytes());
            payload.extend_from_slice(b"\r\n");

            expected.extend_from_slice(chunk.as_bytes());
        }
        payload.extend_from_slice(b"0\r\n\r\n");

        let mut decoder = Decoder::new(payload.as_slice());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();

        assert_eq!(out, expected);
    }

    #[test]
    fn test_repeated_read_after_eof() {
        let payload = b"3\r\nhey\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"hey");

        let mut buf = [0u8; 10];
        for _ in 0..5 {
            assert_eq!(decoder.read(&mut buf).unwrap(), 0);
        }
    }

    #[test]
    fn test_slow_byte_by_byte_reader() {
        let payload = b"5\r\nhello\r\n5\r\nworld\r\n0\r\n\r\n";
        let slow_reader = ChunkedInputReader::new(payload, 1);
        let mut decoder = Decoder::new(slow_reader);

        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"helloworld");
    }

    #[test]
    fn test_fragmented_reader_arbitrary_chunks() {
        let payload = b"7\r\nMozilla\r\n9\r\nDeveloper\r\n0\r\n\r\n";
        for chunk_size in [1, 2, 3, 5, 7] {
            let chunked_reader = ChunkedInputReader::new(payload, chunk_size);
            let mut decoder = Decoder::new(chunked_reader);

            let mut out = Vec::new();
            decoder.read_to_end(&mut out).unwrap();
            assert_eq!(
                out, b"MozillaDeveloper",
                "Failed with input chunk size {}",
                chunk_size
            );
        }
    }

    #[test]
    fn test_cursor_reader() {
        let payload = b"4\r\ntest\r\n0\r\n\r\n";
        let cursor = io::Cursor::new(payload);
        let mut decoder = Decoder::new(cursor);
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"test");
    }

    #[test]
    fn test_deprecated_decoder_reader_type_alias() {
        let payload = b"4\r\nrust\r\n0\r\n\r\n";
        #[allow(deprecated)]
        let mut decoder: DecoderReader<&[u8]> = DecoderReader::new(payload.as_ref());
        let mut out = Vec::new();
        decoder.read_to_end(&mut out).unwrap();
        assert_eq!(out, b"rust");
    }

    // --- Error checking tests ---

    #[test]
    fn test_invalid_hex_chunk_size_error() {
        let payload = b"zz\r\nhello\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut buf = [0u8; 10];
        let err = decoder.read(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn test_invalid_hex_symbol_error() {
        let payload = b"1g\r\nhello\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut buf = [0u8; 10];
        let err = decoder.read(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn test_missing_lf_after_cr_in_chunk_size_error() {
        let payload = b"5\rXhello\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut buf = [0u8; 10];
        let err = decoder.read(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn test_missing_crlf_after_chunk_data_error() {
        let payload = b"5\r\nhelloXX0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut buf = [0u8; 10];
        let err = decoder.read(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn test_non_utf8_chunk_size_error() {
        let payload = b"\xff\xff\r\nhello\r\n0\r\n\r\n";
        let mut decoder = Decoder::new(payload.as_ref());
        let mut buf = [0u8; 10];
        let err = decoder.read(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Other);
    }

    #[test]
    fn test_underlying_reader_immediate_error_propagation() {
        let payload = b"5\r\nhello\r\n0\r\n\r\n";
        let failing_reader = FailingReader::new(payload, 0, io::ErrorKind::PermissionDenied);
        let mut decoder = Decoder::new(failing_reader);
        let mut buf = [0u8; 10];
        let err = decoder.read(&mut buf).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn test_underlying_reader_error_mid_stream_propagation() {
        let payload = b"5\r\nhello\r\n5\r\nworld\r\n0\r\n\r\n";
        // Fail after reading the first chunk ("5\r\nhello\r\n" is 12 bytes)
        let failing_reader = FailingReader::new(payload, 12, io::ErrorKind::BrokenPipe);
        let mut decoder = Decoder::new(failing_reader);

        let mut out = Vec::new();
        let result = decoder.read_to_end(&mut out);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }
}
