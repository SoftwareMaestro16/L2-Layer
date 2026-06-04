use l2_node::config::SecretString;
use l2_node::signer::{
    build_signer_router, CommandSignerBackend, SignerRole, SignerServiceConfig,
    DEFAULT_SIGNER_COMMAND_TIMEOUT_MS, DEFAULT_SIGNER_MAX_BODY_BYTES,
    DEFAULT_SIGNER_RATE_LIMIT_PER_MINUTE,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "l2_node=info,tower_http=info".to_owned()),
        )
        .init();
    let _ = dotenvy::from_filename(".env.local");
    let _ = dotenvy::dotenv();

    let addr: SocketAddr = optional_env("L2_SIGNER_ADDR", "127.0.0.1:8800").parse()?;
    let token = SecretString::new(required_env("L2_SIGNER_TOKEN")?)?;
    let signer_address = required_env("L2_SIGNER_ADDRESS")?;
    let role = SignerRole::from_str(&optional_env("L2_SIGNER_ROLE", "sequencer"))?;
    let max_body_bytes =
        parse_usize_env("L2_SIGNER_MAX_BODY_BYTES", DEFAULT_SIGNER_MAX_BODY_BYTES)?;
    let rate_limit = parse_u32_env(
        "L2_SIGNER_RATE_LIMIT_PER_MINUTE",
        DEFAULT_SIGNER_RATE_LIMIT_PER_MINUTE,
    )?;
    let command = PathBuf::from(required_env("L2_SIGNER_COMMAND")?);
    let command_timeout_ms = parse_u64_env(
        "L2_SIGNER_COMMAND_TIMEOUT_MS",
        DEFAULT_SIGNER_COMMAND_TIMEOUT_MS,
    )?;

    let config = SignerServiceConfig {
        token,
        signer_address,
        role,
        max_body_bytes,
        rate_limit_per_minute: rate_limit,
    };
    config.validate()?;
    let backend = CommandSignerBackend::new(command, Duration::from_millis(command_timeout_ms));
    let app = build_signer_router(config.clone(), backend);

    tracing::info!(
        addr = %addr,
        signer_address = %config.signer_address,
        role = ?config.role,
        max_body_bytes = config.max_body_bytes,
        rate_limit_per_minute = config.rate_limit_per_minute,
        "starting l2 signer service"
    );
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn required_env(key: &str) -> anyhow::Result<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{key} is required"))
}

fn optional_env(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn parse_usize_env(key: &str, default: usize) -> anyhow::Result<usize> {
    optional_env(key, &default.to_string())
        .parse()
        .map_err(|_| anyhow::anyhow!("{key} must be an unsigned integer"))
}

fn parse_u32_env(key: &str, default: u32) -> anyhow::Result<u32> {
    optional_env(key, &default.to_string())
        .parse()
        .map_err(|_| anyhow::anyhow!("{key} must be an unsigned 32-bit integer"))
}

fn parse_u64_env(key: &str, default: u64) -> anyhow::Result<u64> {
    optional_env(key, &default.to_string())
        .parse()
        .map_err(|_| anyhow::anyhow!("{key} must be an unsigned 64-bit integer"))
}
