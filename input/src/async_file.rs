use libc::c_int;
use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{Poll, PollOpt, Ready, Token};
use std::convert::AsRef;
use std::convert::TryInto;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::io::{Error, Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{self, Context};
use tokio::io::{AsyncRead, AsyncWrite, PollEvented};

pub struct AsyncFile {
    file: PollEvented<Inner>,
}

impl AsyncFile {
    pub async fn open(path: impl AsRef<Path>, mode: OpenMode) -> Result<Self, Error> {
        let path = path.as_ref().to_owned();

        tokio::task::spawn_blocking(move || Self::open_sync(path, mode)).await?
    }

    fn open_sync(path: PathBuf, mode: OpenMode) -> Result<Self, Error> {
        let path =
            CString::new(path.into_os_string().into_vec()).map_err(|_| ErrorKind::InvalidInput)?;
        let flags = match mode {
            OpenMode::Read => libc::O_RDONLY,
            OpenMode::Write => libc::O_WRONLY,
            OpenMode::ReadWrite => libc::O_RDWR,
        };

        let fd = unsafe { libc::open(path.as_ptr(), flags | libc::O_NONBLOCK) };
        if fd == -1 {
            return Err(Error::last_os_error());
        }

        Ok(AsyncFile {
            file: PollEvented::new(Inner { fd })?,
        })
    }
}

impl AsRawFd for AsyncFile {
    fn as_raw_fd(&self) -> RawFd {
        self.file.get_ref().fd
    }
}

impl AsyncRead for AsyncFile {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context,
        buffer: &mut [u8],
    ) -> task::Poll<Result<usize, Error>> {
        Pin::new(&mut self.file).poll_read(context, buffer)
    }
}

impl AsyncWrite for AsyncFile {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context,
        data: &[u8],
    ) -> task::Poll<Result<usize, Error>> {
        Pin::new(&mut self.file).poll_write(context, data)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context,
    ) -> task::Poll<Result<(), Error>> {
        Pin::new(&mut self.file).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context,
    ) -> task::Poll<Result<(), Error>> {
        Pin::new(&mut self.file).poll_shutdown(context)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum OpenMode {
    Read,
    Write,
    ReadWrite,
}

struct Inner {
    fd: c_int,
}

impl Read for Inner {
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let size = buffer
            .len()
            .try_into()
            .map_err(|_| ErrorKind::InvalidInput)?;
        let read = unsafe { libc::read(self.fd, buffer.as_mut_ptr() as *mut _, size) };
        if read == -1 {
            return Err(Error::last_os_error());
        }

        Ok(read.try_into().unwrap())
    }
}

impl Write for Inner {
    fn write(&mut self, data: &[u8]) -> Result<usize, Error> {
        let size = data.len().try_into().map_err(|_| ErrorKind::InvalidInput)?;
        let written = unsafe { libc::write(self.fd, data.as_ptr() as *mut _, size) };
        if written == -1 {
            return Err(Error::last_os_error());
        }

        Ok(written.try_into().unwrap())
    }

    fn flush(&mut self) -> Result<(), Error> {
        if unsafe { libc::fsync(self.fd) == -1 } {
            return Err(Error::last_os_error());
        }

        Ok(())
    }
}

impl Evented for Inner {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), Error> {
        EventedFd(&self.fd).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> Result<(), Error> {
        EventedFd(&self.fd).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &Poll) -> Result<(), Error> {
        EventedFd(&self.fd).deregister(poll)
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}
