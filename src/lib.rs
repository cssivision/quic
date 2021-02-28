macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}

pub mod client;
pub mod connection;
mod io_connection;
pub mod server;

use std::io;

fn other(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg)
}
