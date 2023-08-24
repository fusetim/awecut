use crate::error::PackError;
use base64::Engine;
use std::io::prelude::*;
use std::io::BufRead;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

#[derive(Default, Eq, Debug, Clone, PartialEq)]
pub struct PackFile {
    pub fingerprint: Vec<(String, Vec<u32>)>,
}

impl PackFile {
    pub fn decode<T: BufRead>(reader: &mut T) -> Result<Self, PackError> {
        let mut fingerprints = Vec::new();
        let mut line = String::new();
        let mut i = 0;
        loop {
            let read = reader.read_line(&mut line)?;
            if read > 0 {
                let parts: Vec<_> = line.trim().split(":").collect();
                if parts.len() != 2 {
                    return Err(PackError::InvalidLine { index: i });
                }
                let b64engine = base64::engine::general_purpose::STANDARD;
                let mut fp: Vec<u32> = Vec::new();
                let fp_bytes = b64engine.decode(parts[1])?;
                for i in 0..(fp_bytes.len() / 4) {
                    fp.push(u32::from_le_bytes(
                        fp_bytes
                            .get((4 * i)..(4 * i + 4))
                            .ok_or(PackError::InvalidU32)?
                            .try_into()
                            .or(Err(PackError::InvalidU32))?,
                    ));
                }
                fingerprints.push((parts[0].to_string(), fp));
            } else {
                break;
            }
            line.clear();
            i += 1;
        }
        Ok(PackFile {
            fingerprint: fingerprints,
        })
    }

    pub async fn decode_async<T: AsyncBufRead + std::marker::Unpin>(
        reader: &mut T,
    ) -> Result<Self, PackError> {
        let mut fingerprints = Vec::new();
        let mut line = String::new();
        let mut i = 0;
        loop {
            let read = reader.read_line(&mut line).await?;
            if read > 0 {
                let parts: Vec<_> = line.trim().split(":").collect();
                if parts.len() != 2 {
                    return Err(PackError::InvalidLine { index: i });
                }
                let b64engine = base64::engine::general_purpose::STANDARD;
                let mut fp: Vec<u32> = Vec::new();
                let fp_bytes = b64engine.decode(parts[1])?;
                for i in 0..(fp_bytes.len() / 4) {
                    fp.push(u32::from_le_bytes(
                        fp_bytes
                            .get((4 * i)..(4 * i + 4))
                            .ok_or(PackError::InvalidU32)?
                            .try_into()
                            .or(Err(PackError::InvalidU32))?,
                    ));
                }
                fingerprints.push((parts[0].to_string(), fp));
            } else {
                break;
            }
            line.clear();
            i += 1;
        }
        Ok(PackFile {
            fingerprint: fingerprints,
        })
    }

    pub fn encode<T: Write>(&self, writer: &mut T) -> Result<(), PackError> {
        for (name, fp) in &self.fingerprint {
            let mut fp_bytes: Vec<u8> = Vec::new();
            for x in fp {
                for byte in x.to_be_bytes() {
                    fp_bytes.push(byte);
                }
            }
            let b64engine = base64::engine::general_purpose::STANDARD;
            let encoded = b64engine.encode(fp_bytes);
            writeln!(writer, "{}:{}", name, encoded)?;
        }
        Ok(())
    }

    pub async fn encode_async<T: AsyncWrite + std::marker::Unpin>(
        &self,
        writer: &mut T,
    ) -> Result<(), PackError> {
        for (name, fp) in &self.fingerprint {
            let mut fp_bytes: Vec<u8> = Vec::new();
            for x in fp {
                for byte in x.to_be_bytes() {
                    fp_bytes.push(byte);
                }
            }
            let b64engine = base64::engine::general_purpose::STANDARD;
            let encoded = b64engine.encode(fp_bytes);
            writer
                .write_all(format!("{}:{}\n", name, encoded).as_bytes())
                .await?;
        }
        Ok(())
    }
}
