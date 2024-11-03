use std::time::Duration;

use rquest::{
    header::{HeaderMap, COOKIE},
    tls::Impersonate,
    Client,
};

pub fn get_client(cookies: &str) -> Client {
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, cookies.parse().unwrap());

    Client::builder()
        .cookie_store(true)
        .impersonate(Impersonate::Chrome130)
        .default_headers(headers)
        .timeout(Duration::from_secs(5))
        .referer(true)
        .build()
        .unwrap()
}
