use futures_util::stream::StreamExt;
use http_json_stream::{JsonPart, JsonStream};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct City {
    pub name: String,
    pub country: String,
}

#[tokio::main]
async fn main() {
    let url = "https://raw.githubusercontent.com/lutangar/cities.json/master/cities.json";
    let fut = reqwest::Client::new()
        .get(url)
        .header("Accept-Encoding", "gzip")
        .send();
    let mut stream = JsonStream::request(fut, JsonPart::level(1).group(0));
    while let Some(city) = stream.next().await {
        let city: City = city.unwrap();
        println!("{:?}", city);
    }
}
