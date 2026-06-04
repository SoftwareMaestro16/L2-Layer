use anyhow::{bail, Context};
use l2_node::storage::{
    BatchCommitStatus, BatchFinalizationRecord, BatchFinalizationStatus, PostgresStorage, Storage,
};

#[derive(Clone, Debug, Default)]
struct Args {
    confirmed_through: Option<u64>,
    finalized_through: Option<u64>,
    yes: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::from_filename(".env.local");
    let _ = dotenvy::dotenv();

    let args = parse_args()?;
    if !args.yes {
        bail!("refusing to reconcile local L2 batch state without --yes");
    }

    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL is required; put it in .env.local or the environment")?;
    if database_url.trim().is_empty() {
        bail!("DATABASE_URL is empty");
    }

    let storage = PostgresStorage::connect(&database_url)
        .await
        .context("connect to local L2 Postgres")?;

    if let Some(batch_no) = args.confirmed_through {
        confirm_commits(&storage, batch_no).await?;
    }
    if let Some(batch_no) = args.finalized_through {
        finalize_batches(&storage, batch_no).await?;
    }

    Ok(())
}

async fn confirm_commits(storage: &PostgresStorage, through: u64) -> anyhow::Result<()> {
    if through == 0 {
        bail!("--confirmed-through must be non-zero");
    }
    for batch_no in 1..=through {
        let mut record = storage
            .get_batch_commit(batch_no)
            .await?
            .with_context(|| format!("batch {batch_no} commit record is missing"))?;
        record.status = BatchCommitStatus::Confirmed;
        record.last_error = None;
        storage.save_batch_commit(record).await?;
        println!("marked batch {batch_no} commit confirmed");
    }
    Ok(())
}

async fn finalize_batches(storage: &PostgresStorage, through: u64) -> anyhow::Result<()> {
    if through == 0 {
        bail!("--finalized-through must be non-zero");
    }
    for batch_no in 1..=through {
        let commit = storage
            .get_batch_commit(batch_no)
            .await?
            .with_context(|| format!("batch {batch_no} commit record is missing"))?;
        if commit.status != BatchCommitStatus::Confirmed {
            bail!("batch {batch_no} commit must be confirmed before finalization reconcile");
        }

        let mut record = storage
            .get_batch_finalization(batch_no)
            .await?
            .unwrap_or_else(|| BatchFinalizationRecord::pending(&commit, 0));
        record.status = BatchFinalizationStatus::Finalized;
        record.last_error = None;
        storage.save_batch_finalization(record).await?;
        println!("marked batch {batch_no} finalized");
    }
    Ok(())
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = Args::default();
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--confirmed-through" => {
                args.confirmed_through = Some(parse_u64_flag(
                    "--confirmed-through",
                    iter.next().as_deref(),
                )?);
            }
            "--finalized-through" => {
                args.finalized_through = Some(parse_u64_flag(
                    "--finalized-through",
                    iter.next().as_deref(),
                )?);
            }
            "--yes" => args.yes = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => bail!("unknown argument: {arg}"),
        }
    }
    if args.confirmed_through.is_none() && args.finalized_through.is_none() {
        bail!("pass --confirmed-through, --finalized-through, or both");
    }
    Ok(args)
}

fn parse_u64_flag(flag: &'static str, value: Option<&str>) -> anyhow::Result<u64> {
    value
        .context(format!("{flag} requires a value"))?
        .parse::<u64>()
        .with_context(|| format!("{flag} must be an unsigned integer"))
}

fn print_help() {
    println!(
        "Usage: cargo run -p l2-node --bin l2-batch-reconcile -- \\\n\
         --confirmed-through <batch_no> --finalized-through <batch_no> --yes\n\
         \n\
         Reconciles local Postgres batch relay/finality rows after manual Acton\n\
         testnet CommitBatch/FinalizeBatch operations. It does not verify L1;\n\
         use only after checking RollupRoot on testnet."
    );
}
