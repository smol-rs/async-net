use std::io::{IoSlice, IoSliceMut};
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, RawSocket};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_io::Async;
use futures_lite::*;

use crate::addr::AsyncToSocketAddrs;

/// A TCP server, listening for connections.
///
/// After creating a [`TcpListener`] by [`bind`][`TcpListener::bind()`]ing it to an address, it
/// listens for incoming TCP connections. These can be accepted by calling
/// [`accept()`][`TcpListener::accept()`] or by awaiting items from the stream of
/// [`incoming`][`TcpListener::incoming()`] connections.
///
/// Cloning a [`TcpListener`] creates another handle to the same socket. The socket will be closed
/// when all handles to it are dropped.
///
/// The Transmission Control Protocol is specified in [IETF RFC 793].
///
/// [IETF RFC 793]: https://tools.ietf.org/html/rfc793
///
/// # Examples
///
/// ```no_run
/// use async_net::TcpListener;
/// use futures_lite::*;
///
/// # futures_lite::future::block_on(async {
/// let listener = TcpListener::bind("127.0.0.1:8080").await?;
/// let mut incoming = listener.incoming();
///
/// while let Some(stream) = incoming.next().await {
///     let mut stream = stream?;
///     stream.write_all(b"hello").await?;
/// }
/// # std::io::Result::Ok(()) });
/// ```
#[derive(Clone, Debug)]
pub struct TcpListener(Arc<Async<std::net::TcpListener>>);

impl TcpListener {
    /// Creates a new [`TcpListener`] bound to the given address.
    ///
    /// Binding with a port number of 0 will request that the operating system assigns an available
    /// port to this listener. The assigned port can be queried via the
    /// [`local_addr()`][`TcpListener::local_addr()`] method.
    ///
    /// If `addr` yields multiple addresses, binding will be attempted with each of the addresses
    /// until one succeeds and returns the listener. If none of the addresses succeed in creating a
    /// listener, the error from the last attempt is returned.
    ///
    /// # Examples
    ///
    /// Create a TCP listener bound to `127.0.0.1:80`:
    ///
    /// ```no_run
    /// use async_net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = TcpListener::bind("127.0.0.1:80").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    ///
    /// Create a TCP listener bound to `127.0.0.1:80`. If that address is unavailable, then try
    /// binding to `127.0.0.1:443`:
    ///
    /// ```no_run
    /// use async_net::{SocketAddr, TcpListener};
    ///
    /// # futures_lite::future::block_on(async {
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 80)),
    ///     SocketAddr::from(([127, 0, 0, 1], 443)),
    /// ];
    /// let listener = TcpListener::bind(&addrs[..]).await.unwrap();
    /// # std::io::Result::Ok(()) });
    pub async fn bind<A: AsyncToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let mut last_err = None;

        for addr in addr.to_socket_addrs().await? {
            match Async::<std::net::TcpListener>::bind(addr) {
                Ok(listener) => return Ok(TcpListener(Arc::new(listener))),
                Err(err) => last_err = Some(err),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any of the addresses",
            )
        }))
    }

    /// Returns the local address this listener is bound to.
    ///
    /// # Examples
    ///
    /// Bind to port 0 and then see which port was assigned by the operating system:
    ///
    /// ```no_run
    /// use async_net::{SocketAddr, TcpListener};
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = TcpListener::bind("127.0.0.1:0").await?;
    /// println!("Listening on {}", listener.local_addr()?);
    /// # std::io::Result::Ok(()) });
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().local_addr()
    }

    /// Accepts a new incoming connection.
    ///
    /// Returns a TCP stream and the address it is connected to.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = TcpListener::bind("127.0.0.1:8080").await?;
    /// let (stream, addr) = listener.accept().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let (stream, addr) = self.0.accept().await?;
        let stream = TcpStream(Arc::new(stream));
        Ok((stream, addr))
    }

    /// Returns a stream of incoming connections.
    ///
    /// Iterating over this stream is equivalent to calling [`accept()`][`TcpListener::accept()`]
    /// in a loop. The stream of connections is infinite, i.e awaiting the next connection will
    /// never result in [`None`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::TcpListener;
    /// use futures_lite::*;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = TcpListener::bind("127.0.0.1:0").await?;
    /// let mut incoming = listener.incoming();
    ///
    /// while let Some(stream) = incoming.next().await {
    ///     let mut stream = stream?;
    ///     stream.write_all(b"hello").await?;
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming(self)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// This option configures the time-to-live field that is used in every packet sent from this
    /// socket.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = TcpListener::bind("127.0.0.1:80").await?;
    /// listener.set_ttl(100)?;
    /// assert_eq!(listener.ttl()?, 100);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn ttl(&self) -> io::Result<u32> {
        self.0.get_ref().ttl()
    }

    /// Sets the value of the `IP_TTL` option for this socket.
    ///
    /// This option configures the time-to-live field that is used in every packet sent from this
    /// socket.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::TcpListener;
    ///
    /// # futures_lite::future::block_on(async {
    /// let listener = TcpListener::bind("127.0.0.1:80").await?;
    /// listener.set_ttl(100)?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.0.get_ref().set_ttl(ttl)
    }
}

