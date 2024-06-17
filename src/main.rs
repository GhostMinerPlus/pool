use std::{io, sync::Arc, time::Duration};

use earth::AsConfig;
use pool::{connector, server};
use edge_lib::{
    data::{AsDataManager, MemDataManager, RecDataManager},
    EdgeEngine, ScriptTree,
};
use serde::{Deserialize, Serialize};
use tokio::time;

#[derive(Debug, Deserialize, Serialize, Clone, AsConfig)]
struct Config {
    ip: String,
    name: String,
    port: u16,
    db_url: String,
    thread_num: u8,
    log_level: String,
    key: String,
    moon_servers: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ip: "0.0.0.0".to_string(),
            name: "pool".to_string(),
            port: 80,
            db_url: Default::default(),
            thread_num: 8,
            log_level: "INFO".to_string(),
            key: format!(""),
            moon_servers: Vec::new(),
        }
    }
}

fn main() -> io::Result<()> {
    let mut arg_v: Vec<String> = std::env::args().collect();
    arg_v.remove(0);
    let file_name = if !arg_v.is_empty() && !arg_v[0].starts_with("--") {
        arg_v.remove(0)
    } else {
        "config.toml".to_string()
    };

    let mut config = Config::default();
    config.merge_by_file(&file_name);
    if !arg_v.is_empty() {
        config.merge_by_arg_v(&arg_v);
    }

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&config.log_level))
        .init();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(config.thread_num as usize)
        .build()?
        .block_on(async {
            let dm = RecDataManager::new(Arc::new(MemDataManager::new()));
            let mut edge_engine = EdgeEngine::new(dm.divide());
            // config.ip, config.port, config.name
            let base_script = [
                format!("root->name = = {} _", config.name),
                format!("root->ip = = {} _", config.ip),
                format!("root->port = = {} _", config.port),
                format!("root->path = = {} _", format!("/{}", config.name)),
                format!("root->key = = {} _", config.key),
            ]
            .join("\n");
            let option_script = config
                .moon_servers
                .into_iter()
                .map(|moon_server| format!("root->moon_server += = {moon_server} _"))
                .reduce(|acc, line| format!("{acc}\n{line}"))
                .unwrap_or(String::new());
            edge_engine
                .execute1(&ScriptTree {
                    script: format!("{base_script}\n{option_script}"),
                    name: "".to_string(),
                    next_v: vec![],
                })
                .await?;
            edge_engine.commit().await?;

            tokio::spawn(connector::HttpConnector::new(dm.divide()).run());
            tokio::spawn(server::HttpServer::new(dm.divide()).run());
            loop {
                log::info!("alive");
                time::sleep(Duration::from_secs(10)).await;
            }
        })
}
