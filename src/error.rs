#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("pack error")]
    Pack {
        #[from]
        source: PackError,
    },

    #[error("fingerprint error")]
    Fingerprint {
        #[from]
        source: FingerprintError,
    },

    #[error("cut error")]
    Cut {
        #[from]
        source: CutError,
    },

    #[error("underlying IO error")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("unexpected error (due to failed assumptions), context: {}", .context)]
    Unexpected { context: String },

    #[error("unknown error")]
    Unknown,
}

#[derive(thiserror::Error, Debug)]
pub enum PackError {
    #[error("underlying IO error")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("invalid fingerprint error")]
    InvalidFingerprint {
        #[from]
        source: base64::DecodeError,
    },

    #[error("invalid u32 error")]
    InvalidU32,

    #[error("invalid line error, index: {}", .index)]
    InvalidLine { index: usize },

    #[error("unknown pack error")]
    Unknown,
}

#[derive(thiserror::Error, Debug)]
pub enum FingerprintError {
    #[error("underlying IO error")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("no audio track found")]
    MissingAudioTrack,

    #[error("no sample rate found")]
    MissingSampleRate,

    #[error("Underlying Symphonia (the Audio decoder) error, context: {}", .context)]
    SymphoniaError {
        source: symphonia::core::errors::Error,
        context: String,
    },

    #[error("underlying chromaprint error")]
    PrinterReset {
        #[from]
        source: rusty_chromaprint::ResetError,
    },

    #[error("unknown pack error")]
    Unknown,
}

#[derive(thiserror::Error, Debug)]
pub enum CutError {
    #[error("underlying IO error")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("fingerprint error")]
    Fingerprint {
        #[from]
        source: FingerprintError,
    },

    #[error("pack error")]
    Pack {
        #[from]
        source: PackError,
    },

    #[error("command (sub-process failed) error")]
    Command {
        #[from]
        source: CommandError,
    },

    #[error("underlying chromaprint error (match system)")]
    PrinterMatch {
        #[from]
        source: rusty_chromaprint::MatchError,
    },

    #[error("unknown pack error")]
    Unknown,
}

#[derive(thiserror::Error, Debug)]
pub enum CommandError {
    #[error("underlying IO error")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Output cannot be parsed in a expected type or value range, context: {}", .context)]
    ParsingError {
        context: String,
    },

    #[error("unknown pack error")]
    Unknown,
}