use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Image not found")]
    PhotoNotFound,

    #[error("Directory was not found")]
    DirectoryNotFound
}