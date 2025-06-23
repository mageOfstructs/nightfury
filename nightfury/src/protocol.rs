use std::io::Write;

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
