//! Unix domain sockets.
//!
//! This module is an async version of [`std::os::unix::net`].

use std::future::Future;
use std::io;
use std::net::Shutdown;
#[cfg(unix)]
use std::os::unix::io::{AsRawFd, RawFd};
use std::os::unix::net::SocketAddr;
#[cfg(windows)]
use std::os::windows::io::{AsRawSocket, RawSocket};
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_io::Async;
use futures_util::io::{AsyncRead, AsyncWrite};
use futures_util::stream::Stream;

/// A Unix server, listening for connections.
///
/// After creating a [`UnixListener`] by [`bind`][`UnixListener::bind()`]ing it to an address, it
/// listens for incoming connections. These can be accepted by calling
/// [`accept()`][`UnixListener::accept()`] or by awaiting items from the async stream of
/// [`incoming`][`UnixListener::incoming()`] connections.
///
/// Cloning a [`UnixListener`] creates another handle to the same socket. The socket will be closed
/// when all handles to it are dropped.
///
/// # Examples
///
/// ```no_run
/// use async_net::unix::UnixListener;
/// use futures::prelude::*;
///
/// # blocking::block_on(async {
/// let listener = UnixListener::bind("/tmp/socket")?;
/// let mut incoming = listener.incoming();
///
/// while let Some(stream) = incoming.next().await {
///     let mut stream = stream?;
///     stream.write_all(b"hello").await?;
/// }
/// # std::io::Result::Ok(()) });
/// ```
#[derive(Clone, Debug)]
pub struct UnixListener(Arc<Async<std::os::unix::net::UnixListener>>);

impl UnixListener {
    /// Creates a new [`UnixListener`] bound to the given path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixListener;
    /// use futures::prelude::*;
    ///
    /// # blocking::block_on(async {
    /// let listener = UnixListener::bind("/tmp/socket")?;
    /// let mut incoming = listener.incoming();
    ///
    /// while let Some(stream) = incoming.next().await {
    ///     let mut stream = stream?;
    ///     stream.write_all(b"hello").await?;
    /// }
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        let path = path.as_ref().to_owned();
        let listener = Async::<std::os::unix::net::UnixListener>::bind(path)?;
        Ok(UnixListener(Arc::new(listener)))
    }

    /// Accepts a new incoming connection.
    ///
    /// Returns a TCP stream and the address it is connected to.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixListener;
    ///
    /// # blocking::block_on(async {
    /// let listener = UnixListener::bind("/tmp/socket")?;
    /// let (stream, addr) = listener.accept().await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let (stream, addr) = self.0.accept().await?;
        Ok((UnixStream(Arc::new(stream)), addr))
    }

    /// Returns a stream of incoming connections.
    ///
    /// Iterating over this stream is equivalent to calling [`accept()`][`UnixListener::accept()`]
    /// in a loop. The stream of connections is infinite, i.e awaiting the next connection will
    /// never result in [`None`].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixListener;
    /// use futures::prelude::*;
    ///
    /// # blocking::block_on(async {
    /// let listener = UnixListener::bind("/tmp/socket")?;
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

    /// Returns the local address this listener is bound to.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixListener;
    ///
    /// # blocking::block_on(async {
    /// let listener = UnixListener::bind("/tmp/socket")?;
    /// println!("Local address is {:?}", listener.local_addr()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().local_addr()
    }
}

#[cfg(unix)]
impl AsRawFd for UnixListener {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[cfg(windows)]
impl AsRawSocket for UnixListener {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

/// A stream of incoming Unix connections.
///
/// This stream is infinite, i.e awaiting the next connection will never result in [`None`]. It is
/// created by the [`UnixListener::incoming()`] method.
#[derive(Debug)]
pub struct Incoming<'a>(&'a UnixListener);

impl Stream for Incoming<'_> {
    type Item = io::Result<UnixStream>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let future = self.0.accept();
        futures_util::pin_mut!(future);

        let (socket, _) = futures_util::ready!(future.poll(cx))?;
        Poll::Ready(Some(Ok(socket)))
    }
}

