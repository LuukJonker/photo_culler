use std::sync::{Arc, atomic::AtomicBool};

use slint::{Rgb8Pixel, SharedPixelBuffer};
use crate::model::image_container::ImageSettings;
use crate::error::ModelError;

pub struct Request {
    // Id for the job, exclusive to this job
    id: usize,

    // The actual command
    pub command: Commands,

    // Priority of the request
    // Not used yet, only when building the priority queue
    priority: Priority,

    // Used to cancel the job
    cancelation_token: Arc<AtomicBool>,
}

pub enum Priority {
    Low,
    Medium,
    High,
}

impl Commands {
    pub fn request(self) -> Request {
        Request { id: 0, command: self, priority: Priority::Medium, cancelation_token: Arc::new(AtomicBool::new(false)) }
    }
}

pub enum Commands {
    LoadPhoto(u32),
    LoadDirectory(String),
    LoadFilmstrip,

    AdjustImagesettings(u32, ImageSettings),
}

pub struct Response {
    // The request the response is for
    pub request: Request,

    // The return value of the request
    pub value: Result<ResponseData, ModelError>,
}

pub enum ResponseData {
    LoadedDirectory,
    LoadedPhoto(SharedPixelBuffer<Rgb8Pixel>, ImageSettings),
    SettingsForPhoto(u32, ImageSettings),
}
