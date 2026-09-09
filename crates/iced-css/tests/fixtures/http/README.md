These are public, test-only localhost TLS fixtures. The private key is deliberately
part of the test suite and must never be used outside loopback tests. Production
trust uses webpki roots plus explicitly supplied application roots, not this CA.

The leaf is valid from 2026-09-07 04:50:32 UTC to 2036-09-04 04:50:32 UTC and has
only DNS:localhost as its subject alternative name. The wrong-host test connects
to 127.0.0.1 and must fail normal hostname verification. Tests do not disable time,
hostname or certificate validation; regenerate fixtures when the interval expires.

Generation commands and OpenSSL output are retained in
`docs/tmp/implementation-http-transport/certificates/`. SHA256:

- ca.der: da746f4794b590f9bbc1ac1ab2f0855c1de27e0e4cd430ccd227d35d07869495
- leaf.der: 3ecaaa0fb5213c4d5ced9c8ef5e06c148585b2c2ac4ec1e015869d56b5d0f9f8
- leaf-key.der: 7533c6889f5fc7776b8c362a8b2a5be679e5dfe39328c62b02bbdc536df20c99
