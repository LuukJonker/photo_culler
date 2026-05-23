use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, atomic::AtomicBool};

use crate::error::ModelError;
use crate::model::image_container::{
    FilterSettings, FilterState, ImageContainerState, ImageSettings,
};
use rsraw::ProcessedImage;
use slint::{Rgb8Pixel, SharedPixelBuffer};

// Set the next id of a request as static so it can be accessed globally. The
// counter needs to be atomic because jobs can be created on several threads.
static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

fn get_id() -> usize {
    // Dont actually know what the ordering means, but seems aight
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone)]
pub struct Request {
    // Id for the job, exclusive to this job
    id: usize,

    // The actual command
    command: Commands,

    // Priority of the request
    // Not used yet, only when building the priority queue
    priority: Priority,

    // Used to cancel the job
    cancelation_token: Arc<AtomicBool>,
}

impl Request {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn command(&self) -> Commands {
        self.command.clone()
    }

    pub fn priority(&self) -> Priority {
        self.priority
    }

    pub fn cancel(&mut self) {
        self.cancelation_token.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelation_token.load(Ordering::Relaxed)
    }
}

impl From<Commands> for Request {
    fn from(value: Commands) -> Self {
        value.request()
    }
}

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum Priority {
    Low,
    Medium,
    High,

    Critical,
}

impl Commands {
    fn make(self, priority: Priority) -> Request {
        Request {
            id: get_id(),
            command: self,
            priority,
            cancelation_token: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn request(self) -> Request {
        self.make(Priority::Medium)
    }

    pub fn high(self) -> Request {
        self.make(Priority::High)
    }

    pub fn low(self) -> Request {
        self.make(Priority::Low)
    }

    pub fn critical(self) -> Request {
        self.make(Priority::Critical)
    }
}

#[derive(Clone)]
pub enum Commands {
    LoadPhoto(u32),
    LoadRawPhoto(u32),
    LoadDirectory(String),
    LoadThumbnail(u32),

    ExportImage(u32, PathBuf),

    AdjustImagesettings(u32, ImageSettings),
    SetNormalFilter(u32, FilterState),
    SetSchermFilter(u32, bool),

    SaveState,
    KillThread,
}

pub struct Response {
    // The request the response is for
    pub request: Request,

    // The return value of the request
    pub value: Result<ResponseData, ModelError>,
}

pub enum ResponseData {
    LoadedDirectory(u32),
    LoadedPhoto(
        u32,
        SharedPixelBuffer<Rgb8Pixel>,
        crate::model::image_container::ImageContainerState,
    ),
    LoadedPreview(SharedPixelBuffer<Rgb8Pixel>, ImageContainerState),
}

pub enum RenderRequest {
    PreloadRaw(ProcessedImage<16>), // ! Might be nice to have a uid to make sure these apply to the same
    ComputeChange(ImageSettings),
}