#[cfg(unix)]
impl AsRawFd for TcpListener {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[cfg(windows)]
impl AsRawSocket for TcpListener {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

/// A stream of incoming TCP connections.
///
/// This stream is infinite, i.e awaiting the next connection will never result in [`None`]. It is
/// created by the [`TcpListener::incoming()`] method.
#[derive(Debug)]
pub struct Incoming<'a>(&'a TcpListener);

impl<'a> Stream for Incoming<'a> {
    type Item = io::Result<TcpStream>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let future = self.0.accept();
        pin!(future);

        let (socket, _) = ready!(future.poll(cx))?;
        Poll::Ready(Some(Ok(socket)))
    }
}

/// A TCP connection.
///
/// A [`TcpStream`] can be created by [`connect`][`TcpStream::connect()`]ing to an endpoint or by
/// [`accept`][`TcpListener::accept()`]ing an incoming connection.
///
/// [`TcpStream`] is a bidirectional stream that implements traits [`AsyncRead`] and
/// [`AsyncWrite`].
///
/// Cloning a [`TcpStream`] creates another handle to the same socket. The socket will be closed
/// when all handles to it are dropped. The reading and writing portions of the connection can also
/// be shut down individually with the [`shutdown()`][`TcpStream::shutdown()`] method.
///
/// The Transmission Control Protocol is specified in [IETF RFC 793].
///
/// [IETF RFC 793]: https://tools.ietf.org/html/rfc793
///
/// # Examples
///
/// ```no_run
/// use async_net::TcpStream;
/// use futures_lite::*;
///
/// # futures_lite::future::block_on(async {
/// let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
/// stream.write_all(b"hello").await?;
///
/// let mut buf = vec![0u8; 1024];
/// let n = stream.read(&mut buf).await?;
/// # std::io::Result::Ok(()) });
/// ```
#[derive(Clone, Debug)]
pub struct TcpStream(Arc<Async<std::net::TcpStream>>);

