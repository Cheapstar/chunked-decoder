# chunked-decoder

Streaming encoders and decoders for HTTP/1.1 `Transfer-Encoding: chunked` bodies.

Decode body bytes incrementally with `decoder::ChunkedDecoder`, or wrap a `std::io::Read` or `std::io::Write` implementation to encode or decode a stream. Decoding removes chunk sizes and framing CRLF bytes, returning only the body data. CRLF bytes within the body data are preserved.

## Installation

Requires Rust 1.87 or later.

```bash
cargo add chunked-decoder
```

Or add it to your `Cargo.toml`:

```toml
[dependencies]
chunked-decoder = "0.3.1"
```

Use `chunked_decoder` (with an underscore) in Rust imports.

## API overview

| Type | Purpose |
| ---- | ------- |
| `decoder::ChunkedDecoder` | Incrementally decode byte slices and retrieve buffered body data. |
| `reader::Decoder<R: Read>` | Read decoded body data from a reader containing chunked bytes. |
| `writer::Decoder<W: Write>` | Write chunked bytes and forward decoded body data to a writer. |
| `encoder::reader::Encoder<R: Read>` | Read chunked bytes generated from an underlying body reader. |
| `encoder::writer::Encoder<W: Write>` | Write body data and forward chunked bytes to a writer. |

All paths are relative to `chunked_decoder`. Each I/O adapter has a `new(inner)` constructor and implements the corresponding `std::io` trait. The former names `reader::DecoderReader` and `writer::DecoderWriter` remain as deprecated aliases.

## Incremental decoding

```rust
use chunked_decoder::decoder::{ChunkedDecoder, ChunkedDecoderState};

fn main() {
    let mut decoder = ChunkedDecoder::new();
    let payload = b"5\r\nhello\r\n6;ext=true\r\n world\r\n0\r\n\r\n";
    let mut body = Vec::new();

    // Input can arrive one byte at a time or in larger slices.
    for byte in payload {
        decoder.decode(&[*byte]).unwrap();
        body.extend(decoder.get_processed_chunk(None));
    }

    assert_eq!(body, b"hello world");
    assert!(decoder.get_state() == &ChunkedDecoderState::Done);
}
```

Import the low-level types from `chunked_decoder::decoder`.

| Method | Behavior |
| ------ | -------- |
| `ChunkedDecoder::new() -> Self` | Create a decoder in the `ChunkSize` state. |
| `decode(&mut self, chunk: &[u8]) -> Result<(), ChunkedDecoderError>` | Process another slice of chunked input, buffering incomplete framing and decoded body bytes. |
| `get_processed_chunk(&mut self, len: Option<usize>) -> Vec<u8>` | Drain all available body bytes with `None`, or up to `n` bytes with `Some(n)`. Remaining bytes stay buffered. |
| `processed_chunk_length(&self) -> usize` | Return the number of decoded body bytes waiting to be retrieved. |
| `get_state(&self) -> &ChunkedDecoderState` | Inspect the current parsing state. |

The states are `ChunkSize`, `ChunkSizeCrlf`, `Chunk`, `ChunkDataCrlf`, and `Done`. Drain the output regularly to keep decoded data from accumulating in memory.

`decode` accepts incomplete input, so `Ok(())` does not mean the body is complete. `Done` means the decoder has recognized a zero-size chunk; it does not validate the remaining terminal framing or parse trailers. Chunk extensions are ignored, and bytes supplied after `Done` are ignored. The API does not return unused input bytes, so pass only the chunked body to the decoder.

## Decoding with I/O adapters

### Reader

```rust
use std::io::{self, Read};
use chunked_decoder::reader::Decoder;

fn main() -> io::Result<()> {
    let payload = b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
    let mut decoder = Decoder::new(payload.as_slice());
    let mut body = String::new();

    decoder.read_to_string(&mut body)?;

    assert_eq!(body, "hello world");
    Ok(())
}
```

### Writer

```rust
use std::io::{self, Write};
use chunked_decoder::writer::Decoder;

fn main() -> io::Result<()> {
    let payload = b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
    let mut body = Vec::new();

    {
        let mut decoder = Decoder::new(&mut body);
        for part in payload.chunks(3) {
            decoder.write_all(part)?;
        }
        decoder.flush()?;
    }

    assert_eq!(body, b"hello world");
    Ok(())
}
```

The decoder writer's `flush()` drains decoded output but does not flush the underlying writer. Flush that writer separately when needed. Neither decoder adapter reports an incomplete body as an error merely because input ends; use the low-level decoder's state when you need to check whether a zero-size chunk was seen.

## Encoding with I/O adapters

### Writer

Each nonempty `write` encodes its input as one chunk. To finish a body without trailers, append `0\r\n\r\n` directly to the underlying writer after releasing the encoder.

```rust
use std::io::{self, Write};
use chunked_decoder::encoder::writer::Encoder;

fn main() -> io::Result<()> {
    let mut encoded = Vec::new();

    {
        let mut encoder = Encoder::new(&mut encoded);
        encoder.write_all(b"hello")?;
        encoder.write_all(b" world")?;
    }

    // Finish the chunked body on the underlying writer.
    encoded.write_all(b"0\r\n\r\n")?;
    encoded.flush()?;

    assert_eq!(encoded, b"5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n");
    Ok(())
}
```

The encoder writer's `flush()` is a no-op. The adapter does not emit the terminating chunk on flush, drop, or an empty write, and it never forwards `flush()` to the underlying writer.

### Reader

Use an explicit read loop and stop after the terminating chunk:

```rust
use std::io::{self, Read};
use chunked_decoder::encoder::reader::Encoder;

fn main() -> io::Result<()> {
    let mut encoder = Encoder::new(b"hello world".as_slice());
    let mut encoded = Vec::new();
    let mut buffer = [0u8; 1024];

    loop {
        let len = encoder.read(&mut buffer)?;
        let chunk = &buffer[..len];
        encoded.extend_from_slice(chunk);

        if chunk == b"0\r\n\r\n" {
            break;
        }
    }

    assert_eq!(encoded, b"0b\r\nhello world\r\n0\r\n\r\n");
    Ok(())
}
```

Use the 1024-byte output buffer shown above. Buffers smaller than 6 bytes return `Ok(0)` without reading input, and some other sizes (such as 15 bytes) can panic in the current implementation. With the buffer above, the reader emits `0\r\n\r\n` whenever the underlying reader is exhausted, including on subsequent reads. Stop at the first terminating chunk: `read_to_end`, `read_to_string`, and `std::io::copy` do not terminate on source EOF with this adapter.

## Errors

The low-level decoder returns `decoder::ChunkedDecoderError`:

| Variant | Meaning |
| ------- | ------- |
| `InvalidChunkSizeFormat` | The chunk-size line is not valid UTF-8. |
| `InvalidLength` | The chunk size cannot be parsed as a hexadecimal `usize`. |
| `IncorrectEncoding` | Invalid chunk framing was detected. |
| `MaliciousChunk` | Defined in the public API, but not currently emitted by the decoder. |

The decoder I/O adapters map parsing failures to `std::io::ErrorKind::Other`. Underlying I/O errors are propagated by the adapters.

## Documentation

Full API documentation is available on [docs.rs](https://docs.rs/chunked-decoder).

## License

Licensed under the Apache License, Version 2.0 ([LICENSE](LICENSE)).

Copyright [2026] Cheapstar
