use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Request {
    Init(String),
    GetCapabilities,
    Advance(char),
    AdvanceStr(String),
    Reset,
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Capabilities(Vec<String>),
    Expanded(String),
    Ok,
}