/// A Unix connection.
///
/// A [`UnixStream`] can be created by [`connect`][`UnixStream::connect()`]ing to an endpoint or by
/// [`accept`][`UnixListener::accept()`]ing an incoming connection.
///
/// [`UnixStream`] is a bidirectional stream that implements traits [`AsyncRead`] and
/// [`AsyncWrite`].
///
/// Cloning a [`UnixStream`] creates another handle to the same socket. The socket will be closed
/// when all handles to it are dropped. The reading and writing portions of the connection can also
/// be shut down individually with the [`shutdown()`][`UnixStream::shutdown()`] method.
///
/// # Examples
///
/// ```no_run
/// use async_net::unix::UnixStream;
/// use futures::prelude::*;
///
/// # blocking::block_on(async {
/// let mut stream = UnixStream::connect("/tmp/socket").await?;
/// stream.write_all(b"hello").await?;
///
/// let mut buf = vec![0u8; 1024];
/// let n = stream.read(&mut buf).await?;
/// # std::io::Result::Ok(()) });
/// ```
#[derive(Clone, Debug)]
pub struct UnixStream(Arc<Async<std::os::unix::net::UnixStream>>);

impl UnixStream {
    /// Creates a Unix connection to given path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixStream;
    ///
    /// # blocking::block_on(async {
    /// let stream = UnixStream::connect("/tmp/socket").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn connect<P: AsRef<Path>>(path: P) -> io::Result<UnixStream> {
        let path = path.as_ref().to_owned();
        let stream = Arc::new(Async::<std::os::unix::net::UnixStream>::connect(path).await?);
        Ok(UnixStream(stream))
    }

    /// Creates a pair of connected Unix sockets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixStream;
    ///
    /// # blocking::block_on(async {
    /// let (stream1, stream2) = UnixStream::pair()?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn pair() -> io::Result<(UnixStream, UnixStream)> {
        let (a, b) = Async::<std::os::unix::net::UnixStream>::pair()?;
        let a = UnixStream(Arc::new(a));
        let b = UnixStream(Arc::new(b));
        Ok((a, b))
    }

    /// Returns the local address this socket is connected to.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixStream;
    ///
    /// # blocking::block_on(async {
    /// let stream = UnixStream::connect("/tmp/socket").await?;
    /// println!("Local address is {:?}", stream.local_addr()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().local_addr()
    }

    /// Returns the remote address this socket is connected to.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixStream;
    ///
    /// # blocking::block_on(async {
    /// let stream = UnixStream::connect("/tmp/socket").await?;
    /// println!("Connected to {:?}", stream.peer_addr()?);
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
    /// ```no_run
    /// use async_net::unix::UnixStream;
    /// use std::net::Shutdown;
    ///
    /// # blocking::block_on(async {
    /// let stream = UnixStream::connect("/tmp/socket").await?;
    /// stream.shutdown(Shutdown::Both)?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.get_ref().shutdown(how)
    }
}

#[cfg(unix)]
impl AsRawFd for UnixStream {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[cfg(windows)]
impl AsRawSocket for UnixStream {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}

impl AsyncRead for UnixStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_read(cx, buf)
    }
}

impl AsyncRead for &UnixStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self.0).poll_read(cx, buf)
    }
}

impl AsyncWrite for UnixStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut &*self).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self).poll_flush(cx)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut &*self).poll_close(cx)
    }
}

impl AsyncWrite for &UnixStream {
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

/// A Unix datagram socket.
///
/// After creating a [`UnixDatagram`] by [`bind`][`UnixDatagram::bind()`]ing it to a path, data can
/// be [sent to] and [received from] any other socket address.
///
/// Cloning a [`UnixDatagram`] creates another handle to the same socket. The socket will be closed
/// when all handles to it are dropped. The reading and writing portions of the socket can also be
/// shut down individually with the [`shutdown()`][`UnixStream::shutdown()`] method.
///
/// [received from]: UnixDatagram::recv_from()
/// [sent to]: UnixDatagram::send_to()
///
/// # Examples
///
/// ```no_run
/// use async_net::unix::UnixDatagram;
///
/// # blocking::block_on(async {
/// let socket = UnixDatagram::bind("/tmp/socket1")?;
/// socket.send_to(b"hello", "/tmp/socket2").await?;
///
/// let mut buf = vec![0u8; 1024];
/// let (n, addr) = socket.recv_from(&mut buf).await?;
/// # std::io::Result::Ok(()) });
/// ```
#[derive(Clone, Debug)]
pub struct UnixDatagram(Arc<Async<std::os::unix::net::UnixDatagram>>);

impl UnixDatagram {
    /// Creates a new [`UnixDatagram`] bound to the given address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::bind("/tmp/socket")?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
        let path = path.as_ref().to_owned();
        let socket = Async::<std::os::unix::net::UnixDatagram>::bind(path)?;
        Ok(UnixDatagram(Arc::new(socket)))
    }

