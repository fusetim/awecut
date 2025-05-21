use std::path::{Path, PathBuf};

use awecut::samples::read_samples;
use rustfft::{FftPlanner, num_complex::Complex};

const SAMPLE_RATE: usize = 48000; // 48kHz
const CHUNK_DURATION: usize = 10; // 10s
const SAMPLE_PER_CHUNK: usize = SAMPLE_RATE * CHUNK_DURATION; // 10s of samples
const AUDIO_CHANNELS : usize = 1; // Mono

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
            match read_samples::<&Path, SAMPLE_PER_CHUNK, SAMPLE_RATE, AUDIO_CHANNELS>(path.as_path()) {
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
                },
                Err(_) => {
                    eprintln!("Error reading samples from file: {:?}", path);
                },
            }
        }
    }

    println!("Loaded {} reference samples", references.len());

    // Load the input segments (5s segments)
    let mut input_chunks = read_samples::<PathBuf, SAMPLE_PER_CHUNK, SAMPLE_RATE, AUDIO_CHANNELS>(input_file.into())
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

/// Function to give the number which is a next power of two greater than or equal to n
pub fn next_power_of_two(n: usize) -> usize {
    let mut power = 1;
    while power < n {
        power *= 2;
    }
    power
}

/// Zero-pad an input vector for the given length
pub fn zero_pad(input: &[f32], length: usize) -> Vec<Complex<f32>> {
    let input_len = input.len();
    if input_len > length {
        panic!("Input length is greater than the specified length");
    }
    let mut padded = vec![Complex::new(0.0, 0.0); length];
    for i in 0..input_len {
        padded[i] = Complex::new(input[i], 0.0);
    }
    padded
}

pub fn compare_segments(
    segment: &[f32],
    reference: &[f32],
) -> (isize, f32) {
    let mut planner = FftPlanner::<f32>::new();

    // Zero-pad the segment and reference to the next power of two
    let segment_len = segment.len();
    let reference_len = reference.len();
    let cross_correlation_len = segment_len + reference_len - 1;
    let fft_len = next_power_of_two(cross_correlation_len);

    let mut segment_padded = zero_pad(segment, fft_len);
    let mut reference_padded = zero_pad(reference, fft_len);

    // Create FFT plans
    let fft = planner.plan_fft_forward(fft_len);
    let ifft = planner.plan_fft_inverse(fft_len);
    let fft_scratch_len = fft
        .get_inplace_scratch_len()
        .max(ifft.get_inplace_scratch_len());
    let mut fft_scratch = vec![Complex::new(0.0, 0.0); fft_scratch_len];

    // Perform FFT on the segment and reference
    fft.process_with_scratch(&mut segment_padded, &mut fft_scratch);
    fft.process_with_scratch(&mut reference_padded, &mut fft_scratch);

    // Compute the cross-correlation
    let mut cross_correlation = vec![Complex::new(0.0, 0.0); fft_len];
    for i in 0..fft_len {
        cross_correlation[i] = segment_padded[i] * reference_padded[i].conj();
    }

    // Perform IFFT on the cross-correlation
    ifft.process_with_scratch(&mut cross_correlation, &mut fft_scratch);

    // Get only the real part and interesting part of the cross-correlation
    let mut real_part = vec![0.0; cross_correlation_len];
    for i in 0..cross_correlation_len {
        real_part[i] = cross_correlation[i].re / fft_len as f32;
    }

    // Find the maximum value in the cross-correlation
    let mut max_index = 0;
    let mut max_value = 0.0;
    for i in 0..cross_correlation_len {
        if real_part[i] > max_value {
            max_value = real_part[i];
            max_index = i;
        }
    }

    // Normalize the index to the time domain
    let normalized_index = max_index as isize - (segment_len as isize - 1);
    (normalized_index, max_value)
}
