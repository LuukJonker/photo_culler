use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Image not found")]
    PhotoNotFound(#[from] std::io::Error),

    #[error("Image file not supported or corrupted")]
    PhotoCorrupted(#[from] image::ImageError),

    #[error("Directory was not found")]
    DirectoryNotFound,

    #[error("Undefined error")]
    Undefined,
}