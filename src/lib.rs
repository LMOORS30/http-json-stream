//! An asynchronous JSON streamer for HTTP network requests.
//!
//! See the [README](https://github.com/LMOORS30/http-json-streamer#http-json-streamer) for additional information, [Installation](https://github.com/LMOORS30/http-json-streamer#cargotoml) and [Features](https://github.com/LMOORS30/http-json-streamer#features).
//!
//! [![github-com]](https://github.com/LMOORS30/http-json-streamer)<br>[![crates-io]](https://crates.io/crates/http-json-streamer)<br>[![docs-rs]](crate)
//!
//! [github-com]: https://img.shields.io/badge/github.com-LMOORS30/http--json--streamer-5e728a?labelColor=505050&style=for-the-badge&logo=github
//! [crates-io]: https://img.shields.io/badge/crates.io-http--json--streamer-5e888a?labelColor=505050&style=for-the-badge&logo=rust
//! [docs-rs]: https://img.shields.io/badge/docs.rs-http--json--streamer-5e8a76?labelColor=505050&style=for-the-badge&logo=docs.rs
//!
//! # Example
//! ```
//! use futures_util::stream::{StreamExt, TryStreamExt};
//! use http_json_stream::{JsonPart, JsonStream};
//! use serde::de::DeserializeOwned;
//! use std::fmt::Debug;
//!
//! async fn log_json_list<T: Debug + DeserializeOwned>(url: &str) {
//!     let fut = Box::pin(reqwest::get(url));
//!     // expected JSON response: '[{}, {}, {}]'
//!     let mut stream = JsonStream::<_, _, T>::request(fut, JsonPart::level(1));
//!     while let Some(item) = stream.try_next().await.unwrap() {
//!         println!("{:?}", item);
//!     }
//! }
//!
//! async fn log_error_response<T>(resp: reqwest::Response) {
//!     if resp.status().is_client_error() {
//!         // expected JSON response: '{ "errors": [{}, {}, {}] }'
//!         let mut stream = JsonStream::process(resp, JsonPart::level(2).group(0));
//!         while let Some(item) = stream.next().await {
//!             let item: serde_json::Value = item.unwrap();
//!             println!("{:?}", item);
//!         }
//!     }
//! }
//! ```
//! <br>
//! 
//! See [`JsonStream`] and [`JsonPart`] for more information.

mod body_decoder;
mod error;
mod json_stream;
mod partial_json;

pub use body_decoder::{BodyDecoder, ContentEncoding};
pub use error::{Error, Result};
pub use json_stream::JsonStream;
pub use partial_json::{JsonPart, PartialJson};
