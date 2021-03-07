use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use crate::io_connection::IoConnection;

use bytes::{Buf, BufMut, BytesMut};
use concurrent_queue::ConcurrentQueue;
use futures::{Sink, Stream};
use parking_lot::Mutex;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::UdpSocket;
use tokio::time::{sleep, Instant, Sleep};

use super::other;

const MAX_DATAGRAM_SIZE: usize = 1350;

impl Future for Connection {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        self.get_mut().poll(cx)
    }
}

pub struct Connection {
    conn: Pin<Box<quiche::Connection>>,
    io: IoConnection,
    sleep: Option<Pin<Box<Sleep>>>,
    send_buf: BytesMut,
    recv_buf: BytesMut,
    inner: Arc<Inner>,
}

struct QData {
    id: u64,
    data: Option<Vec<u8>>,
    fin: bool,
}

struct Action {
    waker: Option<Waker>,
}

struct QStreamInner {
    waker: Option<Waker>,
    data: BytesMut,
    fin: bool,
}

pub struct QStream {
    id: u64,
    inner: Arc<Mutex<QStreamInner>>,
    streams: Streams,
}

impl Drop for QStream {
    fn drop(&mut self) {
        self.streams.0.stores.lock().remove(&self.id);
    }
}

impl AsyncRead for QStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let mut inner = self.inner.lock();
        if !inner.data.is_empty() {
            buf.put_slice(&inner.data.split());
            return Poll::Ready(Ok(()));
        }
        if inner.fin {
            return Poll::Ready(Ok(()));
        }
        inner.waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

impl AsyncWrite for QStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let _ = self.streams.0.queue.push(QData {
            data: Some(buf.to_vec()),
            id: self.id,
            fin: false,
        });

        self.streams.0.wake();

        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let _ = self.streams.0.queue.push(QData {
            data: None,
            id: self.id,
            fin: true,
        });

        self.streams.0.wake();

        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

pub struct Inner {
    stores: Arc<Mutex<HashMap<u64, Arc<Mutex<QStreamInner>>>>>,
    id_generator: AtomicU64,
    queue: Arc<ConcurrentQueue<QData>>,
    action: Arc<Mutex<Action>>,
}

impl Inner {
    fn wake(&self) {
        let mut action = self.action.lock();
        if let Some(waker) = action.waker.take() {
            waker.wake();
        }
    }
}

pub struct Streams(Arc<Inner>);

impl Clone for Streams {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Streams {
    pub fn new(&self) -> QStream {
        let id = self
            .0
            .id_generator
            .fetch_add(10, Ordering::SeqCst)
            .wrapping_add(1);

        let inner = Arc::new(Mutex::new(QStreamInner {
            waker: None,
            data: BytesMut::new(),
            fin: false,
        }));

        self.0.stores.lock().insert(id, inner.clone());
        QStream {
            inner,
            id,
            streams: self.clone(),
        }
    }
}

impl Connection {
    pub fn new(io: UdpSocket, conn: Pin<Box<quiche::Connection>>) -> (Connection, Streams) {
        let action = Action { waker: None };

        let queue = Arc::new(ConcurrentQueue::unbounded());
        let action = Arc::new(Mutex::new(action));

        let inner = Arc::new(Inner {
            stores: Arc::new(Mutex::new(HashMap::new())),
            id_generator: AtomicU64::new(0),
            queue,
            action,
        });

        let conn = Connection {
            conn,
            io: IoConnection::new(io),
            sleep: None,
            send_buf: BytesMut::with_capacity(MAX_DATAGRAM_SIZE),
            recv_buf: BytesMut::with_capacity(65535),
            inner: inner.clone(),
        };

        (conn, Streams(inner))
    }

    fn timeout(&mut self) {
        match self.conn.timeout() {
            Some(timeout) => {
                if self.sleep.is_none() {
                    self.sleep = Some(Box::pin(sleep(timeout)));
                } else {
                    self.sleep
                        .as_mut()
                        .unwrap()
                        .as_mut()
                        .reset(Instant::now() + timeout);
                }
            }
            None => self.sleep = None,
        }
    }

