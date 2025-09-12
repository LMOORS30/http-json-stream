use std::fmt;
use std::string::FromUtf8Error;

/// Type alias for the [`http_json_stream::Error`](Error) result.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Errors which can occur while streaming an HTTP JSON response.
#[derive(Debug)]
pub enum Error {
    Conn(Box<dyn std::error::Error + 'static>),
    Body(Box<dyn std::error::Error + 'static>),
    Json(serde_json::Error, Result<String, FromUtf8Error>),
    Http(http::StatusCode, Result<String, FromUtf8Error>),
    Io(std::io::Error),
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Conn(err) => Some(&**err),
            Error::Body(err) => Some(&**err),
            Error::Json(err, _) => Some(err),
            Error::Http(_, _) => None,
            Error::Io(err) => Some(err),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Conn(err) => write!(f, "Conn: {err}"),
            Error::Body(err) => write!(f, "Body: {err}"),
            Error::Json(err, buf) => match buf {
                Ok(json) => write!(f, "Json: {err}: {json}"),
                Err(utf8) => write!(f, "Json: {err}: {utf8}"),
            },
            Error::Http(status, body) => match body {
                Ok(text) => write!(f, "HTTP {status}: {text}"),
                Err(utf8) => write!(f, "HTTP {status}: {utf8}"),
            },
            Error::Io(err) => write!(f, "IO: {err}"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
