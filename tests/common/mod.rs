use futures_util::io::AsyncReadExt;
use tide::{Request, Server};

pub fn app() -> Server<()> {
    let mut app = tide::new();

    app.at("/").post(handle);

    app
}

async fn handle(mut request: Request<()>) -> Result<String, tide::Error> {
    println!("'handle' called");
    let mut buf = Vec::new();

    request.read_to_end(&mut buf).await?;

    let s = String::from_utf8(buf)?;

    println!("read string: {}", s);

    Ok(s)
}

pub fn request(payload: &'static str, payload_length: Option<usize>) -> Request<()> {
    let length = payload_length.or_else(|| Some(payload.len()));
    let reader = futures_util::io::BufReader::new(payload.as_bytes());

    let body = tide::http::Body::from_reader(reader, length);

    let mut request = tide::http::Request::new(
        tide::http::Method::Post,
        tide::http::Url::parse("http://example.com").unwrap(),
    );

    request.set_body(body);

    request.into()
}
