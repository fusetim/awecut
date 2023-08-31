use futures_util::stream::{FuturesUnordered, StreamExt};
use futures_util::Future;
use indicatif::{ProgressBar, ProgressStyle};
use rusty_chromaprint::{match_fingerprints, Configuration, MatchError, Segment};
use std::boxed::Box;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::fmt::Display;
use std::str::FromStr;
use tokio::io::{AsyncReadExt, AsyncBufReadExt, BufReader};
use std::io::Write;

use crate::duration_util::DurationDisplay;
use crate::error::{CutError, PackError, CommandError, InteractiveError};
use crate::fingerprint::calculate_fingerprint;
use crate::pack::PackFile;
use crate::PROGRESS;
use crate::command::{self, StreamRange};

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
        "|- - - - - - - - Name - - - - - - - - - -|- - Start - -|- - E n d - -|- Duration -| Score |"
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
    let config = Configuration::preset_test3();
    let keyframes : Vec<f32> = command::ffmpeg_keyframes(None, &input).await?;
    let duration = command::ffmpeg_duration(None, &input).await?;
    let temp = tempdir::TempDir::new("awecut")?;

    cues.push(0.0);
    cues.push(duration);

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut buf = String::new();
    loop {
        print!("> ");
        let _ = std::io::stdout().flush();
        let len = reader.read_line(&mut buf).await?;
        if len > 0 {
            let comps : Vec<_> = buf.trim().split(" ").collect();
            if comps.len() == 0 {
                continue;
            }
            match comps[0] {
                "exit" | "quit" => break,
                "help" => println!("help - todo!"),
                "matches" => {
                    println!("Inclusion list:");
                    print_fg_segments(&config, &inc_segs);
                    println!("Exclusion list:");
                    print_fg_segments(&config, &exc_segs);
                },
                "cues" => {
                    println!("Cues:");
                    print_cues(&cues);
                },
                "add" => {
                    if comps.len() > 1 {
                        let time = match parse_time(comps[1]) {
                            Ok(time) => time,
                            Err(err) => {
                                eprintln!("add command failed, due to: {:?}", err);
                                continue;
                            }
                        };
                        match cues.binary_search_by(|c| c.partial_cmp(&time).unwrap()) {
                            Ok(i) => cues.insert(i, time),
                            Err(i) => cues.insert(i, time),
                        }
                        println!("cue added.");
                    } else {
                        eprintln!("missing 1 argument: time (as 00.00s or 00:00:00.00)");
                    }
                },
                "remove" | "rem" | "del" | "delete" => {
                    if comps.len() > 1 {
                        if let Ok(index) = usize::from_str(comps[1]) {
                            let _ = cues.remove(index);
                            println!("cue {} removed!", index);
                        } else {
                            eprintln!("bad argument given, please provide 1 argument of type Number!");
                        }
                    } else {
                        eprintln!("1 argument required.")
                    }
                },
                "inspect" => {
                    if comps.len() > 1 {
                        if let Ok(center) = parse_time(comps[1]) {
                                let range = if comps.len() <= 2 {
                                    let mut start_time = center - (20.0*60.0);
                                    if start_time < 0.0 { start_time = 0.0; }
                                    let mut end_time = center + (20.0*60.0);
                                    if end_time > duration { end_time = duration; }
                                    StreamRange::nsecs_betweens(Some(start_time), Some(end_time), 60.0)
                                } else if comps[2] == "key" || comps[2] == "keyframe" {
                                    let mut start_time = center - 20.0;
                                    if start_time < 0.0 { start_time = 0.0; }
                                    let mut end_time = center + 20.0;
                                    if end_time > duration { end_time = duration; }
                                    StreamRange::keyframes_betweens(Some(start_time), Some(end_time))
                                } else if comps[2] == "frame" || comps[2] == "all" {
                                    let mut start_time = center - 5.0;
                                    if start_time < 0.0 { start_time = 0.0; }
                                    let mut end_time = center + 5.0;
                                    if end_time > duration { end_time = duration; }
                                    StreamRange::frames_betweens(Some(start_time), Some(end_time))
                                } else if let Ok(scale) = parse_time(comps[2]) {
                                    let mut start_time = center - 20.0*scale;
                                    if start_time < 0.0 { start_time = 0.0; }
                                    let mut end_time = center + (20.0*scale);
                                    if end_time > duration { end_time = duration; }
                                    StreamRange::nsecs_betweens(Some(start_time), Some(end_time), scale)
                                } else {
                                    eprintln!("incorrect optional argument 2, provide a valid duration.");
                                    continue;
                                };
                                println!("Clearing directory...");
                                let _ = tokio::fs::remove_dir_all(temp.path()).await;
                                tokio::fs::create_dir_all(temp.path()).await?;
                                println!("Extracting frames to inspect at: {}", temp.path().display());
                                command::ffmpeg_extract_frames(None, &input, temp.path(), &range).await?;
                        } else {
                            eprintln!("incorrect arg 1, please provide a time/duration.");
                        }
                    } else {
                        eprintln!("1 argument required.")
                    }
                }
                _ => eprintln!("invalid command!"),
            }
        } else {
            break;
        }
        buf.clear();
    }

    Ok(())
}

fn print_cues(cues: &Vec<f32>) {
    println!("Number | Timestamp");
    for (i, c) in cues.iter().enumerate() {
        println!(
            "{:^6} | {} ({}s)",
            i,
            DurationDisplay(*c),
            c,
        );
    }
}

fn parse_time(time: &'_ str) -> Result<f32, InteractiveError> {
    let dur : Vec<_> = time.rsplit(":").collect();
    match dur.len() {
        0 => Err(InteractiveError::ParsingError { context: "failed to parse duration: empty".into() }),
        1 => Ok::<f32, InteractiveError>(f32::from_str(dur[0]).map_err(|err| InteractiveError::ParsingError { context: format!("failed to parse duration: {:?}", err) })?),
        2 => {
            let mins = u32::from_str(dur[1]).map_err(|err| InteractiveError::ParsingError { context: format!("failed to parse duration: {:?}", err) })?;
            let secs = f32::from_str(dur[0]).map_err(|err| InteractiveError::ParsingError { context: format!("failed to parse duration: {:?}", err) })?;
            Ok(((mins as f32)*60.0)+secs)
        },
        _ => {
            let hours = u32::from_str(dur[2]).map_err(|err| InteractiveError::ParsingError { context: format!("failed to parse duration: {:?}", err) })?;
            let mins = u32::from_str(dur[1]).map_err(|err| InteractiveError::ParsingError { context: format!("failed to parse duration: {:?}", err) })?;
            let secs = f32::from_str(dur[0]).map_err(|err| InteractiveError::ParsingError { context: format!("failed to parse duration: {:?}", err) })?;
            Ok(((((hours as f32)*60.0) + (mins as f32))*60.0)+secs)
        },
    }
}