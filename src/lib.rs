//! / A streaming parser for `Transfer-Encoding: chunked` Https/1.1 and Https request bodies.
//!
//! `ChunkedDecoder` is designed to be fed data incrementally via [`decode`],
//! rather than requiring the entire body to be loaded into memory up front.
//! Small parts are buffered in memory and as soon as the any data is processed it
//! can be retreived via [`get_processed_chunks`] continuously.
//!
//!
//!
//! [`decode`]: ChunkedDecoder::decode
//! [`get_processed_chunks`]: ChunkedDecoder::get_processed_chunks
//!
//! # Example
//! ```no_run
//!     let mut decoder = ChunkedDecoder::new();
//!     let payload = "8\r\nTesting \r\n9;ext=true\r\na robust \r\n7\r\nparser!\r\n0\r\nExpires: Sun, 02 Aug 2026 02:00:00 GMT\r\n\r\n";
//!     let mut out = Vec::new();
//!     // byte by byte processing
//!     for b in payload {
//!         decoder.decode(&[*b]).unwrap();
//!         out.extend(decoder.get_processed_chunk());
//!     }
//! ```
//!
//! //! # Errors
//!
//! Parsing failures are reported via [`ChunkedDecoderError`].

pub mod chunked_decoder;

#[cfg(test)]
mod tests {
    use super::*;
    use chunked_decoder::ChunkedDecoder;

    fn decode_all_at_once(payload: &[u8]) -> Vec<u8> {
        let mut decoder = ChunkedDecoder::new();
        decoder.decode(payload).unwrap();
        decoder.get_processed_chunk()
    }

    fn decode_byte_by_byte(payload: &[u8]) -> Vec<u8> {
        let mut decoder = ChunkedDecoder::new();
        let mut out = Vec::new();
        for b in payload {
            decoder.decode(&[*b]).unwrap();
            out.extend(decoder.get_processed_chunk());
        }
        out
    }

    fn decode_split_at(payload: &[u8], at: usize) -> Vec<u8> {
        let mut decoder = ChunkedDecoder::new();
        let (part1, part2) = payload.split_at(at);
        decoder.decode(part1).unwrap();
        let mut out = decoder.get_processed_chunk();
        decoder.decode(part2).unwrap();
        out.extend(decoder.get_processed_chunk());
        out
    }

    #[test]
    fn single_call_whole_payload() {
        let payload = b"7\r\nMozilla\r\n9\r\nDeveloper\r\n7\r\nNetwork\r\n0\r\n\r\n";
        let result = decode_all_at_once(payload);
        assert_eq!(result, b"Mozilla\r\nDeveloper\r\nNetwork\r\n");
    }

    #[test]
    fn single_chunk() {
        let payload = b"5\r\nhello\r\n0\r\n\r\n";
        assert_eq!(decode_all_at_once(payload), b"hello\r\n");
    }

    #[test]
    fn empty_body() {
        let payload = b"0\r\n\r\n";
        assert_eq!(decode_all_at_once(payload), b"");
    }

    #[test]
    fn uppercase_hex_length() {
        // chunk sizes are case-insensitive hex
        let payload = b"A\r\n0123456789\r\n0\r\n\r\n";
        assert_eq!(decode_all_at_once(payload), b"0123456789\r\n");
    }

    #[test]
    fn lowercase_hex_length() {
        let payload = b"1a\r\n01234567890123456789012345\r\n0\r\n\r\n";
        assert_eq!(
            decode_all_at_once(payload),
            b"01234567890123456789012345\r\n"
        );
    }

    #[test]
    fn split_mid_chunk_data() {
        let payload = b"7\r\nMozilla\r\n9\r\nDeveloper\r\n7\r\nNetwork\r\n0\r\n\r\n";
        let expected = b"Mozilla\r\nDeveloper\r\nNetwork\r\n".to_vec();
        assert_eq!(decode_split_at(payload, 10), expected);
    }

    #[test]
    fn split_mid_chunk_size_line() {
        // splits right after "1" of chunk size "1a"
        let payload = b"1a\r\n01234567890123456789012345\r\n0\r\n\r\n";
        assert_eq!(
            decode_split_at(payload, 1),
            b"01234567890123456789012345\r\n".to_vec()
        );
    }

    #[test]
    fn split_on_crlf_after_length() {
        // splits between \r and \n after "7"
        let payload = b"7\r\nMozilla\r\n0\r\n\r\n";
        assert_eq!(decode_split_at(payload, 2), b"Mozilla\r\n".to_vec());
    }

    #[test]
    fn split_on_crlf_after_chunk() {
        // splits between \r and \n right after chunk data ends
        let payload = b"7\r\nMozilla\r\n0\r\n\r\n";
        assert_eq!(decode_split_at(payload, 10), b"Mozilla\r\n".to_vec());
    }

