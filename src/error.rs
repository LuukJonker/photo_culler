use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error(transparent)]
    ImageDecoding(#[from] image::ImageError),

    #[error("Directory was not found")]
    DirectoryNotFound,

    #[error("Coudn't store state to disk")]
    StateDiskStore,

    #[error("{0}")]
    WithMessage(String),

    #[error("Undefined error")]
    _Undefined,
}
