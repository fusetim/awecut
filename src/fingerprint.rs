use futures_util::stream::{FuturesUnordered, StreamExt};
use futures_util::Future;
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use rusty_chromaprint::Configuration as FpConf;
use std::boxed::Box;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::fs::read_dir;

use crate::error::{AppError, FingerprintError};
use crate::pack::PackFile;
use crate::PROGRESS;

pub async fn update_fingerprints(inputs: Vec<PathBuf>) -> Result<(), AppError> {
    let fut_files: FuturesUnordered<Pin<Box<dyn Future<Output = Vec<PathBuf>>>>> =
        FuturesUnordered::new();
    let fut_packs: FuturesUnordered<
        Pin<Box<dyn Future<Output = Result<(PathBuf, PackFile), AppError>>>>,
    > = FuturesUnordered::new();
    for input in inputs {
        let pack = input.with_extension("pck");
        match tokio::fs::try_exists(&pack).await {
            Ok(true) => {
                fut_packs.push(Box::pin(async move {
                    let mut reader = tokio::io::BufReader::new(tokio::fs::File::open(&pack).await?);
                    let pack_file = PackFile::decode_async(&mut reader).await?;
                    Ok::<(PathBuf, PackFile), AppError>((pack, pack_file))
                }));
            }
            Ok(false) => {}
            Err(err) => {
                log::error!("unable to verify if the packed fingerprints exist at {}. It will certainly failed to save too. Cause:\n{:?}", pack.display(), err);
            }
        }
        fut_files.push(Box::pin(async move {
            match read_dir(&input).await {
                Ok(mut search) => {
                    let mut files: Vec<PathBuf> = Vec::new();
                    loop {
                        match search.next_entry().await {
                            Ok(Some(entry)) => {
                                let ft = entry.file_type().await;
                                if ft.is_ok() && ft.unwrap().is_file() {
                                    files.push(entry.path());
                                }
                            }
                            Ok(None) => break,
                            Err(err) => {
                                log::error!(
                                    "an error occured while listing directory {}, cause:\n{:?}",
                                    input.display(),
                                    err
                                );
                                break;
                            }
                        }
                    }
                    files
                }
                Err(err) => {
                    log::error!(
                        "failed to list files in directory {}, cause:\n{:?}",
                        input.display(),
                        err
                    );
                    vec![]
                }
            }
        }))
    }
    let files: Vec<Vec<PathBuf>> = fut_files.collect().await;
    let packs: Vec<Result<(PathBuf, PackFile), AppError>> = fut_packs.collect().await;
    let mut packs: Vec<(PathBuf, PackFile)> = packs
        .into_iter()
        .filter_map(|pack| match pack {
            Ok(val) => Some(val),
            Err(err) => {
                log::error!("an error occured while reading a pack-file:\n{:?}", err);
                None
            }
        })
        .collect();
    packs.sort_by(|v1, v2| v1.0.cmp(&v2.0));

    let count = files.iter().fold(0, |acc, dir| acc + dir.len());
    let bar = PROGRESS
        .get()
        .expect("PROGRESS not initialized!")
        .add(ProgressBar::new(count as u64));
    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{pos:>4}/{len:<4}] {wide_msg} [{bar:40.cyan/blue}] [ETA: {eta}]",
        )
        .unwrap(),
    );
    let conf = FpConf::preset_test3();

    for dir in files {
        if dir.len() <= 0 {
            continue;
        }
        if dir[0].parent().is_none()
            || dir[0].parent().unwrap().parent().is_none()
            || dir[0].parent().unwrap().file_name().is_none()
        {
            log::error!(
                "failed to open parent dir at {}, ignoring this file.",
                dir[0].display()
            );
            continue;
        }
        let dir_path = dir[0].parent().unwrap();
        let pack_path = dir_path.with_extension("pck");
        bar.set_message(format!("visiting {}...", dir_path.display()));
        let mut pack = match packs.binary_search_by(|p| p.0.cmp(&pack_path)) {
            Ok(i) => packs.remove(i),
            Err(_) => (pack_path, PackFile::default()),
        };
        for file in dir {
            if let Some(name) = file.file_name() {
                let name = name.to_string_lossy().to_string();
                let index = match pack.1.fingerprint.binary_search_by(|(n, _)| n.cmp(&&name)) {
                    Ok(_) => {
                        bar.update(|state: &mut ProgressState| {
                            state.set_len(state.len().unwrap_or(0).saturating_sub(1))
                        });
                        log::info!("skipping {}, fingerprint found in pack-file.", &name);
                        continue;
                    }
                    Err(i) => i,
                };
                bar.set_message(format!("fingerprinting {}...", &name));
                match calculate_fingerprint(&file, &conf).await {
                    Ok(fg) => {
                        pack.1.fingerprint.insert(index, (name.clone(), fg));
                    }
                    Err(err) => {
                        log::error!(
                            "failed to calculate fingerprint for {}, err:\n{:?}",
                            file.display(),
                            err
                        );
                    }
                }
                log::info!(
                    "  [{pos:>4}/{len:<4}] fingerprint added for {}",
                    &name,
                    pos = bar.position(),
                    len = bar.length().unwrap_or(0)
                );
            } else {
                log::error!(
                    "  [{pos:>4}/{len:<4}] ignoring {} (no filename).",
                    file.display(),
                    pos = bar.position(),
                    len = bar.length().unwrap_or(0)
                );
            }
            bar.inc(1);
        }
        match tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(false)
            .open(&pack.0)
            .await
        {
            Ok(mut pack_file) => match pack.1.encode_async(&mut pack_file).await {
                Ok(()) => {}
                Err(err) => log::error!(
                    "Failed to write pack_file at {}, err:\n{:?}",
                    pack.0.display(),
                    err
                ),
            },
            Err(err) => log::error!(
                "Failed to open pack_file at {}, err:\n{:?}",
                pack.0.display(),
                err
            ),
        }
    }
    bar.abandon_with_message("Completed!");
    Ok(())
}

