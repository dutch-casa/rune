- Static web exports accept escaped page titles and descriptions, preload wasm, and
  generate deterministic gzip assets plus Brotli variants when the CLI is available.
  Hosts must negotiate encodings with the original MIME type and `Vary: Accept-Encoding`.
  Keep original assets and deploy each export together. Compression runs at build time.
