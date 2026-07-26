#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("XML parse error: {0}")]
    Xml(String),
    #[error("UTF-8 error: {0}")]
    Encoding(#[from] std::string::FromUtf8Error),
    #[error("file too large ({0} bytes)")]
    TooLarge(usize),
    #[error("no note events found")]
    Empty,
    #[error("ZIP error: {0}")]
    Zip(String),
    #[error("MIDI parse error: {0}")]
    Midi(String),
    #[error("ABC parse error: {0}")]
    Abc(String),
}
