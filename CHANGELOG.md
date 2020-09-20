# Version 1.4.1

- Make `TcpStream` and `UnixStream` implement `Sync`.

# Version 1.4.0

- Remove `AsyncRead`/`AsyncWrite` impls for `&TcpStream`/`&UnixStream`
  (technically a breaking change, but the existence of these impls is a bug)

# Version 1.3.0

- Add type converstions using `From` and `TryFrom` impls.

# Version 1.2.0

- Update `blocking` and `async-io` to v1.0

# Version 1.1.0

- Reexport `AddrParseError`.

# Version 1.0.0

- Add `resolve()`.
- Re-export more types from `std::net`.

# Version 0.1.2

- Update `blocking` to v0.5.0

# Version 0.1.1

- Reduce the number of dependencies

# Version 0.1.0

- Initial version
