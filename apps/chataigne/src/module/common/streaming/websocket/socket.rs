use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
};
use tokio_tungstenite::tungstenite::WebSocket;

pub(crate) type WireSocket = WebSocket<BudgetSocket>;
const IO_BYTES: usize = 64 * 1024;
const IO_CALLS: usize = 4;

/// Bound even protocol-internal fragment/control loops and partial flushes.
pub(crate) struct BudgetSocket {
    pub stream: TcpStream,
    read_bytes: usize,
    write_bytes: usize,
    reads: usize,
    writes: usize,
}
impl BudgetSocket {
    pub fn new(stream: TcpStream) -> Self {
        let mut socket = Self {
            stream,
            read_bytes: 0,
            write_bytes: 0,
            reads: 0,
            writes: 0,
        };
        socket.reset();
        socket
    }
    pub fn reset(&mut self) {
        self.read_bytes = IO_BYTES;
        self.write_bytes = IO_BYTES;
        self.reads = 0;
        self.writes = 0;
    }
    pub fn remote(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }
}
impl Read for BudgetSocket {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.read_bytes == 0 || self.reads >= IO_CALLS {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.reads += 1;
        let size = buffer.len().min(self.read_bytes);
        let read = self.stream.read(&mut buffer[..size])?;
        self.read_bytes -= read;
        Ok(read)
    }
}
impl Write for BudgetSocket {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.write_bytes == 0 || self.writes >= IO_CALLS {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.writes += 1;
        let size = buffer.len().min(self.write_bytes);
        let written = self.stream.write(&buffer[..size])?;
        self.write_bytes -= written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}
