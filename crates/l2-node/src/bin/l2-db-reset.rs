use anyhow::{bail, Context};
use sqlx::postgres::PgPoolOptions;

const NODE_TABLES: &[&str] = &[
    "observer_checkpoints",
    "l1_batch_finalizations",
    "l1_batch_commits",
    "l2_batch_payloads",
    "ent_faucet_grants",
    "l2_withdrawals",
    "l2_transactions",
    "l2_blocks",
    "l2_deposits",
    "l1_cursors",
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::from_filename(".env.local");
    let _ = dotenvy::dotenv();

    let mut yes = false;
    let mut dry_run = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--yes" => yes = true,
            "--dry-run" => dry_run = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => bail!("unknown argument: {arg}"),
        }
    }

    if !yes && !dry_run {
        bail!("refusing to reset local L2 database without --yes; use --dry-run to print SQL");
    }

    let sql = format!(
        "TRUNCATE TABLE {} RESTART IDENTITY CASCADE",
        NODE_TABLES.join(", ")
    );
    if dry_run {
        println!("{sql};");
        return Ok(());
    }

    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL is required; put it in .env.local or the environment")?;
    if database_url.trim().is_empty() {
        bail!("DATABASE_URL is empty");
    }

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .context("connect to local L2 Postgres")?;

    sqlx::query(&sql)
        .execute(&pool)
        .await
        .context("reset local L2 node tables")?;

    println!("reset local L2 node tables: {}", NODE_TABLES.join(", "));
    Ok(())
}

fn print_help() {
    println!(
        "Usage: cargo run -p l2-node --bin l2-db-reset -- --yes\n\
         \n\
         Resets local L2 node Postgres tables using DATABASE_URL from .env.local.\n\
         This does not touch TON testnet contracts, wallets, Git, or environment files.\n\
         \n\
         Options:\n\
           --yes      execute the reset\n\
           --dry-run  print the SQL without connecting"
    );
}