pub async fn calculate_fingerprint<T: AsRef<Path>>(
    path: T,
    config: &FpConf,
) -> Result<Vec<u32>, FingerprintError> {
    use rusty_chromaprint::Fingerprinter;
    use symphonia::core::audio::{Channels, SampleBuffer};
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let path = path.as_ref();
    let src = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|err| FingerprintError::SymphoniaError {
            source: err,
            context: "unsupported format".into(),
        })?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or(FingerprintError::MissingAudioTrack)?;

    let dec_opts: DecoderOptions = Default::default();

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|err| FingerprintError::SymphoniaError {
            source: err,
            context: "unsupported codec".into(),
        })?;

    let track_id = track.id;

    let mut printer = Fingerprinter::new(&config);
    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or(FingerprintError::MissingSampleRate)?;
    let channels = track
        .codec_params
        .channels
        .unwrap_or_else(|| {
            log::warn!("Cannot find the number of channels in the first audio track, guessing 2.");
            Channels::FRONT_LEFT | Channels::FRONT_RIGHT
        })
        .count() as u32;
    printer.start(sample_rate, channels)?;

    let bar = PROGRESS
        .get()
        .expect("PROGRESS not initialized!")
        .add(ProgressBar::new_spinner());
    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{pos:>6}/? ] {wide_msg} [{elapsed} elapsed]",
        )
        .unwrap(),
    );
    bar.set_message("sampling and fingerprinting...");

    let mut sample_buf = None;
    let mut apac_index = 0;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                apac_index += 1;
                if apac_index % 1000 == 0 {
                    bar.set_position(apac_index);
                }
                if sample_buf.is_none() {
                    let spec = *audio_buf.spec();
                    let duration = audio_buf.capacity() as u64;
                    sample_buf = Some(SampleBuffer::<i16>::new(duration, spec));
                }

                if let Some(buf) = &mut sample_buf {
                    buf.copy_interleaved_ref(audio_buf);
                    printer.consume(buf.samples());
                }
            }
            Err(Error::DecodeError(_)) => (),
            Err(_) => break,
        }
    }

    PROGRESS
        .get()
        .expect("PROGRESS not initialized!")
        .remove(&bar);

    printer.finish();
    Ok(printer.fingerprint().to_vec())
}
