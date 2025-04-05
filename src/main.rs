use std::{default, time::Duration};

use symphonia::core::{
    audio::{self, AudioBuffer, Channels, SampleBuffer, Signal, SignalSpec},
    codecs::{Decoder, DecoderOptions},
    conv::IntoSample,
    formats::FormatOptions,
    io::{MediaSource, MediaSourceStream, MediaSourceStreamOptions},
    meta::{self, MetadataOptions},
    probe::{Hint, Probe},
    sample,
};

fn main() {
    println!("awecut - say bye to commercials!");

    // Get the first two arguments (input file, reference directory)
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <input_file> <reference_directory>", args[0]);
        std::process::exit(1);
    }
    let input_file = &args[1];
    let reference_dir = &args[2];

    // Load the reference samples in memory
    let mut references = Vec::new();
    for entry in std::fs::read_dir(reference_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() {
            let file = std::fs::File::open(&path).unwrap();
            let source = MediaSourceStream::new(Box::new(file), Default::default());
            let hint = Hint::new();
            let format_opts = FormatOptions::default();
            let metadata_opts = MetadataOptions::default();
            match symphonia::default::get_probe().format(
                &hint,
                source,
                &format_opts,
                &metadata_opts,
            ) {
                Ok(mut probe) => {
                    let default_track = probe
                        .format
                        .default_track()
                        .expect("Failed to get default track");
                    let default_track_id = default_track.id;

                    // Prepare an audio buffer
                    let spec = SignalSpec::new(
                        default_track
                            .codec_params
                            .sample_rate
                            .expect("sample rate missing"),
                        Channels::FRONT_LEFT,
                    );
                    let mut sample_buffer: SampleBuffer<f32> =
                        SampleBuffer::new(default_track.codec_params.n_frames.unwrap(), spec);

                    // Prepare the decoder
                    let decoder_opts = DecoderOptions::default();
                    let mut decoder = symphonia::default::get_codecs()
                        .make(&default_track.codec_params, &decoder_opts)
                        .expect("Failed to create decoder");

                    // Decode the audio data in 5s segments
                    while let Ok(packet) = probe.format.next_packet() {
                        let track_id = packet.track_id();
                        if track_id == default_track_id {
                            let decoded = decoder.decode(&packet).expect("Failed to decode packet");
                            sample_buffer.copy_planar_ref(decoded);
                        }
                    }

                    // Convert the sample buffer to a vector of f32
                    references.push(sample_buffer);
                }
                Err(e) => {
                    eprintln!("Error loading reference file {}: {}", path.display(), e);
                }
            }
        }
    }

    println!("Loaded {} reference samples", references.len());

    // Load the input segments (5s segments)
    let mut segments = Vec::new();
    {
        let input_file = std::fs::File::open(input_file).unwrap();
        let source = MediaSourceStream::new(Box::new(input_file), Default::default());
        let hint = Hint::new();
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let mut probe = symphonia::default::get_probe()
            .format(&hint, source, &format_opts, &metadata_opts)
            .expect("Failed to load input file");
        let default_track = probe
            .format
            .default_track()
            .expect("Failed to get default track");
        let default_track_id = default_track.id;
        let decoder_opts = DecoderOptions::default();
        let mut decoder = symphonia::default::get_codecs()
            .make(&default_track.codec_params, &decoder_opts)
            .expect("Failed to create decoder");
        let sample_rate = default_track
            .codec_params
            .sample_rate
            .expect("sample rate missing");
        let sample_duration = (sample_rate * 5) as u64;

        // Prepare an audio buffer
        let mut sample_buffer: SampleBuffer<f32> =
            SampleBuffer::new(2*sample_duration, SignalSpec::new(
                sample_rate,
                Channels::FRONT_LEFT,
            ));
        let mut segment_start_ts = None;
        while let Ok(packet) = probe.format.next_packet() {
            let track_id = packet.track_id();
            if track_id == default_track_id {
                if segment_start_ts.is_none() {
                    segment_start_ts = Some(packet.ts());
                } else if (packet.ts() - segment_start_ts.unwrap()) >= sample_duration {
                    // Store the segment
                    segments.push(sample_buffer);
                    sample_buffer = SampleBuffer::new(sample_duration, SignalSpec::new(
                        sample_rate,
                        Channels::FRONT_LEFT,
                    ));
                    segment_start_ts = Some(packet.ts());
                }

                let decoded = decoder.decode(&packet).expect("Failed to decode packet");
                sample_buffer.copy_planar_ref(decoded);    
            }
        }
    }

    println!("Loaded {} segments", segments.len());

    // Compare the segments with the references
    for (i, reference) in references.iter().enumerate() {
        let mut best_score = 0.0;
        let mut best_index = 0;
        for (j, segment) in segments.iter().enumerate() {
            let (index, score) = compare_segments(segment, reference);
            if score > 0.8 {
                println!(
                    "Found a match! Segment {} matches reference {} with score {} at index {} / time {}min",
                    j, i, score, index, (j as f32) * 5.0 / 60.0
                );
            }
            if score > best_score {
                best_score = score;
                best_index = j;
            }
        }
        println!(
            "Best match for reference {} is segment {} with score {}",
            i, best_index, best_score
        );
    }
}

pub fn compare_segments(
    segment: &SampleBuffer<f32>,
    reference: &SampleBuffer<f32>,
) -> (usize, f32) {
    // Compare the segments by cross-correlation

    let segment_len = segment.len();
    let reference_len = reference.len();
    let buffer_len = segment_len + reference_len - 1;
    let mut buffer = vec![0.0; buffer_len];

    // Compute the cross-correlation
    for d in 0..buffer_len {
        for i in 0..segment_len {
            let k = (d as i32) - (i as i32);
            if k >= 0 && (k as usize) < reference_len {
                buffer[d] += segment.samples()[i] * reference.samples()[k as usize];
            }
        }
    }

    // Compute the score as the maximum value in the cross-correlation
    let mut max_index = 0;
    let mut max_value = 0.0;
    for i in 0..buffer_len {
        if buffer[i] > max_value {
            max_value = buffer[i];
            max_index = i;
        }
    }

    (max_index, max_value)
}