/// Parser state for HTTP/1.1 chunked transfer-encoding.
///
/// Transitions:
/// ChunkSize -> ChunkSizeCrlf -> ChunkData -> ChunkDataCrlf -> ChunkSize -> ... -> Done
#[derive(PartialEq)]
enum ChunkedDecoderState {
    /// Decoding Chunk Length
    ChunkSize,
    /// Just consumed the chunk-size line; now expecting exactly "\r\n"
    /// before chunk-data begins.
    ChunkSizeCrlf,
    /// Decoding Chunk Data
    Chunk,
    /// Chunk-data fully consumed; now expecting the trailing "\r\n"
    /// that follows every chunk-data block before the next chunk-size line
    ChunkDataCrlf,
    /// Decoding Finished
    Done,
}

#[derive(Debug)]
pub enum ChunkedDecoderError {
    InvalidChunkSizeFormat,
    InvalidLength,
    IncorrectEncoding,
    MaliciousChunk,
}

pub struct ChunkedDecoder {
    processed_chunk: Vec<u8>,
    buffer: Vec<u8>,
    state: ChunkedDecoderState,
    curr_chunk_size: usize,
}

impl ChunkedDecoder {
    pub fn new() -> Self {
        Self {
            processed_chunk: Vec::new(),
            buffer: Vec::new(),
            state: ChunkedDecoderState::ChunkSize,
            curr_chunk_size: 0,
        }
    }

    pub fn decode(&mut self, chunk: &[u8]) -> Result<(), ChunkedDecoderError> {
        let mut index = 0;
        let mut new_chunk: Vec<u8> = Vec::new();

        if self.buffer.is_empty() {
            new_chunk.extend_from_slice(chunk);
        } else {
            new_chunk.extend_from_slice(&self.buffer);
            new_chunk.extend_from_slice(chunk);

            self.buffer.clear();
        }

        loop {
            if self.state == ChunkedDecoderState::Done {
                break;
            }
            if self.state == ChunkedDecoderState::ChunkSize {
                let newline_exists = find_cr(&new_chunk[index..]);

                if newline_exists.is_none() {
                    self.buffer.extend_from_slice(&new_chunk[index..]);
                    break;
                }

                let newline_pos = newline_exists.unwrap();

                let chunk_size_bytes = &new_chunk[index..index + newline_pos];
                let chunk_size_str = str::from_utf8(chunk_size_bytes)
                    .map_err(|_| ChunkedDecoderError::InvalidChunkSizeFormat)?;
                let chunk_size = usize::from_str_radix(chunk_size_str, 16)
                    .map_err(|_| ChunkedDecoderError::InvalidLength)?;

                if chunk_size == 0 {
                    self.state = ChunkedDecoderState::Done;
                    break;
                }

                self.curr_chunk_size = chunk_size;
                self.state = ChunkedDecoderState::ChunkSizeCrlf;
                index += newline_pos;
            }

            // Just After Chunk Size  -> "\r\n"
            if self.state == ChunkedDecoderState::ChunkSizeCrlf {
                if new_chunk.len() - index < 2 {
                    self.buffer.extend_from_slice(&new_chunk[index..]);
                    break;
                }

                if new_chunk[index] != b'\r' || new_chunk[index + 1] != b'\n' {
                    return Err(ChunkedDecoderError::IncorrectEncoding);
                }

                index += 2; // start of the chunk
                self.state = ChunkedDecoderState::Chunk;
            }

            if self.state == ChunkedDecoderState::Chunk {
                let new_chunk_size = new_chunk.len() - index;

                if new_chunk_size <= self.curr_chunk_size {
                    // new_chunk will be fully consumed
                    self.processed_chunk.extend_from_slice(&new_chunk[index..]);

                    // update the chunk_size
                    self.curr_chunk_size -= new_chunk_size;

                    if self.curr_chunk_size == 0 {
                        self.state = ChunkedDecoderState::ChunkDataCrlf;
                    }

                    break;
                }

                self.processed_chunk
                    .extend_from_slice(&new_chunk[index..index + self.curr_chunk_size]);
                index += self.curr_chunk_size;

                self.state = ChunkedDecoderState::ChunkDataCrlf;
                self.curr_chunk_size = 0
            }

            if self.state == ChunkedDecoderState::ChunkDataCrlf {
                if new_chunk.len() - index < 2 {
                    self.buffer.extend_from_slice(&new_chunk[index..]);
                    break;
                }

                if new_chunk[index] != b'\r' && new_chunk[index + 1] != b'\n' {
                    return Err(ChunkedDecoderError::IncorrectEncoding);
                }

                self.state = ChunkedDecoderState::ChunkSize;

                self.processed_chunk.extend_from_slice(b"\r\n");
                index += 2; // start of the chunk
            }
        }

        Ok(())
    }

    pub fn get_processed_chunk(&mut self) -> Vec<u8> {
        let mut ret = Vec::new();
        ret.append(&mut self.processed_chunk);
        ret
    }
}

fn find_cr(chunk: &[u8]) -> Option<usize> {
    chunk.windows(1).position(|item| item == b"\r")
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
