#[derive(Debug, thiserror::Error)]
pub enum MoiError {
    #[error("{0}")]
    Config(String),
    #[error("{context}: {source}")]
    Context {
        context: String,
        source: Box<MoiError>,
    },
    #[error("{path}: {source}")]
    Io {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("{0}")]
    IoMessage(String),
    #[error("command failed with exit code {0}")]
    CommandExit(i32),
    #[error("{path}: invalid TOML: {source}")]
    Toml {
        path: std::path::PathBuf,
        source: toml::de::Error,
    },
}

impl MoiError {
    pub(crate) fn context(self, context: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            source: Box::new(self),
        }
    }
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::CommandExit(code) => *code,
            Self::Context { source, .. } => source.exit_code(),
            _ => 1,
        }
    }

    pub(crate) fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    pub(crate) fn io(
        path: impl Into<std::path::PathBuf>,
        source: std::io::Error,
    ) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
