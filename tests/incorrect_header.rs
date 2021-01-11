use futures_util::io::AsyncReadExt;
use tide::{Request, Server};
use tide_upload_limit::UploadLimit;

/// This tests that the behaviour is correct when the content-length header is
/// not incorrectly (or maliciously) set.

#[async_std::test]
async fn payload_over_limit() {
    let payload = "this string is 23 bytes";
    let upload_limit = 10;
    let header_length = 9;

    let response = get_response(payload, upload_limit, header_length).await;

    assert_eq!(response.status(), tide::StatusCode::PayloadTooLarge);
}

async fn get_response(
    payload: &'static str,
    upload_limit: usize,
    header_size: usize,
) -> tide::Response {
    let mut app = app();

    // set a global upload limit
    app.with(UploadLimit::new(upload_limit));

    let request = request(payload, Some(header_size));

    // get response
    app.respond(request).await.unwrap()
}

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
