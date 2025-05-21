use std::{
    io::{BufReader, ErrorKind, Read as _},
    path::Path,
};

/// An iterator that reads audio samples from an ffmpeg process output.
/// 
/// This iterator reads audio samples in chunks of size `N` and converts them
/// from raw bytes to `f32` values. The samples are read from the standard output
/// of an ffmpeg process, which is expected to be in the `f32le` format.
pub struct FfmpegSamplesIter<const N: usize> {
    ffmpeg_out: BufReader<std::process::ChildStdout>,
    buffer: Vec<u8>,
}

/// Creates a new instance of `FfmpegSamplesIter` with a specified buffer size.
///
/// # Parameters
/// - `ffmpeg_out`: A `std::process::ChildStdout` representing the standard output
///   of an ffmpeg process. This will be wrapped in a `BufReader` for efficient reading.
///
/// # Returns
/// A new `FfmpegSamplesIter` instance with an internal buffer of size `4 * N`.
///
/// # Type Parameters
/// - `N`: The size of the buffer divided by 4. The total buffer size will be `4 * N`.
impl<const N: usize> FfmpegSamplesIter<N> {
    pub fn new(ffmpeg_out: std::process::ChildStdout) -> Self {
        let buffer = vec![0; 4 * N];
        let ffmpeg_out = BufReader::new(ffmpeg_out);
        Self { ffmpeg_out, buffer }
    }
}

impl<const N: usize> Iterator for FfmpegSamplesIter<N> {
    type Item = Box<[f32; N]>;

    fn next(&mut self) -> Option<Self::Item> {
        // Read the samples, and try to fill the buffer
        // If EOF is reached before the buffer is filled,
        // 0-fill the rest of the buffer
        let mut buffer_cursor = 0;
        while buffer_cursor < self.buffer.len() {
            match self.ffmpeg_out.read(&mut self.buffer[buffer_cursor..]) {
                Ok(0) => {
                    if buffer_cursor == 0 {
                        return None; // EOF reached, no more data
                    }
                    // EOF reached, fill the rest of the buffer with 0s
                    for i in buffer_cursor..self.buffer.len() {
                        self.buffer[i] = 0;
                    }
                    break; // Exit the loop
                } // EOF
                Ok(n) => buffer_cursor += n,
                Err(err) => {
                    if err.kind() == ErrorKind::Interrupted {
                        continue; // Retry on interruption
                    } else {
                        return None; // Error reading
                    }
                }
            }
        }

        // Convert the bytes to f32 samples
        let mut samples = Box::new([0.0; N]);
        for i in 0..N {
            let start = i * std::mem::size_of::<f32>();
            let end = start + std::mem::size_of::<f32>();
            samples[i] = f32::from_le_bytes(self.buffer[start..end].try_into().ok()?);
        }

        Some(samples)
    }
}

/// Read an audio file and returns an iterator over sample chunks.
/// The iterator yields chunks of samples, each containing a specified number of samples.
/// 
/// # Arguments
/// 
/// * `path` - The path to the audio file.
/// 
/// # Gerneric Parameters
/// * `N` - The number of samples in each chunk.
///           Note: You must ensure that the number of samples is a multiple of the number of channels.
/// * `S` - The sampling frequency in Hz.
/// * `C` - The number of channels to preserve.
/// 
pub fn read_samples<P: AsRef<Path>, const N: usize, const S: usize, const C: usize>(
    path: P,
) -> Result<FfmpegSamplesIter<N>, ()> {
    // Check if the file exists
    if !path.as_ref().exists() {
        return Err(());
    }

    // Use ffmpeg to read the samples
    let mut cmd = std::process::Command::new("ffmpeg");
    cmd.arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-i")
        .arg(path.as_ref())
        .arg("-vn")
        .arg("-dn")
        .arg("-f")
        .arg("f32le")
        .arg("-ar")
        .arg(S.to_string())
        .arg("-ac")
        .arg(C.to_string())
        .arg("-c:a")
        .arg("pcm_f32le")
        .arg("pipe:1");

    // Setup the output pipe to collect the samples
    let stdout = cmd
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| ())?
        .stdout
        .ok_or(())?;

    // Create an iterator that reads the samples in chunks
    let iter = FfmpegSamplesIter::<N>::new(stdout);

    Ok(iter)
}

/// An iterator that provides overlapping samples from an underlying sample iterator.
/// 
/// This iterator is designed to work with audio samples, where each sample is represented
/// as a fixed-size array of `f32` values. The iterator yields chunks of samples, with
/// overlapping regions between consecutive chunks. The overlap size is half the size of
/// the sample size.
/// 
/// # Type Parameters
/// 
/// * `N`: The size of the sample array. This should be an even number.
/// 
/// # Panics
/// 
/// This iterator will panic if `N` is not an even number.
pub struct OverlappingSamplesIter<'a, const N: usize> {
    buffer: Vec<f32>,
    sample_iter: Box<dyn Iterator<Item = Box<[f32; N]>> + 'a>,
    is_first: bool,
    is_overlap: bool,
}

