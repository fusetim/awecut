#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("pack error")]
    Pack {
        #[from]
        source: PackError,
    },

    #[error("Underlying IO error")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("unknown error")]
    Unknown,
}

#[derive(thiserror::Error, Debug)]
pub enum PackError {
    #[error("Underlying IO error")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Invalid fingerprint error")]
    InvalidFingerprint {
        #[from]
        source: base64::DecodeError,
    },

    #[error("Invalid line error, index: {}", .index)]
    InvalidLine { index: usize },

    #[error("unknown pack error")]
    Unknown,
}