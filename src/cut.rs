use futures_util::stream::{FuturesUnordered, StreamExt};
use futures_util::Future;
use indicatif::{ProgressBar, ProgressStyle};
use rusty_chromaprint::{match_fingerprints, Configuration, MatchError, Segment};
use std::boxed::Box;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::fmt::Display;

use crate::duration_util::DurationDisplay;
use crate::error::{CutError, PackError};
use crate::fingerprint::calculate_fingerprint;
use crate::pack::PackFile;
use crate::PROGRESS;

type SegmentMatch = (Arc<String>, Segment);
type SegmentMatches = Vec<SegmentMatch>;

pub async fn cut_matches(
    input: PathBuf,
    include: Vec<PathBuf>,
    exclude: Vec<PathBuf>,
) -> Result<(Vec<SegmentMatch>, Vec<SegmentMatch>), CutError> {
    let mut include_fgs: FuturesUnordered<
        Pin<Box<dyn Future<Output = Result<Vec<(String, Vec<u32>)>, CutError>>>>,
    > = FuturesUnordered::new();
    let mut exclude_fgs: FuturesUnordered<
        Pin<Box<dyn Future<Output = Result<Vec<(String, Vec<u32>)>, CutError>>>>,
    > = FuturesUnordered::new();

    let pack_bar = PROGRESS
        .get()
        .expect("PROGRESS not initialized!")
        .add(ProgressBar::new((include.len() + exclude.len()) as u64));
    pack_bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{pos:>4}/{len:<4}] {wide_msg} [{bar:40.cyan/blue}] [ETA: {eta}]",
        )
        .unwrap(),
    );

    for inc in include {
        let bar = pack_bar.clone();
        include_fgs.push(Box::pin(async move {
            bar.set_message(format!("loading inclusion pack {}...", inc.display()));
            let mut reader = tokio::io::BufReader::new(tokio::fs::File::open(&inc).await?);
            let pack_file = PackFile::decode_async(&mut reader).await?;
            bar.inc(1);
            Ok::<Vec<(String, Vec<u32>)>, CutError>(pack_file.fingerprint)
        }));
    }

    for exc in exclude {
        let bar = pack_bar.clone();
        exclude_fgs.push(Box::pin(async move {
            bar.set_message(format!("loading exclusion pack {}...", exc.display()));
            let mut reader = tokio::io::BufReader::new(tokio::fs::File::open(&exc).await?);
            let pack_file = PackFile::decode_async(&mut reader).await?;
            bar.inc(1);
            Ok::<Vec<(String, Vec<u32>)>, CutError>(pack_file.fingerprint)
        }));
    }

    let config = Arc::new(Configuration::preset_test3());
    let input_fg = Arc::new(calculate_fingerprint(&input, &config).await?);
    let mut inc_segments: FuturesUnordered<Pin<Box<dyn Future<Output = (String, Vec<Segment>)>>>> =
        FuturesUnordered::new();
    let mut exc_segments: FuturesUnordered<Pin<Box<dyn Future<Output = (String, Vec<Segment>)>>>> =
        FuturesUnordered::new();
    let seg_bar = Arc::new(
        PROGRESS
            .get()
            .expect("PROGRESS not initialized!")
            .add(ProgressBar::new(0)),
    );
    seg_bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{pos:>4}/{len:<4}] {wide_msg} [{bar:40.cyan/blue}] [ETA: {eta}]",
        )
        .unwrap(),
    );
    while let Some(task) = include_fgs.next().await {
        let input_fg = input_fg.clone();
        let bar = seg_bar.clone();
        let config = config.clone();
        match task {
            Ok(incs) => {
                bar.inc_length(incs.len() as u64);
                for (name, fp) in incs {
                    let input_fg = input_fg.clone();
                    let bar = bar.clone();
                    let config = config.clone();
                    inc_segments.push(Box::pin(async move {
                        match tokio::task::block_in_place(move || {
                            match_fingerprints(&input_fg, &fp, &config)
                        }) {
                            Ok(segs) => {
                                bar.inc(1);
                                (name, segs)
                            }
                            Err(err) => {
                                log::error!("Matching error (fg_name: {}): {:?}", &name, err);
                                (name, vec![])
                            }
                        }
                    }));
                }
            }
            Err(err) => {
                log::error!("Task failed: load inclusion pack, cause: {:?}", err);
            }
        }
    }
    while let Some(task) = exclude_fgs.next().await {
        let input_fg = input_fg.clone();
        let bar = seg_bar.clone();
        let config = config.clone();
        match task {
            Ok(excs) => {
                bar.inc_length(excs.len() as u64);
                for (name, fp) in excs {
                    let input_fg = input_fg.clone();
                    let bar = bar.clone();
                    let config = config.clone();
                    exc_segments.push(Box::pin(async move {
                        match tokio::task::block_in_place(move || {
                            match_fingerprints(&input_fg, &fp, &config)
                        }) {
                            Ok(segs) => {
                                bar.inc(1);
                                (name, segs)
                            }
                            Err(err) => {
                                log::error!("Matching error (fg_name: {}): {:?}", &name, err);
                                (name, vec![])
                            }
                        }
                    }));
                }
            }
            Err(err) => {
                log::error!("Task failed: load exclusioon pack, cause: {:?}", err);
            }
        }
    }
    PROGRESS
        .get()
        .expect("PROGRESS not initialized!")
        .remove(&pack_bar);

    let inc_segments: Vec<(String, Vec<Segment>)> = inc_segments.collect().await;
    let exc_segments: Vec<(String, Vec<Segment>)> = exc_segments.collect().await;
    PROGRESS
        .get()
        .expect("PROGRESS not initialized!")
        .remove(&seg_bar);

    let mut inc_segments: Vec<(Arc<String>, Segment)> = inc_segments
        .into_iter()
        .map(|(name, segs)| {
            let name = Arc::new(name);
            segs.into_iter()
                .map(|seg| (name.clone(), seg))
                .collect::<Vec<(Arc<String>, Segment)>>()
        })
        .flatten()
        .collect();
    let mut exc_segments: Vec<(Arc<String>, Segment)> = exc_segments
        .into_iter()
        .map(|(name, segs)| {
            let name = Arc::new(name);
            segs.into_iter()
                .map(|seg| (name.clone(), seg))
                .collect::<Vec<(Arc<String>, Segment)>>()
        })
        .flatten()
        .collect();

    inc_segments
        .sort_unstable_by(|(_, seg1), (_, seg2)| seg1.score.partial_cmp(&seg2.score).unwrap());
    exc_segments
        .sort_unstable_by(|(_, seg1), (_, seg2)| seg1.score.partial_cmp(&seg2.score).unwrap());

    Ok((inc_segments, exc_segments))
}

pub fn print_fg_segments<T: Display>(config: &Configuration, segments: &Vec<(T, Segment)>) {
    println!(
        "|- - - - - - - - Name - - - - - - - - - -|- - Start - -|- - E n d - -|- Duration -| Score | (Exlusion list)"
    );
    for (name, seg) in segments {
        println!(
            "{:40} -  {} -  {} - {} - {:.3}",
            name,
            DurationDisplay(seg.start1(config)),
            DurationDisplay(seg.end1(config)),
            DurationDisplay(seg.duration(config)),
            seg.score
        );
    }
}

pub async fn cut_interactive(inc_segs: SegmentMatches, exc_segs: SegmentMatches, input: PathBuf, output: PathBuf) -> Result<(), CutError> {
    let mut cues : Vec<f32> = Vec::new();
    Ok(())
}