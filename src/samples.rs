use std::{
    io::{BufReader, ErrorKind, Read as _},
    path::Path,
};

/**
 * Read an audio file and returns an iterator over sample chunks.
 * The iterator yields chunks of samples, each containing a specified number of samples.
 *
 * # Arguments
 *
 * * * `path` - The path to the audio file.
 *
 * # Gerneric Parameters
 *
 * * * `N` - The number of samples in each chunk.
 *           Note: You must ensure that the number of samples is a multiple of the number of channels.
 * * * `S` - The sampling frequency in Hz.
 * * * `C` - The number of channels to preserve.
 */
pub fn read_samples<P: AsRef<Path>, const N: usize, const S: usize, const C: usize>(
    path: P,
) -> Result<impl Iterator<Item = [f32; N]>, ()> {
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

    // Create a buffer to read the samples into
    let mut buffer = vec![0; N * std::mem::size_of::<f32>()];
    let mut reader = std::io::BufReader::new(stdout);

    // Create an iterator that reads the samples in chunks
    let iter = std::iter::from_fn(move || {
        // Read the samples, and try to fill the buffer
        // If EOF is reached before the buffer is filled,
        // 0-fill the rest of the buffer
        let mut buffer_cursor = 0;
        while buffer_cursor < buffer.len() {
            match reader.read(&mut buffer[buffer_cursor..]) {
                Ok(0) => {
                    if buffer_cursor == 0 {
                        return None; // EOF reached, no more data
                    }
                    // EOF reached, fill the rest of the buffer with 0s
                    for i in buffer_cursor..buffer.len() {
                        buffer[i] = 0;
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
        let samples: [f32; N] = {
            let mut samples = [0.0; N];
            for i in 0..N {
                let start = i * std::mem::size_of::<f32>();
                let end = start + std::mem::size_of::<f32>();
                samples[i] = f32::from_le_bytes(buffer[start..end].try_into().ok()?);
            }
            samples
        };

        Some(samples)
    });

    Ok(iter)
}

/**
 * Make a 50% overlap sample iterator
 * 
 * N must be even - assert!(N % 2 == 0)
 */
pub fn make_overlap_samples<const N: usize>(
    mut sample_iter: impl Iterator<Item = [f32; N]>,
) -> impl Iterator<Item = [f32; N]> {
    assert_eq!(N % 2, 0, "N must be even");
    let half = N / 2;
    let mut buffer = vec![0.0; 3*half];
    let mut tmp = [0.0; N];
    let mut is_first = true;
    let mut is_overlap = false; // Flag to indicate if we are in the overlap region

    std::iter::from_fn(move || {
        if is_first {
            // Get the first chunk
            match sample_iter.next() {
                Some(chunk) => {
                    // Copy the chunk to the buffer (at part 1)
                    buffer[0..half].copy_from_slice(&chunk[half..]);
                    is_first = false;
                    is_overlap = true;
                    Some(chunk)
                }
                None => None,
            }
        } else {
            // If in the overlap region, get the next chunk
            if is_overlap {
                match sample_iter.next() {
                    Some(chunk) => {
                        // Copy the chunk to the buffer (at part 2 and 3)
                        buffer[half..].copy_from_slice(&chunk);
                        tmp.copy_from_slice(&buffer[0..N]);
                        is_overlap = false;
                        Some(tmp)
                    }
                    None => None,
                }
            } else {
                // If not in the overlap region, 
                // the next chunk is actually part 2 and 3 of the buffer
                // We just need to return that and move the buffer by N
                tmp.copy_from_slice(&buffer[half..]);
                buffer.copy_within(N..N+half, 0);
                is_overlap = true;
                Some(tmp)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_overlap_samples_empty() {
        let empty_iter: Vec<[f32; 2]> = vec![];
        let overlap_iter = make_overlap_samples::<2>(empty_iter.into_iter());
        assert_eq!(overlap_iter.count(), 0);
    }

    #[test]
    fn test_make_overlap_samples_once() {
        let single_chunk = [[1.0, 2.0]];
        let mut overlap_iter = make_overlap_samples::<2>(single_chunk.into_iter());
        assert_eq!(overlap_iter.next(), Some([1.0, 2.0]));
        assert_eq!(overlap_iter.next(), None);
    }

    #[test]
    fn test_make_overlap_samples_twice() {
        let chunks = [[1.0, 2.0], [3.0, 4.0]];
        let mut overlap_iter = make_overlap_samples::<2>(chunks.into_iter());
        assert_eq!(overlap_iter.next(), Some([1.0, 2.0]));
        assert_eq!(overlap_iter.next(), Some([2.0, 3.0]));
        assert_eq!(overlap_iter.next(), Some([3.0, 4.0]));
        assert_eq!(overlap_iter.next(), None);
    }

    #[test]
    fn test_make_overlap_samples_thrice() {
        let chunks = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0]];
        let mut overlap_iter = make_overlap_samples::<2>(chunks.into_iter());
        assert_eq!(overlap_iter.next(), Some([1.0, 2.0]));
        assert_eq!(overlap_iter.next(), Some([2.0, 3.0]));
        assert_eq!(overlap_iter.next(), Some([3.0, 4.0]));
        assert_eq!(overlap_iter.next(), Some([4.0, 5.0]));
        assert_eq!(overlap_iter.next(), Some([5.0, 6.0]));
        assert_eq!(overlap_iter.next(), None);
    }

}