use std::path::{Path, PathBuf};

use awecut::audio_correlation::compare_segments;
use awecut::samples::read_samples;

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
                            reference.extend_from_slice(chunk);
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
    let mut input_chunks =
        read_samples::<PathBuf, SAMPLE_PER_CHUNK, SAMPLE_RATE, AUDIO_CHANNELS>(input_file.into())
            .expect("Failed to read input file");

    // Compare the segments with the references
    for (i, reference) in references.iter().enumerate() {
        let mut best_score = 0.0;
        let mut best_index = 0;
        for (j, segment) in input_chunks.by_ref().enumerate() {
            let (index, score) = compare_segments(&segment, reference);
            if score > 1000.0 {
                println!(
                    "Found a match! Segment {} matches reference {} with score {} at index {} / time {}min",
                    j,
                    i,
                    score,
                    index,
                    j as f32 * (CHUNK_DURATION as f32) / 60.0
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
