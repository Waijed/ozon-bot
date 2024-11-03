use std::{fmt::Debug, sync::Arc};

use rquest::Client;
use serde_json::Value;
use tokio::{sync::RwLock, time::Instant};
use utils::get_client;

use crate::{data::ProxyGroup, prelude::*};

use self::api::*;

pub mod api;
pub mod types;
pub mod utils;

pub struct Task {
    pub name: String,
    pub client: Client,
    pub retry_delay: u64,
    pub product_ids: Vec<u32>,
    pub cart_total_price_limit: u32,
}

impl Debug for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "name={}; product_ids={:?}; cart_limit={}",
            self.name, self.product_ids, self.cart_total_price_limit
        )
    }
}

impl Task {
    pub fn new(
        name: String,
        cookies: &str,
        retry_delay: u64,
        product_ids: Vec<u32>,
        cart_total_price_limit: u32,
    ) -> Self {
        Self {
            name,
            client: get_client(cookies),
            retry_delay,
            product_ids,
            cart_total_price_limit,
        }
    }

    pub async fn rotate_proxy(&mut self, proxy_group: &Arc<RwLock<ProxyGroup>>) -> Result<()> {
        let proxy = proxy_group.write().await.next_proxy()?;
        self.client.set_proxies(&[proxy]);

        Ok(())
    }

    pub async fn add_to_cart(&self) -> Result<()> {
        add_to_cart(&self.client, &self.product_ids).await
    }

    pub async fn get_cart_total_price(&self) -> Result<u32> {
        get_cart_total_price(&self.client).await
    }

    pub async fn get_session_uid(&self) -> Result<String> {
        get_session_uid(&self.client).await
    }

    pub async fn go_to_checkout(&self, session_uid: &str) -> Result<()> {
        go_to_checkout(&self.client, session_uid).await
    }

    pub async fn create_order(&self) -> Result<Value> {
        create_order(&self.client).await
    }

    #[tracing::instrument(skip(proxy_group))]
    pub async fn run(&mut self, proxy_group: Option<Arc<RwLock<ProxyGroup>>>) -> Result<()> {
        loop {
            info!("Adding to cart...");
            let now = Instant::now();
            match self.add_to_cart().await {
                Ok(_) => {
                    info!(
                        "Successfully added to cart; time_taken={:?}",
                        now.elapsed().as_millis()
                    );
                    break;
                }
                Err(err) => {
                    error!("Failed to add to cart: {}", err);
                    if let Some(proxy_group) = &proxy_group {
                        if let Err(err) = self.rotate_proxy(proxy_group).await {
                            error!("Failed to rotate proxy: {}", err);
                        }
                    }

                    sleep(self.retry_delay).await;
                }
            }
        }

        info!("Prepairing to cart total monitor...");
        let session_uid = loop {
            match self.get_session_uid().await {
                Ok(session_uid) => break session_uid,
                Err(err) => {
                    error!("Failed to go to cart: {}", err);
                    if let Some(proxy_group) = &proxy_group {
                        if let Err(err) = self.rotate_proxy(proxy_group).await {
                            error!("Failed to rotate proxy: {}", err);
                        }
                    }

                    sleep(self.retry_delay).await;
                }
            }
        };

        loop {
            match self.go_to_checkout(&session_uid).await {
                Ok(_) => {
                    break;
                }
                Err(err) => {
                    error!("Failed to go to checkout: {}", err);
                    if let Some(proxy_group) = &proxy_group {
                        if let Err(err) = self.rotate_proxy(proxy_group).await {
                            error!("Failed to rotate proxy: {}", err);
                        }
                    }

                    sleep(self.retry_delay).await;
                }
            }
        }

        loop {
            info!("Getting cart total price...");
            let now = Instant::now();

            match self.get_cart_total_price().await {
                Ok(total_price) => {
                    if total_price <= self.cart_total_price_limit {
                        info!(
                            "Cart total price is less than limit; cart_total_price={}; time_taken={:?}ms",
                            total_price,
                            now.elapsed().as_millis()
                        );
                        break;
                    }

                    info!(
                        "Cart total price is more than limit; cart_total_price={}; time_taken={:?}ms",
                        total_price, now.elapsed().as_millis()
                    );
                    if let Some(proxy_group) = &proxy_group {
                        if let Err(err) = self.rotate_proxy(proxy_group).await {
                            error!("Failed to rotate proxy: {}", err);
                        }
                    }
                }
                Err(err) => {
                    error!("Failed to get cart total price: {}", err);
                    if let Some(proxy_group) = &proxy_group {
                        if let Err(err) = self.rotate_proxy(proxy_group).await {
                            error!("Failed to rotate proxy: {}", err);
                        }
                    }
                }
            }

            sleep(self.retry_delay).await;
        }

        loop {
            info!("Creating order...");
            let now = Instant::now();
            match self.create_order().await {
                Ok(response) => {
                    info!(
                        "Successfully created order; time_taken={:?}ms; response={:#?}",
                        now.elapsed().as_millis(),
                        response
                    );

                    return Ok(());
                }
                Err(err) => {
                    error!("Failed to create order: {}", err);
                    if let Some(proxy_group) = &proxy_group {
                        if let Err(err) = self.rotate_proxy(proxy_group).await {
                            error!("Failed to rotate proxy: {}", err);
                        }
                    }

                    sleep(self.retry_delay).await;
                }
            }
        }
    }
}
