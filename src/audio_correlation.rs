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

/// Calculate the time difference between two audio segments
/// given their maximum cross-correlation index
/// Returns the time difference in seconds
/// 
/// TODO: Does not take into account the Zero-padding added to the segments
/// 
/// # Arguments
/// 
/// * `index` - The index of the maximum cross-correlation
/// * `sample_rate` - The sample rate of the audio segments
/// * `duration_first` - The duration of the first segment in seconds
/// * `duration_second` - The duration of the second segment in seconds
/// 
/// # Returns
/// 
/// * `f32` - The time difference in seconds
///           Positive value indicates the first segment is ahead of the second
///           Negative value indicates the first segment is behind the second  
pub fn calculate_time_difference(index: isize, sample_rate: f32, duration_first: f32, duration_second: f32) -> f32 {
    let time_diff = index as f32 / sample_rate;
    let time_diff_seconds = (duration_first - duration_second) / 2.0 - time_diff;
    time_diff_seconds
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_power_of_two() {
        assert_eq!(next_power_of_two(5), 8);
        assert_eq!(next_power_of_two(8), 8);
        assert_eq!(next_power_of_two(9), 16);
        assert_eq!(next_power_of_two(15), 16);
    }

    #[test]
    fn test_zero_pad() {
        let input = vec![1.0, 2.0, 3.0];
        let padded = zero_pad(&input, 6);
        assert_eq!(padded.len(), 6);
        assert_eq!(padded[0], Complex::new(1.0, 0.0));
        assert_eq!(padded[1], Complex::new(2.0, 0.0));
        assert_eq!(padded[2], Complex::new(3.0, 0.0));
        assert_eq!(padded[3], Complex::new(0.0, 0.0));
        assert_eq!(padded[4], Complex::new(0.0, 0.0));
        assert_eq!(padded[5], Complex::new(0.0, 0.0));
    }

    #[test]
    fn test_calculate_time_difference() {
        let first_segment = [1.0, 2.0, 3.0, 4.0, 7.0, 0.0, 0.0, 0.0, 9.0, 10.0, 3.0, 2.0];
        let second_segment = [9.0, 10.0, 3.0, 2.0, 1.0, 8.0, 18.0, 4.0, 63.0, 52.0, 53.0, 0.0, 0.0, 10.0];
        let sample_rate = 2.0;

        let (index, _) = compare_segments(&first_segment, &second_segment);
        let duration_first = first_segment.len() as f32 / sample_rate;
        let duration_second = second_segment.len() as f32 / sample_rate;
        let time_diff = calculate_time_difference(index, sample_rate, duration_first, duration_second);
        assert_eq!(time_diff, 5.0);
    }

    #[test]
    fn test_calculate_time_difference_2() {
        let first_segment = [1.0, 2.0, 3.0, 4.0, 7.0, 0.0, 0.0, 0.0, 9.0, 10.0, 3.0, 2.0];
        let second_segment = [9.0, 10.0, 3.0, 2.0, 1.0, 8.0, 18.0, 4.0, 63.0, 52.0, 53.0, 0.0, 0.0, 10.0];
        let sample_rate = 4.0;

        let (index, _) = compare_segments(&first_segment, &second_segment);
        let duration_first = first_segment.len() as f32 / sample_rate;
        let duration_second = second_segment.len() as f32 / sample_rate;
        let time_diff = calculate_time_difference(index, sample_rate, duration_first, duration_second);
        assert_eq!(time_diff, 2.5);
    }
}