impl TcpStream {
    /// Creates a TCP connection to the specified address.
    ///
    /// This method will create a new TCP socket and attempt to connect it to the provided `addr`,
    ///
    /// If `addr` yields multiple addresses, connecting will be attempted with each of the
    /// addresses until connecting to one succeeds. If none of the addresses result in a successful
    /// connection, the error from the last connect attempt is returned.
    ///
    /// # Examples
    ///
    /// Connect to `example.com:80`:
    ///
    /// ```
    /// use async_net::TcpStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = TcpStream::connect("example.com:80").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    ///
    /// Connect to `127.0.0.1:8080`. If that fails, then try connecting to `127.0.0.1:8081`:
    ///
    /// ```no_run
    /// use async_net::{SocketAddr, TcpStream};
    ///
    /// # futures_lite::future::block_on(async {
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 8080)),
    ///     SocketAddr::from(([127, 0, 0, 1], 8081)),
    /// ];
    /// let stream = TcpStream::connect(&addrs[..]).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn connect<A: AsyncToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
        let mut last_err = None;

        for addr in addr.to_socket_addrs().await? {
            match Async::<std::net::TcpStream>::connect(addr).await {
                Ok(stream) => return Ok(TcpStream(Arc::new(stream))),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not connect to any of the addresses",
            )
        }))
    }

    /// Returns the local address this stream is bound to.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_net::TcpStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = TcpStream::connect("example.com:80").await?;
    /// println!("Local address is {}", stream.local_addr()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().local_addr()
    }

    /// Returns the remote address this stream is connected to.
    ///
    /// # Examples
    ///
    /// ```
    /// use async_net::TcpStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = TcpStream::connect("example.com:80").await?;
    /// println!("Connected to {}", stream.peer_addr()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().peer_addr()
    }

    /// Shuts down the read half, write half, or both halves of this connection.
    ///
    /// This method will cause all pending and future I/O in the given directions to return
    /// immediately with an appropriate value (see the documentation of [`Shutdown`]).
    ///
    /// [`Shutdown`]: https://doc.rust-lang.org/std/net/enum.Shutdown.html
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::{Shutdown, TcpStream};
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// stream.shutdown(Shutdown::Both)?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn shutdown(&self, how: std::net::Shutdown) -> std::io::Result<()> {
        self.0.get_ref().shutdown(how)
    }

    /// Receives data without removing it from the queue.
    ///
    /// On success, returns the number of bytes peeked.
    ///
    /// Successive calls return the same data. This is accomplished by passing `MSG_PEEK` as a flag
    /// to the underlying `recv` system call.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::TcpStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    ///
    /// let mut buf = vec![0; 1024];
    /// let n = stream.peek(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf).await
    }

    /// Gets the value of the `TCP_NODELAY` option for this socket.
    ///
    /// If set to `true`, this option disables the [Nagle algorithm][nagle-wiki]. This means that
    /// written data is always sent as soon as possible, even if there is only a small amount of
    /// it.
    ///
    /// When set to `false`, written data is buffered until there is a certain amount to send out,
    /// thereby avoiding the frequent sending of small packets.
    ///
    /// [nagle-wiki]: https://en.wikipedia.org/wiki/Nagle%27s_algorithm
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::TcpStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// println!("TCP_NODELAY is set to {}", stream.nodelay()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn nodelay(&self) -> io::Result<bool> {
        self.0.get_ref().nodelay()
    }

    /// Sets the value of the `TCP_NODELAY` option for this socket.
    ///
    /// If set to `true`, this option disables the [Nagle algorithm][nagle-wiki]. This means that
    /// written data is always sent as soon as possible, even if there is only a small amount of
    /// it.
    ///
    /// When set to `false`, written data is buffered until there is a certain amount to send out,
    /// thereby avoiding the frequent sending of small packets.
    ///
    /// [nagle-wiki]: https://en.wikipedia.org/wiki/Nagle%27s_algorithm
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::TcpStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// stream.set_nodelay(false)?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.0.get_ref().set_nodelay(nodelay)
    }

    /// Gets the value of the `IP_TTL` option for this socket.
    ///
    /// This option configures the time-to-live field that is used in every packet sent from this
    /// socket.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::TcpStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// println!("IP_TTL is set to {}", stream.ttl()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn ttl(&self) -> io::Result<u32> {
        self.0.get_ref().ttl()
    }

    /// Sets the value of the `IP_TTL` option for this socket.
    ///
    /// This option configures the time-to-live field that is used in every packet sent from this
    /// socket.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::TcpStream;
    ///
    /// # futures_lite::future::block_on(async {
    /// let stream = TcpStream::connect("127.0.0.1:8080").await?;
    /// stream.set_ttl(100)?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.0.get_ref().set_ttl(ttl)
    }
}

#[cfg(unix)]
impl AsRawFd for TcpStream {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[cfg(windows)]
impl AsRawSocket for TcpStream {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

impl AsyncRead for TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_read(cx, buf)
    }

    fn poll_read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_read_vectored(cx, bufs)
    }
}

impl AsyncRead for &TcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for TcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_write(cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_write_vectored(cx, bufs)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self).poll_close(cx)
    }
}

impl AsyncWrite for &TcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self.0).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self.0).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self.0).poll_close(cx)
    }
}
