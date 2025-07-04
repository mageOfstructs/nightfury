use std::fmt::Display;
use std::io::{self, BufRead, ErrorKind, Write};

pub trait WriteNullDelimitedExt {
    fn write_with_null(&mut self, data: &[u8]) -> std::io::Result<()>;
    fn write_with_null_flush(&mut self, data: &[u8]) -> std::io::Result<()>;
}

pub trait ReadUntilNullExt {
    fn read_until_null(&mut self, buf: &mut String) -> std::io::Result<()>;
}

impl<S: BufRead> ReadUntilNullExt for S {
    fn read_until_null(&mut self, buf: &mut String) -> std::io::Result<()> {
        let mut bytes_buf = Vec::with_capacity(buf.len());
        self.read_until(0, &mut bytes_buf)?;
        match str::from_utf8(&bytes_buf) {
            Ok(str) => buf.push_str(str),
            Err(_) => return Err(std::io::Error::new(ErrorKind::InvalidInput, "not utf8!")),
        }
        Ok(())
    }
}

impl<S: Write> WriteNullDelimitedExt for S {
    fn write_with_null(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.write(data)?;
        self.write(&[0])?;
        Ok(())
    }
    fn write_with_null_flush(&mut self, data: &[u8]) -> std::io::Result<()> {
        self.write_with_null(data)?;
        self.flush()?;
        Ok(())
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum Request {
    GetCapabilities,
    InstallLanguage(String, Option<String>),
    Revert,
    Reset,
    Initialize(String),
    SetCursor(u16),
}

#[derive(Debug)]
pub enum Error {
    Empty,
    InvalidControlCode,
    InvalidEncoding,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

pub trait ReadRequest {
    fn read_request(&mut self) -> io::Result<Request>;
}

impl<R: BufRead> ReadRequest for R {
    fn read_request(&mut self) -> io::Result<Request> {
        let mut buf = vec![0, 0];
        self.read(&mut buf[..1])?;
        if buf[0] > 0x04 {
            self.read_until(0, &mut buf)?;
        }

        Request::try_from(buf).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }
}

impl TryFrom<Vec<u8>> for Request {
    type Error = Error;
    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(Error::Empty);
        }
        match value[0] {
            0x01 => Ok(Request::GetCapabilities),
            0x02 => todo!(),
            0x03 => Ok(Request::Revert),
            0x04 => Ok(Request::Reset),
            0x05 => {
                if value.len() < 3 {
                    return Err(Error::Empty);
                }
                match str::from_utf8(&value[1..value.len() - 1]) {
                    Ok(str) => Ok(Request::Initialize(str.to_string())),
                    Err(_) => Err(Error::InvalidEncoding),
                }
            }
            0x06 => {
                if value.len() < 3 {
                    return Err(Error::Empty);
                }
                let cursor_handle = (value[1] as u16) << 8 | value[2] as u16;
                Ok(Request::SetCursor(cursor_handle))
            }
            _ => Err(Error::InvalidControlCode),
        }
    }
}

impl Request {
    fn discriminant(&self) -> u8 {
        unsafe { *(self as *const Self as *const u8) }
    }
    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let protocol_id = self.discriminant();
        writer.write(&[protocol_id])?;
        match self {
            Self::Initialize(lang) => {
                writer.write_with_null(&lang.as_bytes())?;
            }
            Self::SetCursor(handle) => {
                writer.write_with_null(&[
                    (handle >> 8).try_into().unwrap(),
                    (handle & 8).try_into().unwrap(),
                ])?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum Response<'a> {
    Ok,
    RError(&'a str),
    Capabilities(Vec<String>),
    CursorHandle(u16),
}

impl<'a> TryFrom<&'a [u8]> for Response<'a> {
    type Error = Error;
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(Error::Empty);
        }
        if value.len() == 2 {
            return Ok(Response::CursorHandle(
                (value[0] as u16) << 8 | value[1] as u16,
            ));
        }
        match value[0] {
            0x0 => Ok(Response::Ok),
            0x1 => str::from_utf8(&value[1..value.len() - 1])
                .map(|str| Response::RError(str))
                .map_err(|_| Error::InvalidEncoding),
            _ => str::from_utf8(&value[1..value.len() - 1])
                .map(|str| {
                    Response::Capabilities(str.split(';').map(|str| String::from(str)).collect())
                })
                .map_err(|_| Error::InvalidEncoding),
        }
    }
}