    /// Creates a Unix datagram socket not bound to any address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::unbound()?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn unbound() -> io::Result<UnixDatagram> {
        let socket = std::os::unix::net::UnixDatagram::unbound()?;
        Ok(UnixDatagram(Arc::new(Async::new(socket)?)))
    }

    /// Creates a pair of connected Unix datagram sockets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let (socket1, socket2) = UnixDatagram::pair()?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn pair() -> io::Result<(UnixDatagram, UnixDatagram)> {
        let (a, b) = std::os::unix::net::UnixDatagram::pair()?;
        let a = UnixDatagram(Arc::new(Async::new(a)?));
        let b = UnixDatagram(Arc::new(Async::new(b)?));
        Ok((a, b))
    }

    /// Connects the Unix datagram socket to the given address.
    ///
    /// When connected, methods [`send()`][`UnixDatagram::send()`] and
    /// [`recv()`][`UnixDatagram::recv()`] will use the specified address for sending and receiving
    /// messages. Additionally, a filter will be applied to
    /// [`recv_from()`][`UnixDatagram::recv_from()`] so that it only receives messages from that
    /// same address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::unbound()?;
    /// socket.connect("/tmp/socket")?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let p = path.as_ref();
        self.0.get_ref().connect(p)
    }

    /// Returns the local address this socket is bound to.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::bind("/tmp/socket")?;
    /// println!("Bound to {:?}", socket.local_addr()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().local_addr()
    }

    /// Returns the remote address this socket is connected to.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::unbound()?;
    /// socket.connect("/tmp/socket")?;
    /// println!("Connected to {:?}", socket.peer_addr()?);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.get_ref().peer_addr()
    }

    /// Receives data from an address.
    ///
    /// On success, returns the number of bytes received and the address data came from.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::bind("/tmp/socket")?;
    ///
    /// let mut buf = vec![0; 1024];
    /// let (n, addr) = socket.recv_from(&mut buf).await?;
    /// println!("Received {} bytes from {:?}", n, addr);
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.recv_from(buf).await
    }

    /// Sends data to the given address.
    ///
    /// On success, returns the number of bytes sent.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::unbound()?;
    /// socket.send_to(b"hello", "/tmp/socket").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn send_to<P: AsRef<Path>>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        self.0.send_to(buf, path.as_ref()).await
    }

    /// Receives data from the connected address.
    ///
    /// On success, returns the number of bytes received.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::unbound()?;
    /// socket.connect("/tmp/socket")?;
    ///
    /// let mut buf = vec![0; 1024];
    /// let n = socket.recv(&mut buf).await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf).await
    }

    /// Sends data to the connected address.
    ///
    /// On success, returns the number of bytes sent.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::unbound()?;
    /// socket.connect("/tmp/socket")?;
    /// socket.send(b"hello").await?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf).await
    }

    /// Shuts down the read half, write half, or both halves of this socket.
    ///
    /// This method will cause all pending and future I/O in the given directions to return
    /// immediately with an appropriate value (see the documentation of [`Shutdown`]).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use async_net::unix::UnixDatagram;
    /// use std::net::Shutdown;
    ///
    /// # blocking::block_on(async {
    /// let socket = UnixDatagram::unbound()?;
    /// socket.shutdown(Shutdown::Both)?;
    /// # std::io::Result::Ok(()) });
    /// ```
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.get_ref().shutdown(how)
    }
}

#[cfg(unix)]
impl AsRawFd for UnixDatagram {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[cfg(windows)]
impl AsRawSocket for UnixDatagram {
    fn as_raw_socket(&self) -> RawSocket {
        self.0.as_raw_socket()
    }
}
