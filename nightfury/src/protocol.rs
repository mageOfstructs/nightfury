use debug_print::debug_println;
use std::fmt::Display;
use std::io::Result as IORes;
use std::io::{self, BufRead, ErrorKind, Write};

pub trait WriteNullDelimitedExt {
    fn write_with_null(&mut self, data: &[u8]) -> IORes<()>;
    fn write_with_null_flush(&mut self, data: &[u8]) -> IORes<()>;
}

pub trait ReadUntilNullExt {
    fn read_until_null(&mut self, buf: &mut String) -> IORes<()>;
}

impl<S: BufRead> ReadUntilNullExt for S {
    fn read_until_null(&mut self, buf: &mut String) -> IORes<()> {
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
    fn write_with_null(&mut self, data: &[u8]) -> IORes<()> {
        self.write_all(data)?;
        self.write_all(&[0])?;
        Ok(())
    }
    fn write_with_null_flush(&mut self, data: &[u8]) -> IORes<()> {
        self.write_with_null(data)?;
        self.flush()?;
        Ok(())
    }
}

pub trait WriteResponse {
    fn write_response(&mut self, resp: Response) -> IORes<()>;
    fn write_err(&mut self, msg: &str) -> IORes<()> {
        self.write_response(Response::RError(msg))
    }
    fn write_ok(&mut self) -> IORes<()> {
        self.write_response(Response::Ok)
    }
}
impl<S: Write> WriteResponse for S {
    fn write_response(&mut self, resp: Response) -> IORes<()> {
        resp.write(self)
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum Request<'a> {
    GetCapabilities,
    InstallLanguage(&'a str, Option<String>),
    Revert,
    Reset,
    Initialize(&'a str),
    SetCursor(u16),
    Advance(&'a str),
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
    fn read_request<'a>(&mut self, buf: &'a mut Vec<u8>) -> io::Result<Request<'a>>;
    fn read_response<'a>(&mut self, buf: &'a mut Vec<u8>) -> io::Result<Response<'a>>;
}

impl<R: BufRead> ReadRequest for R {
    fn read_request<'a>(&mut self, buf: &'a mut Vec<u8>) -> io::Result<Request<'a>> {
        buf.clear();
        let mut sbuf: [u8; 1] = [0];
        self.read_exact(&mut sbuf)?;
        buf.push(sbuf[0]);
        if buf[0] > 0x04 {
            self.read_until(0, buf)?;
        }

        Request::try_from(buf.as_slice())
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }
    fn read_response<'a>(&mut self, buf: &'a mut Vec<u8>) -> io::Result<Response<'a>> {
        let mut stack_buf: [u8; 1] = [0; 1];
        self.read_exact(&mut stack_buf)?;
        if self.has_data_left()? {
            buf.extend(stack_buf);
            self.read_until(0, buf)?;
        }
        Response::try_from(buf.as_slice())
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
    }
}

impl<'a> TryFrom<&'a [u8]> for Request<'a> {
    type Error = Error;
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.is_empty() || value[0] == 0x0 {
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
                    Ok(str) => Ok(Request::Initialize(str)),
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
            _ => str::from_utf8(&value[..value.len() - 1])
                .to_owned()
                .map(Request::Advance)
                .map_err(|_| Error::InvalidEncoding),
        }
    }
}

impl<'a> Request<'a> {
    fn discriminant(&self) -> u8 {
        (unsafe { *(self as *const Self as *const u8) } + 1u8)
    }
    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let protocol_id = self.discriminant();
        if protocol_id < 0x07 {
            writer.write_all(&[protocol_id])?;
        }
        match self {
            Self::Initialize(str) | Self::Advance(str) => {
                writer.write_with_null(str.as_bytes())?;
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
    RegexFull,
    Capabilities(Vec<&'a str>),
    CursorHandle(u8),
    InvalidChar,
    RegexStart,
    Expanded(&'a str),
}

fn from_utf8(data: &[u8]) -> Result<&str, Error> {
    str::from_utf8(data).map_err(|_| Error::InvalidEncoding)
}
fn from_utf8_trim(data: &[u8]) -> Result<&str, Error> {
    from_utf8(&data[1..data.len() - 1])
}

impl<'a> TryFrom<&'a [u8]> for Response<'a> {
    type Error = Error;
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(Error::Empty);
        }
        match value[0] {
            0x00 => Ok(Response::Ok),
            0x01 => from_utf8_trim(value).map(Response::RError),
            0x02 => Ok(Response::RegexFull),
            0x03 => {
                from_utf8_trim(value).map(|str| Response::Capabilities(str.split(';').collect()))
            }
            0x04 => value
                .get(1)
                .map(|handle| Response::CursorHandle(*handle))
                .ok_or(Error::Empty),
            0x05 => Ok(Response::InvalidChar),
            0x06 => Ok(Response::RegexStart),
            _ => from_utf8_trim(value).map(Response::Expanded),
        }
    }
}

impl<'a> Response<'a> {
    fn discriminant(&self) -> u8 {
        unsafe { *(self as *const Self as *const u8) }
    }
    pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        debug_println!("res: {self:?}");
        let disc = self.discriminant();
        if disc < 0x7 {
            writer.write_all(&[disc])?;
        }
        match self {
            Self::RError(msg) => writer.write_with_null(msg.as_bytes()),
            Self::Capabilities(caps) => writer.write_with_null(
                caps.iter()
                    .fold(String::with_capacity(caps.len() * 2), |mut acc, el| {
                        if !acc.is_empty() {
                            acc.push(';');
                        }
                        acc.push_str(el);
                        acc
                    })
                    .as_bytes(),
            ),
            Self::CursorHandle(handle) => writer.write(&[*handle]).map(|_| ()),
            Self::Expanded(s) => writer.write_with_null(s.as_bytes()),
            _ => Ok(()),
        }
    }
}
