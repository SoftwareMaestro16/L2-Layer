use l2_node::api;
use l2_node::config::NodeConfig;
use l2_node::mempool::build_mempool;
use l2_node::storage::build_storage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "l2_node=info,tower_http=info".to_owned()),
        )
        .init();

    let config = NodeConfig::from_env()?;
    let storage = build_storage(&config).await?;
    let mempool = build_mempool(&config).await?;
    api::serve(config, storage, mempool).await
}
