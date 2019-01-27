use crate::task::wait_result;
use std::io;
use std::net;

pub trait TcpListenerExt {
    fn hydro_accept(&self) -> io::Result<(net::TcpStream, net::SocketAddr)>;
    fn hydro_incoming<'a>(&self) -> HydroIncoming;
}

impl TcpListenerExt for net::TcpListener {
    /// Call set_nonblocking(true) first.
    fn hydro_accept(&self) -> io::Result<(net::TcpStream, net::SocketAddr)> {
        wait_result(
            || self.accept(),
            |e| if e.kind() == io::ErrorKind::WouldBlock {
                None
            } else {
                Some(e)
            }
        )
    }

    /// Call set_nonblocking(true) first.
    fn hydro_incoming(&self) -> HydroIncoming {
        HydroIncoming { listener: self }
    }
}

pub struct HydroIncoming<'a> { listener: &'a net::TcpListener }

impl<'a> Iterator for HydroIncoming<'a> {
    type Item = io::Result<net::TcpStream>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.listener.hydro_accept().map(|p| p.0))
    }
}
