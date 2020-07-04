//! Async networking primitives for TCP/UDP/Unix communication.
//!
//! This crate is an async version of [`std::net`] and [`std::os::unix::net`].
//!
//! # Implementation
//!
//! This crate uses [`async-io`] for async I/O and [`blocking`] for DNS lookups.
//!
//! [`async-io`]: https://docs.rs/async-io
//! [`blocking`]: https://docs.rs/blocking
//!
//! # Examples
//!
//! A simple UDP server that echoes messages back to the sender:
//!
//! ```no_run
//! use async_net::UdpSocket;
//!
//! # blocking::block_on(async {
//! let socket = UdpSocket::bind("127.0.0.1:8080").await?;
//! let mut buf = vec![0u8; 1024];
//!
//! loop {
//!     let (n, peer) = socket.recv_from(&mut buf).await?;
//!     socket.send_to(&buf[..n], &peer).await?;
//! }
//! # std::io::Result::Ok(()) });
//! ```

#[cfg(unix)]
pub mod unix;

mod addr;
mod tcp;
mod udp;

pub use tcp::{Incoming, TcpListener, TcpStream};
pub use udp::UdpSocket;
pub use addr::AsyncToSocketAddrs;
