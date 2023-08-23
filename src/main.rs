use clap::{Parser, Subcommand};
use std::path::PathBuf;
use futures_util::stream::{FuturesUnordered, StreamExt};
use futures_util::{Future};
use std::pin::Pin;
use std::boxed::Box;
use tokio::fs::{read_dir};
use indicatif::{MultiProgress, ProgressBar};

mod error;
mod pack;

use error::AppError;
use pack::PackFile;

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
        #[arg(long, short)]
        input: Vec<PathBuf>,
    },
    Cut,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    env_logger::init();
    let args = Args::parse();

    match args.command {
        Command::UpdateFingerprints{input} => {
            update_fingerprints(input).await?;
        },
        Command::Cut => {},
    }

    Ok(())
}

async fn update_fingerprints(inputs: Vec<PathBuf>) -> Result<(), AppError> {
    let fut_files : FuturesUnordered<Pin<Box<dyn Future<Output = Vec<PathBuf>>>>> = FuturesUnordered::new();
    let fut_packs : FuturesUnordered<Pin<Box<dyn Future<Output = Result<(PathBuf, PackFile), AppError>>>>> = FuturesUnordered::new();
    for input in inputs {
        if let Some(parent) = input.parent() {
            if let Some(name) = input.file_name() {
                let mut pack = parent.with_file_name(name);
                let _ = pack.set_extension(".pck");
                match tokio::fs::try_exists(&pack).await {
                    Ok(true) => {
                        fut_packs.push(Box::pin(async move {
                            let mut reader = tokio::io::BufReader::new(tokio::fs::File::open(&pack).await?);
                            let pack_file = PackFile::decode_async(&mut reader).await?;
                            Ok::<(PathBuf, PackFile), AppError>((pack, pack_file))
                        }));
                    },
                    Ok(false) => {},
                    Err(err) => {
                        log::error!("Unable to verify if the packed fingerprints exist at {}. It will certainly failed to save too. Cause:\n{:?}", pack.display(), err);
                    }
                }
            } else {
                log::warn!("No filename available for path {}, certainly due to a non-canonicalized path.", input.display());
            }
        } else {
            log::warn!("No parent available for path {}.", input.display());
        }
        fut_files.push(Box::pin(async move {
            match read_dir(&input).await {
                Ok(mut search) => {

                    let mut files : Vec<PathBuf> = Vec::new();
                    loop {
                        match search.next_entry().await {
                            Ok(Some(entry)) => {
                                let ft = entry.file_type().await;
                                if ft.is_ok() && ft.unwrap().is_file() {
                                    files.push(entry.path());
                                }
                                
                            },
                            Ok(None) => { break },
                            Err(err) => {
                                log::error!("An error occured while listing directory {}, cause:\n{:?}", input.display(), err);
                                break;
                            }
                        }
                    }
                    files
                },
                Err(err) => {
                    log::error!("Failed to list files in directory {}, cause:\n{:?}", input.display(), err);
                    vec![]
                },
            }
        }))
    }
    let files : Vec<Vec<PathBuf>> = fut_files.collect().await;
    let files : Vec<PathBuf> = files.into_iter().flatten().collect();
    let packs : Vec<Result<(PathBuf, PackFile), AppError>> = fut_packs.collect().await;

    // TODO

    Ok(())
}