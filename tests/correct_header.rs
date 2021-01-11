use tide_upload_limit::UploadLimit;

mod common;
use common::{app, request};

/// This tests that the behaviour is correct when the content-length header is
/// not incorrectly (or maliciously) set.

#[async_std::test]
async fn payload_over_limit() {
    let upload_limit = 10;
    let response = get_response(upload_limit).await;

    assert_eq!(response.status(), tide::StatusCode::PayloadTooLarge);
}

#[async_std::test]
async fn payload_under_limit() {
    let upload_limit = 1024;
    let response = get_response(upload_limit).await;

    assert_ne!(response.status(), tide::StatusCode::PayloadTooLarge);
}

async fn get_response(upload_limit: usize) -> tide::Response {
    let mut app = app();

    // set a global upload limit
    app.with(UploadLimit::new(upload_limit));

    let request = request("this string is 23 bytes", None);

    // get response
    app.respond(request).await.unwrap()
}
