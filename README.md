HTTP JSON Stream<br>
[<img alt="github.com" src="https://img.shields.io/badge/github.com-LMOORS30/http--json--stream-5e728a?labelColor=343942&style=for-the-badge&logo=github" height="20">](https://github.com/LMOORS30/http-json-stream)
[<img alt="crates.io" src="https://img.shields.io/badge/crates.io-http--json--stream-5e888a?labelColor=343942&style=for-the-badge&logo=rust" height="20">](https://crates.io/crates/http-json-stream)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-http--json--stream-5e8a76?labelColor=343942&style=for-the-badge&logo=docs.rs" height="20">](https://docs.rs/http-json-stream)
====

An asynchronous JSON streamer for HTTP network requests, inspired by the [hyper-json-stream](https://github.com/arnaudpoullet/hyper-json-stream) repository.

Allows for processing items in a JSON response as they are being downloaded, enabling asynchronous consumption of large network requests with constant memory. Requires all items to be specified at the same depth, determined by the amount of object or list parents, or the amount of open `{` or `[` brackets.

##### Cargo.toml
```toml
[dependencies]
http-json-stream = { version = "0.1.2", features = ["gzip"] }
futures = "0.3"
serde = "1.0"
```

##### Features
`gzip`&ensp;—&ensp;enable support for gzip encoded responses<br>
`flate2-*`&ensp;—&ensp;gzip with the specified flate2 backend

<br>

### Usage
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-http--json--stream-5e8a76?labelColor=343942&style=for-the-badge&logo=docs.rs" height="30">](https://docs.rs/http-json-stream)

<br>

#### License

<sup>
Licensed under either of
<a href="LICENSE-APACHE">Apache License, Version 2.0</a> or
<a href="LICENSE-MIT">MIT license</a>
at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
