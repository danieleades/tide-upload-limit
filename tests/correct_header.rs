use futures_util::io::AsyncReadExt;
use tide::{Request, Server};
use tide_upload_limit::UploadLimit;

/// This tests that the behaviour is correct when the content-length header is
/// not incorrectly (or maliciously) set.

#[async_std::test]
async fn payload_over_limit() {
    let mut app = app();

    // set a global upload limit of 10 bytes
    app.with(UploadLimit::new(10));

    let request = request("this string is 23 bytes");

    // get response
    let response: tide::Response = app.respond(request).await.unwrap();

    assert_eq!(response.status(), tide::StatusCode::PayloadTooLarge);
}

#[async_std::test]
async fn payload_under_limit() {
    let mut app = app();

    // set a global upload limit of 1Kb
    app.with(UploadLimit::new(1024));

    let request = request("this string is 23 bytes");

    // get response
    let response: tide::Response = app.respond(request).await.unwrap();

    assert_ne!(response.status(), tide::StatusCode::PayloadTooLarge);
}

fn app() -> Server<()> {
    let mut app = tide::new();

    app.at("/").post(|mut request: Request<()>| async move {
        let mut buf = Vec::new();
        Ok(request.read_to_end(&mut buf).await.map(|_| "")?)
    });

    app
}

fn request(payload: impl Into<String>) -> Request<()> {
    let mut request = tide::http::Request::new(
        tide::http::Method::Post,
        tide::http::Url::parse("http://example.com").unwrap(),
    );
    request.set_body(payload.into());

    request.into()
}
