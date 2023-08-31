use clap::{Parser, Subcommand};
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use std::path::PathBuf;
use tokio::sync::OnceCell;

mod cut;
mod duration_util;
mod error;
mod fingerprint;
mod pack;
mod command;

use error::AppError;

pub static PROGRESS: OnceCell<MultiProgress> = OnceCell::const_new();

/// awecut - quickly cut out midrolls and more.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
enum Command {
    UpdateFingerprints {
        /// Input dirs with the media to fingerprint.
        #[arg(required = true)]
        input: Vec<PathBuf>,
    },
    Cut {
        /// Input media to cut
        #[arg(required = true)]
        input: PathBuf,

        /// Output media
        #[arg(required = true)]
        output: PathBuf,

        /// Inclusion fingerprint list (as pack-files)
        #[arg(long, short)]
        include: Vec<PathBuf>,

        /// Exclusion fingerprint list (as pack-files)
        #[arg(long, short)]
        exclude: Vec<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let logger =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("awecut"))
            .build();
    PROGRESS
        .set(MultiProgress::new())
        .map_err(|err| AppError::Unexpected {
            context: format!("PROGRESS initilization failed, cause: {:?}", err),
        })?;
    LogWrapper::new(PROGRESS.get().unwrap().clone(), logger)
        .try_init()
        .map_err(|err| AppError::Unexpected {
            context: format!("LogWrapper initilization failed, cause: {:?}", err),
        })?;

    let args = Args::parse();

    match args.command {
        Command::UpdateFingerprints { input } => {
            fingerprint::update_fingerprints(input).await?;
        }
        Command::Cut {
            input,
            include,
            exclude,
            output,
        } => {
            let input = std::fs::canonicalize(input)?;
            let output = std::fs::canonicalize(output)?;

            if include.len() + exclude.len() > 0 {
                let (inc, exc) = cut::cut_matches(input.clone(), include, exclude).await?;
                cut::cut_interactive(inc, exc, input, output).await?;
            } else {
                cut::cut_interactive(vec![], vec![], input, output).await?;
            }
        }
    }

    Ok(())
}
