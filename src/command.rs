use std::path::{Path, PathBuf};
use std::env;
use std::str::FromStr;
use tokio::process::{Command};
use std::io::{Cursor, BufRead, Read};
use tokio::io::{AsyncReadExt, AsyncBufReadExt, BufReader};
use std::process::Stdio;

use crate::error::CommandError;

pub fn get_ffmpeg_home() -> Option<PathBuf> {
    match env::var("FFMPEG_HOME") {
        Ok(val) => Some(PathBuf::from(val)),
        Err(_) => None,
    }
}

pub async fn ffmpeg_duration<T: AsRef<Path>>(ffmpeg_home: Option<T>, input: T) -> Result<f32, CommandError> {
    let ffprobe_bin = ffmpeg_home.map(|p| p.as_ref().join("/bin/ffprobe").display().to_string()).unwrap_or("ffprobe".into());
    let output = Command::new(&ffprobe_bin)
        .arg("-i")
        .arg(input.as_ref().display().to_string())
        .args(&["-show_entries", "format=duration", "-v", "quiet", "-of", "csv=p=0"])
        .output()
        .await?;
    let mut buf : Cursor<&[u8]> = Cursor::new(output.stdout.as_ref());
    let mut dur_str = String::new();
    if let Ok(_) = Read::read_to_string(&mut buf, &mut dur_str) {
        Ok(f32::from_str(&dur_str.trim()).map_err(|err| CommandError::ParsingError { context: format!("ffmpeg_duration - f32 parsing failed: {:?}", err)})?)
    } else {
        Err(CommandError::ParsingError { context: "No line available".into()})
    }
}

pub async fn ffmpeg_keyframes<T: AsRef<Path>>(ffmpeg_home: Option<T>, input: T) -> Result<Vec<f32>, CommandError> {
    let ffprobe_bin = ffmpeg_home.map(|p| p.as_ref().join("/bin/ffprobe").display().to_string()).unwrap_or("/usr/bin/ffprobe".into());
    let mut child = Command::new(&ffprobe_bin)
        .args(&["-hide_banner", "-loglevel", "error", "-skip_frame", "nokey", "-select_streams", "v:0", "-show_entries", "frame=pts_time", "-of", "csv=p=0"])
        .arg(input.as_ref().display().to_string())
        .stdout(Stdio::piped())
        .spawn()?;
        if let Some(stdout) = child.stdout.as_mut() {
        let mut reader = BufReader::new(stdout);
        let mut keyframes : Vec<f32> = Vec::new();
        let mut buf = String::new();
        loop {
            if AsyncBufReadExt::read_line(&mut reader, &mut buf).await? > 0 {
                keyframes.push(f32::from_str(buf.trim()).map_err(|err| CommandError::ParsingError { context: format!("ffmpeg_keyframes - f32 parsing failed: {:?}", err)})?);
                buf.clear();
            } else {
                break;
            }
        }
        Ok(keyframes)
    } else {
        Err(CommandError::ParsingError { context: "stdout not redirected".into() })
    }
}

#[derive(Default, PartialEq, Clone, Debug)]
pub struct StreamRange {
    pub pts_start: Option<f32>,
    pub pts_end: Option<f32>,
    pub skip_frames: Option<&'static str>,
    pub frame_rate: Option<f32>,
}

impl StreamRange {
    pub fn keyframes_betweens(pts_start: Option<f32>, pts_end: Option<f32>) -> Self {
        Self {
            pts_start,
            pts_end,
            skip_frames: Some("nokey"),
            ..Default::default()
        }
    }

    pub fn frames_betweens(pts_start: Option<f32>, pts_end: Option<f32>) -> Self {
        Self {
            pts_start,
            pts_end,
            skip_frames: Some("none"),
            ..Default::default()
        }
    }

    pub fn nsecs_betweens(pts_start: Option<f32>, pts_end: Option<f32>, secs: f32) -> Self {
        Self {
            pts_start,
            pts_end,
            skip_frames: Some("none"),
            frame_rate: Some(1.0/secs),
        }
    }
}

pub async fn ffmpeg_extract_frames<T: AsRef<Path>, R: AsRef<Path>>(ffmpeg_home: Option<T>, input: T, output: R, range: &StreamRange) -> Result<(), CommandError> {
    let ffmpeg_bin = ffmpeg_home.map(|p| p.as_ref().join("/bin/ffmpeg").display().to_string()).unwrap_or("/usr/bin/ffmpeg".into());
    let mut status = Command::new(&ffmpeg_bin)
        .args(if let Some(pts_start) = range.pts_start { vec!["-ss".into(), format!("{:.4}", pts_start)] } else { vec![] })
        .args(if let Some(pts_end) = range.pts_end { vec!["-to".into(), format!("{:.4}", pts_end)] } else { vec![] })
        .args(if let Some(skip_frames) = range.skip_frames { vec!["-skip_frame", skip_frames] } else { vec![] })
        .args(&["-hide_banner", "-loglevel", "error", "-copyts", "-i"])
        .arg(input.as_ref().display().to_string())
        .args(if let Some(frame_rate) = range.frame_rate { vec!["-vf".into(), format!("fps={:.3}", frame_rate)] } else { vec![] })
        .args(&["-enc_time_base", "1/1000", "-vsync", "0", "-f", "image2", "-frame_pts", "1"])
        .arg(format!("{}/%09d.jpg", output.as_ref().display()))
        .stdout(Stdio::piped())
        .status().await?;
    match status.code() {
        None => Err(CommandError::ProcessFailed { context: "ffmpeg_extract_frames - status code is none".into() }),
        Some(0) => Ok(()),
        Some(c) => Err(CommandError::ProcessFailed { context: format!("ffmpeg_extract_frames - status code is {c}") }),
    }
}
