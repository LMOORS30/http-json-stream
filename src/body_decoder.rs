#[cfg(feature = "flate2")]
use flate2::write::MultiGzDecoder;
use http::HeaderMap;
use std::io::Write;

/// Supported HTTP Content-Encoding headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ContentEncoding {
    #[cfg(feature = "flate2")]
    Gzip,
    #[default]
    None,
}

impl From<&HeaderMap> for ContentEncoding {
    fn from(headers: &HeaderMap) -> Self {
        match headers.get("Content-Encoding") {
            Some(encoding) => match encoding.to_str().ok() {
                #[cfg(feature = "flate2")]
                Some("gzip") => ContentEncoding::Gzip,
                _ => ContentEncoding::default(),
            },
            None => ContentEncoding::default(),
        }
    }
}

/// A streaming decoder that abstracts over supported content encodings.
#[derive(Debug)]
pub enum BodyDecoder<W: Write> {
    #[cfg(feature = "flate2")]
    Gzip(MultiGzDecoder<W>),
    None(W),
}

impl<W: Write> BodyDecoder<W> {
    /// Creates a new decoder which will write uncompressed data to the stream.
    pub fn new(writer: W, encoding: ContentEncoding) -> Self {
        match encoding {
            #[cfg(feature = "flate2")]
            ContentEncoding::Gzip => Self::gzip(writer),
            ContentEncoding::None => Self::None(writer),
        }
    }
    /// Creates a new decoder which will write uncompressed gzip data to the stream.
    #[cfg(feature = "flate2")]
    pub fn gzip(writer: W) -> Self {
        Self::Gzip(MultiGzDecoder::new(writer))
    }
}

impl<W: Write> BodyDecoder<W> {
    /// Acquires a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        match self {
            #[cfg(feature = "flate2")]
            BodyDecoder::Gzip(writer) => writer.get_ref(),
            BodyDecoder::None(writer) => writer,
        }
    }
    /// Acquires a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        match self {
            #[cfg(feature = "flate2")]
            BodyDecoder::Gzip(writer) => writer.get_mut(),
            BodyDecoder::None(writer) => writer,
        }
    }
    /// See [`MultiGzDecoder::try_finish`].
    pub fn try_finish(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(feature = "flate2")]
            BodyDecoder::Gzip(writer) => writer.try_finish(),
            BodyDecoder::None(_) => Ok(()),
        }
    }
    /// See [`MultiGzDecoder::finish`].
    pub fn finish(self) -> std::io::Result<W> {
        match self {
            #[cfg(feature = "flate2")]
            BodyDecoder::Gzip(writer) => writer.finish(),
            BodyDecoder::None(writer) => Ok(writer),
        }
    }
}

impl<W: Write> Write for BodyDecoder<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(feature = "flate2")]
            BodyDecoder::Gzip(writer) => writer.write(buf),
            BodyDecoder::None(writer) => writer.write(buf),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            #[cfg(feature = "flate2")]
            BodyDecoder::Gzip(writer) => writer.flush(),
            BodyDecoder::None(writer) => writer.flush(),
        }
    }
}
