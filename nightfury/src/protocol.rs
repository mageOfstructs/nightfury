use std::io::{BufRead, ErrorKind, Read, Write};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    Init(String),
    GetCapabilities,
    Advance(char),
    AdvanceStr(String),
    Reset,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
    Capabilities(Vec<String>),
    Expanded(String),
    Ok,
}

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