impl<'a, const N: usize> OverlappingSamplesIter<'a, N> {
    pub fn new(sample_iter: Box<dyn Iterator<Item = Box<[f32; N]>> + 'a>) -> Self {
        let half = N / 2;
        let buffer = vec![0.0; 3 * half];
        let is_first = true;
        let is_overlap = false;

        Self {
            buffer,
            sample_iter,
            is_first,
            is_overlap,
        }
    }
}

impl<'a, const N: usize> Iterator for OverlappingSamplesIter<'a, N> {
    type Item = Box<[f32; N]>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut tmp = Box::new([0.0; N]);
        if self.is_first {
            // Get the first chunk
            match self.sample_iter.next() {
                Some(chunk) => {
                    // Copy the chunk to the buffer (at part 1)
                    let half = N / 2;
                    self.buffer[0..half].copy_from_slice(&chunk[half..]);
                    self.is_first = false;
                    self.is_overlap = true;
                    Some(chunk)
                }
                None => None,
            }
        } else {
            // If in the overlap region, get the next chunk
            if self.is_overlap {
                match self.sample_iter.next() {
                    Some(chunk) => {
                        // Copy the chunk to the buffer (at part 2 and 3)
                        let half = N / 2;
                        self.buffer[half..].copy_from_slice(chunk.as_ref());
                        tmp.copy_from_slice(&self.buffer[0..N]);
                        self.is_overlap = false;
                        Some(tmp)
                    }
                    None => None,
                }
            } else {
                // If not in the overlap region,
                // the next chunk is actually part 2 and 3 of the buffer
                // We just need to return that and move the buffer by N
                let half = N / 2;
                tmp.copy_from_slice(&self.buffer[half..]);
                self.buffer.copy_within(N..N + half, 0);
                self.is_overlap = true;
                Some(tmp)
            }
        }
    }
}

/// Creates an iterator that provides overlapping samples from an underlying sample iterator.
/// 
/// This function takes an iterator of audio samples and returns an `OverlappingSamplesIter`
/// that yields chunks of samples with overlapping regions.
/// 
/// # Type Parameters
/// 
/// * `N`: The size of the sample array. This should be an even number.
pub fn make_overlap_samples<'a, const N: usize>(
    sample_iter: impl 'a + Iterator<Item = Box<[f32; N]>>,
) -> OverlappingSamplesIter<'a, N> {
    OverlappingSamplesIter::new(Box::new(sample_iter))
}

/// Creates an iterator that provides overlapping samples from an underlying sample iterator.
/// 
/// See [`make_overlap_samples`] for more details. 
/// 
/// This variant is there to ease testing and usage with iterators that yield
/// `[f32; N]` directly instead of `Box<[f32; N]>`.
pub fn make_overlap_samples_unboxed<'a, const N: usize>(
    sample_iter: impl 'a + Iterator<Item = [f32; N]>,
) -> OverlappingSamplesIter<'a, N> {
    let boxed_iter = sample_iter.map(|chunk| Box::new(chunk));
    OverlappingSamplesIter::new(Box::new(boxed_iter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_overlap_samples_empty() {
        let empty_iter: Vec<[f32; 2]> = vec![];
        let mut overlap_iter = make_overlap_samples_unboxed::<2>(empty_iter.into_iter());
        assert_eq!(overlap_iter.next(), None);
    }

    #[test]
    fn test_make_overlap_samples_once() {
        let single_chunk = [[1.0, 2.0]];
        let mut overlap_iter = make_overlap_samples_unboxed::<2>(single_chunk.into_iter());
        assert_eq!(overlap_iter.next(), Some(Box::new([1.0, 2.0])));
        assert_eq!(overlap_iter.next(), None);
    }

    #[test]
    fn test_make_overlap_samples_twice() {
        let chunks = [[1.0, 2.0], [3.0, 4.0]];
        let mut overlap_iter = make_overlap_samples_unboxed::<2>(chunks.into_iter());
        assert_eq!(overlap_iter.next(), Some(Box::new([1.0, 2.0])));
        assert_eq!(overlap_iter.next(), Some(Box::new([2.0, 3.0])));
        assert_eq!(overlap_iter.next(), Some(Box::new([3.0, 4.0])));
        assert_eq!(overlap_iter.next(), None);
    }

    #[test]
    fn test_make_overlap_samples_thrice() {
        let chunks = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]];
        let mut overlap_iter = make_overlap_samples_unboxed::<2>(chunks.into_iter());
        assert_eq!(overlap_iter.next(), Some(Box::new([1.0, 2.0])));
        assert_eq!(overlap_iter.next(), Some(Box::new([2.0, 3.0])));
        assert_eq!(overlap_iter.next(), Some(Box::new([3.0, 4.0])));
        assert_eq!(overlap_iter.next(), Some(Box::new([4.0, 5.0])));
        assert_eq!(overlap_iter.next(), Some(Box::new([5.0, 6.0])));
        assert_eq!(overlap_iter.next(), None);
    }
}
