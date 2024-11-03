use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductSummaryResponse {
    pub id: u32,
    pub total_price: u32,
}
