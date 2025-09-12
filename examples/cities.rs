use futures_util::stream::TryStreamExt;
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
    let fut = Box::pin(reqwest::get(url));
    let mut stream = JsonStream::<_, _, City>::request(fut, JsonPart::level(1).group(0));
    while let Some(city) = stream.try_next().await.unwrap() {
        println!("{:?}", city);
    }
}
