use rustfft::{FftPlanner, num_complex::Complex};

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

/// Compare two audio segments using cross-correlation
pub fn compare_segments(segment: &[f32], reference: &[f32]) -> (isize, f32) {
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
