use std::{
    io::{ErrorKind, Read as _},
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
