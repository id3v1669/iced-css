# Unix filename byte fixture

Input is slash followed by bytes 1 through 255, omitting slash (47).
`all-octets-uri.txt` is generated independently with Python standard-library
`urllib.parse.quote_from_bytes(input, safe="/")`, prefixed by `file://`.
The trailing newline is fixture framing, not URI content. The Rust test compares
encoding against this literal before testing a round trip.
