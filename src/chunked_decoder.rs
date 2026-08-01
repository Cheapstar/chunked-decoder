/// Represent the decoder state machine
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
    ChunkDataCrlf,
    /// Decoding Finished
    Done,
}

/// Represents the errors occured during parsing
#[derive(Debug)]
pub enum ChunkedDecoderError {
    /// Chunk-size must be hexadecimal decimal string
    InvalidChunkSizeFormat,
    /// Chunk-size must be valid hexadecimal
    InvalidLength,
    /// Incorrect Chunked Encoding
    IncorrectEncoding,
    /// Chunk is not correctly encoded
    MaliciousChunk,
}

/// A streaming parser for `Transfer-Encoding: chunked` request bodies.
///
/// `ChunkedDecoder` is designed to be fed data incrementally via [`decode`],
/// rather than requiring the entire body to be loaded into memory up front.
/// Small parts are buffered in memory and as soon as the any data is processed it
/// can be retreived via [`get_processed_chunks`].
///
///
/// [`decode`]: ChunkedDecoder::decode
/// [`get_processed_chunks`]: ChunkedDecoder::get_processed_chunks
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
