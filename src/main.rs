use std::path::{Path, PathBuf};

use awecut::audio_correlation::{calculate_time_difference, compare_segments};
use awecut::samples::{make_overlap_samples, read_samples};

const SAMPLE_RATE: usize = 48000; // 48kHz
const CHUNK_DURATION: usize = 10; // 10s
const SAMPLE_PER_CHUNK: usize = SAMPLE_RATE * CHUNK_DURATION; // 10s of samples
const AUDIO_CHANNELS: usize = 1; // Mono

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
            match read_samples::<&Path, SAMPLE_PER_CHUNK, SAMPLE_RATE, AUDIO_CHANNELS>(
                path.as_path(),
            ) {
                Ok(sample_iter) => {
                    let raw_chunks = sample_iter.collect::<Vec<_>>();
                    if raw_chunks.len() > 0 {
                        // Concatenate the chunks into a single vector
                        let mut reference = Vec::new();
                        for chunk in raw_chunks.iter() {
                            reference.extend_from_slice(chunk.as_ref());
                        }
                        // Store the reference samples
                        references.push(reference);
                    } else {
                        eprintln!("No samples found in file: {:?}", path);
                    }
                }
                Err(_) => {
                    eprintln!("Error reading samples from file: {:?}", path);
                }
            }
        }
    }

    println!("Loaded {} reference samples", references.len());

    // Load the input segments (5s segments)
    let mut input_chunks = {
        let samples = read_samples::<PathBuf, SAMPLE_PER_CHUNK, SAMPLE_RATE, AUDIO_CHANNELS>(input_file.into())
            .expect("Failed to read input file");
        make_overlap_samples(samples) //-- make stack overflow
        //samples
    };

    // Compare the segments with the references
    for (i, reference) in references.iter().enumerate() {
        let mut best_score = 0.0;
        let mut best_index = 0;

        // Get the duration of the reference in seconds
        let duration_second = reference.len() as f32 / SAMPLE_RATE as f32;
        for (j, segment) in input_chunks.by_ref().enumerate() {
            let (index, score) = compare_segments(segment.as_ref(), reference);
            if score > 1000.0 {
                let time_chunk = calculate_time_difference(index, SAMPLE_RATE as f32, CHUNK_DURATION as f32, duration_second);
                let chunk_start_time = j as f32 * (CHUNK_DURATION as f32) / 2.0; // start time of the chunk in seconds
                println!(
                    "Found a match! Segment {} matches reference {} with score {} at index {} / time {}min",
                    j,
                    i,
                    score,
                    index,
                    (chunk_start_time + time_chunk) / 60.0,
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
