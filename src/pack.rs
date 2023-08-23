use crate::error::PackError;
use std::io::BufRead;
use std::io::prelude::*;
use base64::Engine;
use tokio::io::{AsyncBufReadExt, AsyncBufRead, AsyncWrite, AsyncWriteExt};

pub struct PackFile {
    fingerprint: Vec<(String, Vec<u8>)>
}

impl PackFile {
    pub fn decode<T: BufRead>(reader: &mut T) -> Result<Self, PackError> {
        let mut fingerprints = Vec::new();
        let mut line = String::new();
        let mut i = 0;
        loop {
            let read = reader.read_line(&mut line)?;
            if read > 0 {
                let parts : Vec<_> = line.trim().split(":").collect();
                if parts.len() != 2 {
                    return Err(PackError::InvalidLine{ index: i});
                }
                let b64engine = base64::engine::general_purpose::STANDARD;
                let fp = b64engine.decode(parts[1])?;
                fingerprints.push((parts[0].to_string(), fp));
            } else {
                break;
            }
            line.clear();
            i+=1;
        }
        Ok(PackFile {
            fingerprint: fingerprints,
        })
    }

    pub async fn decode_async<T: AsyncBufRead + std::marker::Unpin>(reader: &mut T) -> Result<Self, PackError> {
        let mut fingerprints = Vec::new();
        let mut line = String::new();
        let mut i = 0;
        loop {
            let read = reader.read_line(&mut line).await?;
            if read > 0 {
                let parts : Vec<_> = line.trim().split(":").collect();
                if parts.len() != 2 {
                    return Err(PackError::InvalidLine{ index: i});
                }
                let b64engine = base64::engine::general_purpose::STANDARD;
                let fp = b64engine.decode(parts[1])?;
                fingerprints.push((parts[0].to_string(), fp));
            } else {
                break;
            }
            line.clear();
            i+=1;
        }
        Ok(PackFile {
            fingerprint: fingerprints,
        })
    }

    pub fn encode<T: Write>(&self, writer: &mut T) -> Result<(), PackError> {
        for (name, fp) in &self.fingerprint {
            let b64engine = base64::engine::general_purpose::STANDARD;
            let encoded = b64engine.encode(fp);
            writeln!(writer, "{}:{}", name, encoded)?;
        }
        Ok(())
    }

    pub async fn encode_async<T: AsyncWrite + std::marker::Unpin>(&self, writer: &mut T) -> Result<(), PackError> {
        for (name, fp) in &self.fingerprint {
            let b64engine = base64::engine::general_purpose::STANDARD;
            let encoded = b64engine.encode(fp);
            writer.write_all(format!("{}:{}", name, encoded).as_bytes()).await?;
        }
        Ok(())
    }
}