use crate::{BodyDecoder, ContentEncoding, JsonPart, PartialJson};
use bytes::Buf;
use futures_core::{FusedStream, Future, Stream};
use http::{Response, StatusCode};
use http_body::Body;
use serde::de::DeserializeOwned;
use std::convert::Infallible;
use std::fmt;
use std::future::Ready;
use std::io::Write;
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{Context, Poll};

/// An asynchronous JSON streamer for HTTP network requests.
#[must_use = "streams do nothing unless polled"]
pub struct JsonStream<C, B, T> {
    state: State<C, B, T>,
    part: JsonPart,
}

impl<C, B, T, D, E> JsonStream<C, B, T>
where
    C: Future<Output = Result<D, E>> + Unpin,
    D: Into<Response<B>>,
    B: Body + Unpin,
    T: DeserializeOwned,
    E: std::error::Error + 'static,
    B::Error: std::error::Error + 'static,
{
    /// Creates a new JSON streamer from the given HTTP response future.
    ///
    /// Completes the request and streams the body for status 2XX responses.
    ///
    /// Use [`JsonStream::process`] instead to customize response handling.
    pub fn request(call: C, part: JsonPart) -> Self {
        JsonStream {
            state: State::Connecting(call),
            part,
        }
    }
}

impl<B, T> JsonStream<Ready<Result<Response<B>, Infallible>>, B, T>
where
    B: Body + Unpin,
    T: DeserializeOwned,
    B::Error: std::error::Error + 'static,
{
    /// Creates a new JSON streamer from the given HTTP response.
    pub fn process(resp: impl Into<Response<B>>, part: JsonPart) -> Self {
        let resp = resp.into();
        let writer = PartialJson::new(part);
        let encoding = ContentEncoding::from(resp.headers());
        let json = BodyDecoder::new(writer, encoding);
        JsonStream {
            state: State::Streaming { resp, json },
            part,
        }
    }
}

enum State<C, B, T> {
    Connecting(C),
    Reporting {
        resp: Response<B>,
        text: BodyDecoder<Vec<u8>>,
    },
    Streaming {
        resp: Response<B>,
        json: BodyDecoder<PartialJson<T>>,
    },
    Finished,
}

unsafe impl<C, B, T> Send for State<C, B, T> {}
unsafe impl<C, B, T> Sync for State<C, B, T> {}
impl<C, B, T> Unpin for State<C, B, T> {}

impl<C, B, T> fmt::Debug for JsonStream<C, B, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.state {
            State::Connecting(_) => f.pad("JsonStream(Connecting)"),
            State::Reporting { .. } => f.pad("JsonStream(Reporting)"),
            State::Streaming { .. } => f.pad("JsonStream(Streaming)"),
            State::Finished => f.pad("JsonStream(Finished)"),
        }
    }
}

impl<C, B, T, D, E> FusedStream for JsonStream<C, B, T>
where
    C: Future<Output = Result<D, E>> + Unpin,
    D: Into<Response<B>>,
    B: Body + Unpin,
    T: DeserializeOwned,
    E: std::error::Error + 'static,
    B::Error: std::error::Error + 'static,
{
    fn is_terminated(&self) -> bool {
        matches!(self.state, State::Finished)
    }
}

impl<C, B, T, D, E> Stream for JsonStream<C, B, T>
where
    C: Future<Output = Result<D, E>> + Unpin,
    D: Into<Response<B>>,
    B: Body + Unpin,
    T: DeserializeOwned,
    E: std::error::Error + 'static,
    B::Error: std::error::Error + 'static,
{
    type Item = crate::Result<T>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match self.as_mut().pull(cx) {
                Pull::Pending => return Poll::Pending,
                Pull::Ready(t) => return Poll::Ready(Some(t)),
                Pull::Done => return Poll::Ready(None),
                Pull::Repeat => {}
            }
        }
    }
}

enum Pull<T> {
    Pending,
    Ready(T),
    Repeat,
    Done,
}

