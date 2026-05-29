use thiserror::Error;

/// Errors that can occur within the Model's operations.
#[derive(Error, Debug)]
pub enum ModelError {
    /// Errors related to file system operations.
    #[error(transparent)]
    IO(#[from] std::io::Error),

    /// Errors related to image decoding.
    #[error(transparent)]
    ImageDecoding(#[from] image::ImageError),

    /// The specified directory could not be found.
    #[error("Directory was not found")]
    DirectoryNotFound,

    /// Failure when attempting to save application state to disk.
    #[error("Coudn't store state to disk")]
    StateDiskStore,

    /// A generic error with a custom message.
    #[error("{0}")]
    WithMessage(String),

    /// An undefined or unexpected error.
    #[error("Undefined error")]
    _Undefined,
}
