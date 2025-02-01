use std::io::{Read, Write};
use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::Path;

pub struct RWUnixDatagram {
    unixdatagram: UnixDatagram
}

impl Read for RWUnixDatagram {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.unixdatagram.recv(buf)
    }
}

impl Write for RWUnixDatagram {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.unixdatagram.send(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl RWUnixDatagram {
    pub fn unbound() -> io::Result<RWUnixDatagram> {
        Ok(RWUnixDatagram {
            unixdatagram: UnixDatagram::unbound()?
        })
    }
    
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<RWUnixDatagram> {
        match UnixDatagram::bind(path) {
            Ok(ud) => {
                Ok(RWUnixDatagram { unixdatagram: ud })
            }
            Err(e) => { Err(e) }
        }
    }

    pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self.unixdatagram.connect(path)
    }
}