impl<C, B, T, D, E> JsonStream<C, B, T>
where
    C: Future<Output = Result<D, E>> + Unpin,
    D: Into<Response<B>>,
    B: Body + Unpin,
    T: DeserializeOwned,
    E: std::error::Error + 'static,
    B::Error: std::error::Error + 'static,
{
    fn pull(&mut self, cx: &mut Context<'_>) -> Pull<crate::Result<T>> {
        match &mut self.state {
            State::Connecting(conn) => match Pin::new(conn).poll(cx) {
                Poll::Pending => Pull::Pending,
                Poll::Ready(Err(err)) => {
                    self.state = State::Finished;
                    Pull::Ready(Err(crate::Error::Conn(Box::new(err))))
                }
                Poll::Ready(Ok(resp)) => {
                    let resp = resp.into();
                    match resp.status() {
                        StatusCode::NO_CONTENT => {
                            self.state = State::Finished;
                            Pull::Done
                        }
                        status if status.is_success() => {
                            let writer = PartialJson::new(self.part);
                            let encoding = ContentEncoding::from(resp.headers());
                            let json = BodyDecoder::new(writer, encoding);
                            self.state = State::Streaming { resp, json };
                            Pull::Repeat
                        }
                        _ => {
                            let writer = Vec::new();
                            let encoding = ContentEncoding::from(resp.headers());
                            let text = BodyDecoder::new(writer, encoding);
                            self.state = State::Reporting { resp, text };
                            Pull::Repeat
                        }
                    }
                }
            },
            State::Reporting { resp, text } => {
                let body = resp.body_mut();
                match Pin::new(body).poll_frame(cx) {
                    Poll::Pending => Pull::Pending,
                    Poll::Ready(Some(Err(err))) => {
                        self.state = State::Finished;
                        Pull::Ready(Err(crate::Error::Body(Box::new(err))))
                    }
                    Poll::Ready(Some(Ok(buf))) => {
                        if let Ok(mut data) = buf.into_data() {
                            while data.remaining() > 0 {
                                let chunk = data.chunk();
                                if let Err(err) = text.write_all(chunk) {
                                    return Pull::Ready(Err(err.into()));
                                }
                                data.advance(chunk.len());
                            }
                        }
                        Pull::Repeat
                    }
                    Poll::Ready(None) => {
                        if let Err(err) = text.flush().and_then(|_| text.try_finish()) {
                            return Pull::Ready(Err(err.into()));
                        }
                        let status = resp.status();
                        let mut body = Vec::new();
                        std::mem::swap(&mut body, text.get_mut());
                        let body = String::from_utf8(body);
                        self.state = State::Finished;
                        Pull::Ready(Err(crate::Error::Http(status, body)))
                    }
                }
            }
            State::Streaming { resp, json } => {
                if let Some(item) = json.get_mut().next() {
                    return Pull::Ready(item);
                }
                let body = resp.body_mut();
                match Pin::new(body).poll_frame(cx) {
                    Poll::Pending => Pull::Pending,
                    Poll::Ready(Some(Err(err))) => {
                        self.state = State::Finished;
                        Pull::Ready(Err(crate::Error::Body(Box::new(err))))
                    }
                    Poll::Ready(Some(Ok(buf))) => {
                        if let Ok(mut data) = buf.into_data() {
                            while data.remaining() > 0 {
                                let chunk = data.chunk();
                                if let Err(err) = json.write_all(chunk) {
                                    return Pull::Ready(Err(err.into()));
                                }
                                data.advance(chunk.len());
                            }
                        }
                        if let Err(err) = json.flush() {
                            return Pull::Ready(Err(err.into()));
                        }
                        match json.get_mut().next() {
                            Some(item) => Pull::Ready(item),
                            None => Pull::Repeat,
                        }
                    }
                    Poll::Ready(None) => {
                        if let Err(err) = json.try_finish() {
                            return Pull::Ready(Err(err.into()));
                        };
                        if let Some(item) = json.get_mut().next() {
                            return Pull::Ready(item);
                        }
                        let last = json.get_mut().done();
                        self.state = State::Finished;
                        match last {
                            Some(last) => Pull::Ready(last),
                            None => Pull::Done,
                        }
                    }
                }
            }
            State::Finished => Pull::Done,
        }
    }
}