    fn poll(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        match self.poll_recv(cx) {
            Poll::Ready(Err(e)) => {
                log::error!("poll_recv error: {:?}", e);
            }
            Poll::Ready(Ok(_)) => panic!("unexpeted branch"),
            Poll::Pending => {
                self.timeout();

                if let Err(e) = ready!(self.poll_send(cx)) {
                    log::error!("poll_send error: {:?}", e);
                }

                if let Poll::Ready(Err(_)) = self.poll_timeout(cx) {
                    self.conn.on_timeout();
                }
            }
        }

        Poll::Pending
    }

    fn poll_timeout(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        if let Some(mut sleep) = self.sleep.as_mut() {
            match Pin::new(&mut sleep).poll(cx) {
                Poll::Pending => {}
                Poll::Ready(_) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "connection timeout",
                    )));
                }
            }
        }

        Poll::Ready(Ok(()))
    }

    fn poll_send(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        let mut sink = Pin::new(&mut self.io);
        let mut action = self.inner.action.lock();
        let mut try_send = false;

        while let Ok(data) = self.inner.queue.pop() {
            let _ = self
                .conn
                .stream_send(data.id, &data.data.unwrap_or_default(), data.fin);
        }

        loop {
            let size = if self.send_buf.is_empty() {
                self.conn.send(&mut self.send_buf)
            } else {
                Ok(self.send_buf.len())
            };

            match size {
                Ok(v) => match sink.as_mut().poll_ready(cx)? {
                    Poll::Ready(()) => {
                        try_send = true;
                        sink.as_mut().start_send(self.send_buf.split_to(v))?;
                        self.send_buf.clear();
                    }
                    Poll::Pending => {
                        if try_send {
                            ready!(sink.poll_flush(cx))?;
                        }
                        return Poll::Pending;
                    }
                },
                Err(quiche::Error::Done) => {
                    break;
                }
                Err(e) => {
                    log::error!("quiche conn send error: {:?}", e);
                    return Poll::Ready(Err(other(&e.to_string())));
                }
            }
        }

        if try_send {
            ready!(sink.poll_flush(cx))?;
        }
        action.waker = Some(cx.waker().clone());
        Poll::Ready(Ok(()))
    }

    fn recv_streams(&mut self) {
        if !self.conn.is_readable() {
            return;
        }

        let mut m = HashMap::new();
        for id in self.conn.readable() {
            let mut buf = BytesMut::new();
            let mut finish = false;
            while let Ok((read, fin)) = self.conn.stream_recv(id, &mut self.recv_buf) {
                log::debug!("stream {} has {} bytes (fin? {})", id, read, fin);
                buf.put(self.recv_buf.split_to(read));
                self.recv_buf.reserve(65535);
                finish = fin;
            }
            m.insert(id, (buf, finish));
        }

        let mut wakers = vec![];
        let mut stores = self.inner.stores.lock();
        for (id, (buf, fin)) in m.into_iter() {
            if let Some(qs) = stores.get_mut(&id) {
                let mut s = qs.lock();
                s.data.put(buf);
                s.fin = fin;
                if let Some(waker) = s.waker.take() {
                    wakers.push(waker);
                }
            }
        }

        for waker in wakers {
            waker.wake();
        }
    }

    fn poll_recv(&mut self, cx: &mut Context) -> Poll<io::Result<()>> {
        loop {
            match ready!(Pin::new(&mut self.io).poll_next(cx)) {
                Some(buf) => match buf {
                    Ok(mut buf) => {
                        while !buf.is_empty() {
                            let length = buf.len();
                            let read = match self.conn.recv(&mut buf[..length]) {
                                Ok(v) => v,
                                Err(e) => {
                                    log::error!("quiche recv err: {:?}", e);
                                    break;
                                }
                            };

                            buf.advance(read);
                        }

                        self.recv_streams();
                    }
                    Err(e) => {
                        log::error!("poll_recv err: {}", e.to_string());
                        return Poll::Ready(Err(e));
                    }
                },
                None => {
                    log::error!("poll_recv recv eof");
                    let e = io::Error::new(io::ErrorKind::UnexpectedEof, "eof");
                    return Poll::Ready(Err(e));
                }
            }
        }
    }
}
