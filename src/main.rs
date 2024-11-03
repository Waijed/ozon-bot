use std::sync::Arc;

use data::{read_proxies, read_tasks, ProxyGroup};
use futures_util::{stream::FuturesUnordered, StreamExt};
use tokio::sync::RwLock;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::prelude::*;

pub mod data;
pub mod prelude;
pub mod task;

#[tokio::main]
async fn main() {
    let file_appender = tracing_appender::rolling::hourly("logs", "vuelo_logs");
    let (file_writer, _file_guard) = tracing_appender::non_blocking(file_appender);
    let layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_line_number(true)
        .with_file(false)
        .with_target(true);

    tracing_subscriber::registry()
        .with(layer)
        .with(tracing_subscriber::fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let proxies = read_proxies().await.expect("Failed to read proxies");
    let tasks = read_tasks().await.expect("Failed to read tasks");

    let proxy_group = if proxies.is_empty() {
        None
    } else {
        Some(Arc::new(RwLock::new(
            ProxyGroup::from_strs(proxies).expect("Failed to parse proxies"),
        )))
    };

    let mut threads = FuturesUnordered::new();
    for mut task in tasks {
        let proxy_group = proxy_group.clone();
        let thread = tokio::spawn(async move { task.run(proxy_group).await });

        threads.push(thread);
    }

    while let Some(join_result) = threads.next().await {
        let task_result = match join_result {
            Ok(task_result) => task_result,
            Err(err) => {
                error!("Failed to start thread; err={:?}", err);
                continue;
            }
        };

        match task_result {
            Ok(_) => info!("Task finished successfully"),
            Err(err) => error!("Critical task error; err={:?}", err),
        }
    }
}
