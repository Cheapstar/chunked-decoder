# ChunkedDecoder

A streaming parser for `Transfer-Encoding: chunked` HTTP/1.1 and HTTPS request bodies.

`ChunkedDecoder` is designed to be fed data incrementally via [`decode`](#decode), rather than requiring the entire body to be loaded into memory up front. Small parts are buffered in memory, and as soon as any data is processed it can be retrieved via [`get_processed_chunk`](#get_processed_chunk) continuously.

## Example

```rust
let mut decoder = ChunkedDecoder::new();
let payload = "8\r\nTesting \r\n9;ext=true\r\na robust \r\n7\r\nparser!\r\n0\r\nExpires: Sun, 02 Aug 2026 02:00:00 GMT\r\n\r\n";
let mut out = Vec::new();

// byte by byte processing
for b in payload.as_bytes() {
    decoder.decode(&[*b]).unwrap();
    out.extend(decoder.get_processed_chunk());
}
```

## Installation

Run:

```bash
cargo add chunked_decoder
```

Or add it to your `Cargo.toml` manually:

```toml
[dependencies]
chunked_decoder = "0.1"
```

## API

### `decode`

```rust
pub fn decode(&mut self, chunk: &[u8]) -> Result<(), ChunkedDecoderError>
```

Takes the chunk incrementally and processes it for chunked-encoding, storing the processed chunks which can be retrieved continuously via [`get_processed_chunk`](#get_processed_chunk).

**Arguments**

| Argument | Description                      |
| -------- | -------------------------------- |
| `chunk`  | Chunk of body bytes to be parsed |

**Errors**

Returns a [`ChunkedDecoderError`](#chunkeddecodererror) on failure.

**Note**

This only removes the line containing `chunk_size` — nothing else:

```
chunk_size\r\n
chunk\r\n                        chunk\r\n
chunk_size\r\n                   chunk\r\n
chunk\r\n               =>       chunk\r\n
chunk_size\r\n                   chunk\r\n
chunk\r\n
chunk_size\r\n
chunk\r\n
```

### `get_processed_chunk`

To Retrieve processed chunks continuously.

**Returns**

A vector of processed body bytes.

## Errors

Parsing failures are reported via `ChunkedDecoderError`:

| Variant                  | Description                                     |
| ------------------------ | ----------------------------------------------- |
| `InvalidChunkSizeFormat` | Chunk-size must be a hexadecimal decimal string |
| `InvalidLength`          | Chunk-size must be a valid hexadecimal value    |
| `IncorrectEncoding`      | Incorrect chunked encoding                      |
| `MaliciousChunk`         | Chunk is not correctly encoded                  |

## Documentation

Full API documentation is available on [docs.rs](https://docs.rs/chunked_decoder).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)

Copyright [2026] Cheapstar
