use std::io;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{BufMut, BytesMut};
use futures::{sink::Sink, stream::Stream};
use tokio::{io::ReadBuf, net::UdpSocket};

#[derive(Debug)]
pub struct IoConnection {
    socket: UdpSocket,
    rd: BytesMut,
    wr: BytesMut,
    flushed: bool,
    is_readable: bool,
}

const INITIAL_RD_CAPACITY: usize = 64 * 1024;
const INITIAL_WR_CAPACITY: usize = 8 * 1024;

impl Stream for IoConnection {
    type Item = Result<BytesMut, io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let pin = self.get_mut();

        pin.rd.reserve(INITIAL_RD_CAPACITY);

        loop {
            // Are there still bytes left in the read buffer to decode?
            if pin.is_readable {
                let frame = pin.rd.split_to(pin.rd.len());

                // if this line has been reached then decode has returned `None`.
                pin.is_readable = false;
                pin.rd.clear();

                return Poll::Ready(Some(Ok(frame)));
            }

            // We're out of data. Try and fetch more data to decode
            unsafe {
                // Convert `&mut [MaybeUnit<u8>]` to `&mut [u8]` because we will be
                // writing to it via `poll_recv_from` and therefore initializing the memory.
                let buf = &mut *(pin.rd.chunk_mut() as *mut _ as *mut [MaybeUninit<u8>]);
                let mut read = ReadBuf::uninit(buf);
                let ptr = read.filled().as_ptr();
                ready!(Pin::new(&mut pin.socket).poll_recv(cx, &mut read))?;

                assert_eq!(ptr, read.filled().as_ptr());
                pin.rd.advance_mut(read.filled().len());
            };

            pin.is_readable = true;
        }
    }
}

impl Sink<BytesMut> for IoConnection {
    type Error = io::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if !self.flushed {
            match self.poll_flush(cx)? {
                Poll::Ready(()) => {}
                Poll::Pending => return Poll::Pending,
            }
        }

        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: BytesMut) -> Result<(), Self::Error> {
        let frame = item;

        let pin = self.get_mut();

        pin.wr.reserve(frame.len());
        pin.wr.put(frame);
        pin.flushed = false;

        Ok(())
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        if self.flushed {
            return Poll::Ready(Ok(()));
        }

        let Self {
            ref mut socket,
            ref mut wr,
            ..
        } = *self;

        let n = ready!(socket.poll_send(cx, &wr))?;

        let wrote_all = n == self.wr.len();
        self.wr.clear();
        self.flushed = true;

        let res = if wrote_all {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                "failed to write entire datagram to socket",
            )
            .into())
        };

        Poll::Ready(res)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        ready!(self.poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }
}

impl IoConnection {
    pub fn new(socket: UdpSocket) -> IoConnection {
        Self {
            socket,
            rd: BytesMut::with_capacity(INITIAL_RD_CAPACITY),
            wr: BytesMut::with_capacity(INITIAL_WR_CAPACITY),
            flushed: true,
            is_readable: false,
        }
    }
}
