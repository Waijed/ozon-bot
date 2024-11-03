use rquest::Proxy;
use serde::{Deserialize, Serialize};

use crate::{prelude::*, task::Task};

const TASKS_FILE: &str = "tasks.csv";
const PROXIES_FILE: &str = "proxies.txt";

#[derive(Deserialize, Serialize)]
pub struct TaskData {
    pub name: String,
    pub product_ids: String,
    pub cookies: String,
    pub retry_delay: u64,
    pub cart_total_price_limit: u32,
}

impl Default for TaskData {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            product_ids: "221;222;223".to_string(),
            cookies: "your_cookies_here".to_string(),
            retry_delay: 2500,
            cart_total_price_limit: 1000,
        }
    }
}

impl From<TaskData> for Task {
    fn from(val: TaskData) -> Self {
        Task::new(
            val.name,
            &val.cookies,
            val.retry_delay,
            val.product_ids
                .split(';')
                .map(|s| s.parse().unwrap())
                .collect(),
            val.cart_total_price_limit,
        )
    }
}

#[derive(Clone)]
pub struct ProxyGroup {
    pub proxies: Vec<Proxy>,
    pub index: usize,
}

impl ProxyGroup {
    pub fn from_strs(proxies: Vec<String>) -> Result<Self> {
        let mut parsed_proxies = Vec::new();
        for proxy in proxies {
            let proxy_connection_string = format!(
                "http://{}",
                match proxy.split(':').collect::<Vec<&str>>().as_slice() {
                    [host, port] => format!("{}:{}", host, port),
                    [host, port, username, password] =>
                        format!("{}:{}@{}:{}", username, password, host, port),
                    _ => {
                        bail!("failed to convert to proxy url");
                    }
                }
            );

            let proxy = Proxy::all(proxy_connection_string)
                .map_err(|err| anyhow!("failed to parse proxy url: {:#?}", err))?;

            parsed_proxies.push(proxy);
        }

        Ok(Self {
            proxies: parsed_proxies,
            index: 0,
        })
    }

    pub fn next_proxy(&mut self) -> Result<Proxy> {
        let next_index = if self.index == self.proxies.len() - 1 {
            0
        } else {
            self.index + 1
        };
        let proxy = self
            .proxies
            .get(next_index)
            .ok_or(anyhow!("proxy not found"))?;

        self.index = next_index;

        Ok(proxy.clone())
    }
}

pub async fn read_proxies() -> Result<Vec<String>> {
    let file = tokio::fs::read_to_string(PROXIES_FILE).await?;
    if file.is_empty() {
        return Ok(vec![]);
    };

    let proxies = file.split('\n').map(|s| s.to_string()).collect();

    Ok(proxies)
}

pub async fn read_tasks() -> Result<Vec<Task>> {
    let mut reader = match csv::Reader::from_path(TASKS_FILE) {
        Ok(reader) => reader,
        Err(error) => {
            if error.to_string().contains("No such file or directory") {
                write_default_tasks().await?;
                bail!("Fill the tasks.csv file");
            }

            return Err(anyhow!("could not read csv file: {}", error));
        }
    };

    let records = reader.records();

    let mut tasks = Vec::new();
    for record in records {
        let record = record?;
        let task: TaskData = match record.deserialize(None) {
            Ok(task) => task,
            Err(_error) => {
                continue;
            }
        };

        tasks.push(task.into());
    }

    Ok(tasks)
}

pub async fn write_default_tasks() -> Result<()> {
    let mut writer = csv::Writer::from_path(TASKS_FILE)
        .map_err(|error| anyhow!("could not write csv file {}", error))?;

    {
        let record = TaskData::default();
        writer
            .serialize(record)
            .map_err(|error| anyhow!("could not serialize csv record: {}", error))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_proxies() {
        let proxies = match read_proxies().await {
            Ok(proxies) => {
                println!("proxies: {:?}", proxies);
                proxies
            }
            Err(err) => panic!("failed to read proxies: {:#?}", err),
        };

        if !proxies.is_empty() {
            assert!(ProxyGroup::from_strs(proxies).is_ok());
        }
    }

    #[tokio::test]
    async fn test_read_tasks() {
        let read_tasks_result = read_tasks().await;
        if let Err(err) = read_tasks_result {
            if !err.to_string().contains("Fill the tasks.csv file") {
                panic!("failed to read tasks: {:#?}", err);
            }
        }
    }
}
