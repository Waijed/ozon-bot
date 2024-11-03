use lazy_static::lazy_static;
use regex::Regex;
use rquest::{Client, StatusCode};
use serde_json::{json, Value};

use crate::prelude::*;

use super::types::ProductSummaryResponse;

const BASE_API_URL: &str = "https://www.ozon.ru/api/composer-api.bx/_action";

lazy_static! {
    static ref SESSION_UID_REGEX: Regex = Regex::new(r"session_uid=([a-f0-9-]+)").unwrap();
}

pub async fn add_to_cart(client: &Client, product_ids: &[u32]) -> Result<()> {
    let json = product_ids
        .iter()
        .map(|id| json!({"id": id, "quantity": 1}))
        .collect::<Vec<_>>();

    let request = client.post(format!("{BASE_API_URL}/addToCart")).json(&json);
    let response = request
        .send()
        .await
        .map_err(|err| anyhow!("failed to send request {}", err.without_url().to_string()))?;

    let response_status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|err| anyhow!("failed to get response text {}", err.to_string()))?;

    if !response_status.is_success() {
        bail!("bad status {}", response_status,);
    }

    let deserialized_response: Value = serde_json::from_str(&response_text)
        .map_err(|err| anyhow!("failed to deserialize response {}", err))?;

    if !deserialized_response["success"].as_bool().unwrap_or(false) {
        bail!("bad response {:#?}", deserialized_response);
    }

    Ok(())
}

pub async fn get_session_uid(client: &Client) -> Result<String> {
    let request = client.get("https://www.ozon.ru/cart");
    let response = request
        .send()
        .await
        .map_err(|err| anyhow!("failed to send request {}", err.without_url().to_string()))?;

    if !response.status().is_success() {
        bail!("bad status {}", response.status());
    }

    let response_text = response
        .text()
        .await
        .map_err(|err| anyhow!("failed to get response text {}", err.to_string()))?;

    if let Some(captures) = SESSION_UID_REGEX.captures(&response_text) {
        let session_uid = captures
            .get(1)
            .ok_or(anyhow!("session_uid not found"))?
            .as_str();

        Ok(session_uid.to_string())
    } else {
        bail!("session_uid not found");
    }
}

pub async fn go_to_checkout(client: &Client, session_uid: &str) -> Result<()> {
    let request = client.get(format!(
        "https://www.ozon.ru/gocheckout?start=0&activeTab=0&session_uid={session_uid}&snp=false"
    ));
    let response = request
        .send()
        .await
        .map_err(|err| anyhow!("failed to send request {}", err.without_url().to_string()))?;

    if !response.status().is_success() {
        bail!("bad status {}", response.status());
    }

    Ok(())
}

pub async fn get_cart_total_price(client: &Client) -> Result<u32> {
    let request = client.get(format!("{BASE_API_URL}/summary"));
    let response = request
        .send()
        .await
        .map_err(|err| anyhow!("failed to send request {}", err.without_url().to_string()))?;

    let response_status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|err| anyhow!("failed to get response text {}", err.to_string()))?;

    if !response_status.is_success() {
        bail!("bad status {}", response_status,);
    }

    let deserialized_response: Vec<ProductSummaryResponse> =
        serde_json::from_str(&response_text)
            .map_err(|err| anyhow!("failed to deserialize response {}", err))?;

    let total_cart_price: u32 = deserialized_response
        .iter()
        .map(|product| product.total_price)
        .sum();

    Ok(total_cart_price)
}

pub async fn create_order(client: &Client) -> Result<Value> {
    let request = client.get(format!("{BASE_API_URL}/v2/createOrder"));
    let response = request
        .send()
        .await
        .map_err(|err| anyhow!("failed to send request {}", err.without_url().to_string()))?;

    let response_status = response.status();
    let response_text = response
        .text()
        .await
        .map_err(|err| anyhow!("failed to get response text {}", err.to_string()))?;

    if !response_status.is_success() {
        if response_status != StatusCode::FORBIDDEN {
            bail!("bad status {}; text={:?}", response_status, response_text);
        }

        bail!("bad status {}", response_status);
    }

    let deserialized_response: Value = serde_json::from_str(&response_text)
        .map_err(|err| anyhow!("failed to deserialize response {}", err))?;

    if let Some(err) = deserialized_response["error"].as_str() {
        if !err.is_empty() {
            bail!("bad response {}", err);
        }
    }

    Ok(deserialized_response)
}

#[cfg(test)]
mod tests {
    use crate::task::utils::get_client;

    use super::*;

    // TODO: read from env.test
    const COOKIES: &str = "";

    #[tokio::test]
    async fn test_reservate() {
        let client = get_client(COOKIES);
        add_to_cart(&client, &[223822057]).await.unwrap();

        let total_price = get_cart_total_price(&client).await.unwrap();
        assert!(total_price > 0);

        let session_uid = get_session_uid(&client).await.unwrap();
        go_to_checkout(&client, &session_uid).await.unwrap();

        let order = create_order(&client).await.unwrap();
        println!("{:#?}", order);
    }
}