    #[test]
    fn split_at_every_byte_offset() {
        // try every possible split point and confirm same result
        let payload = b"7\r\nMozilla\r\n9\r\nDeveloper\r\n7\r\nNetwork\r\n0\r\n\r\n";
        let expected = b"Mozilla\r\nDeveloper\r\nNetwork\r\n".to_vec();
        for i in 1..payload.len() {
            let result = decode_split_at(payload, i);
            assert_eq!(result, expected, "failed at split offset {}", i);
        }
    }

    #[test]
    fn byte_by_byte_delivery() {
        // worst case: one byte per decode() call
        let payload = b"7\r\nMozilla\r\n9\r\nDeveloper\r\n0\r\n\r\n";
        assert_eq!(decode_byte_by_byte(payload), b"Mozilla\r\nDeveloper\r\n");
    }

    #[test]
    fn binary_data_containing_cr_and_lf() {
        // chunk-data itself contains \r and \n bytes — must not be
        // mistaken for chunk framing.
        let data: &[u8] = b"ab\rcd\nef";
        let mut payload = Vec::new();
        payload.extend_from_slice(format!("{:x}\r\n", data.len()).as_bytes());
        payload.extend_from_slice(data);
        payload.extend_from_slice(b"\r\n0\r\n\r\n");

        let mut expected: Vec<u8> = Vec::new();
        expected.extend_from_slice(data);
        expected.extend_from_slice(b"\r\n");
        assert_eq!(decode_all_at_once(&payload), expected);
    }

    #[test]
    fn binary_data_split_around_embedded_cr() {
        let data: &[u8] = b"12\r34\r56\r78";
        let mut payload = Vec::new();
        payload.extend_from_slice(format!("{:x}\r\n", data.len()).as_bytes());
        payload.extend_from_slice(data);
        payload.extend_from_slice(b"\r\n0\r\n\r\n");

        // split right after one of the embedded \r bytes
        let split_point = 3 + data.len() / 2;

        let mut expected: Vec<u8> = Vec::new();
        expected.extend_from_slice(data);
        expected.extend_from_slice(b"\r\n");
        assert_eq!(decode_split_at(&payload, split_point), expected);
    }

    #[test]
    fn large_chunk() {
        let mut data = vec![b'x'; 100_000];
        let mut payload = Vec::new();
        payload.extend_from_slice(format!("{:x}\r\n", data.len()).as_bytes());
        payload.extend_from_slice(&data);
        payload.extend_from_slice(b"\r\n0\r\n\r\n");

        data.extend_from_slice(b"\r\n");
        assert_eq!(decode_all_at_once(&payload), data);
    }

    #[test]
    fn many_small_chunks() {
        let mut payload = Vec::new();
        let mut expected = Vec::new();
        for i in 0..50 {
            let piece = format!("chunk{}", i);
            payload.extend_from_slice(format!("{:x}\r\n", piece.len()).as_bytes());
            payload.extend_from_slice(piece.as_bytes());
            payload.extend_from_slice(b"\r\n");
            expected.extend_from_slice(piece.as_bytes());
            expected.extend_from_slice(b"\r\n");
        }
        payload.extend_from_slice(b"0\r\n\r\n");

        assert_eq!(decode_all_at_once(&payload), expected);
    }

    #[test]
    fn invalid_hex_length_errors() {
        let payload = b"zz\r\nhello\r\n0\r\n\r\n";
        let mut decoder = ChunkedDecoder::new();
        assert!(decoder.decode(payload).is_err());
    }

    #[test]
    fn missing_lf_after_cr_errors() {
        // \r not followed by \n after chunk size
        let payload = b"5\rXhello\r\n0\r\n\r\n";
        let mut decoder = ChunkedDecoder::new();
        assert!(decoder.decode(payload).is_err());
    }

    #[test]
    fn missing_crlf_after_chunk_data_errors() {
        // chunk-data not followed by \r\n
        let payload = b"5\r\nhelloXX0\r\n\r\n";
        let mut decoder = ChunkedDecoder::new();
        assert!(decoder.decode(payload).is_err());
    }

    #[test]
    fn truncated_payload_no_terminator() {
        let payload = b"5\r\nhello\r\n";
        let mut decoder = ChunkedDecoder::new();
        assert!(decoder.decode(payload).is_ok());
        assert_eq!(decoder.get_processed_chunk(), b"hello\r\n");
    }

    #[test]
    fn decode_after_terminator_does_not_hang() {
        let mut decoder = ChunkedDecoder::new();
        decoder.decode(b"5\r\nhello\r\n0\r\n\r\n").unwrap();
        let result = decoder.decode(b"");
        assert!(result.is_ok());
    }
}
