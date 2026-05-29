use std::fmt::Formatter;
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

/// A request to be processed by the model's worker threads.
#[derive(Clone)]
pub struct Request {
    /// Unique identifier for the request.
    id: usize,

    /// The specific command to be executed.
    command: Commands,

    /// Priority level of the request, used for scheduling.
    priority: Priority,

    /// Token used to signal cancellation of the request.
    cancelation_token: Arc<AtomicBool>,
}

impl Request {
    /// Returns the unique ID of the request.
    pub fn id(&self) -> usize {
        self.id
    }

    /// Returns a clone of the command associated with this request.
    pub fn command(&self) -> Commands {
        self.command.clone()
    }

    /// Returns the priority level of the request.
    pub fn priority(&self) -> Priority {
        self.priority
    }

    /// Signals that the request should be cancelled.
    pub fn cancel(&mut self) {
        self.cancelation_token.store(true, Ordering::Relaxed);
    }

    /// Checks if the request has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelation_token.load(Ordering::Relaxed)
    }
}

impl From<Commands> for Request {
    /// Converts a Command into a Request with Medium priority.
    fn from(value: Commands) -> Self {
        value.request()
    }
}

/// Priority levels for job scheduling.
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub enum Priority {
    /// Background tasks.
    Low,
    /// Standard user actions.
    Medium,
    /// Time-sensitive UI updates.
    High,
    /// System-critical operations (e.g., shutdown).
    Critical,
}

impl Commands {
    /// Internal helper to wrap a command in a Request with a given priority.
    fn make(self, priority: Priority) -> Request {
        Request {
            id: get_id(),
            command: self,
            priority,
            cancelation_token: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Wraps the command in a Request with Medium priority.
    pub fn request(self) -> Request {
        self.make(Priority::Medium)
    }

    /// Wraps the command in a Request with High priority.
    pub fn high(self) -> Request {
        self.make(Priority::High)
    }

    /// Wraps the command in a Request with Low priority.
    pub fn low(self) -> Request {
        self.make(Priority::Low)
    }

    /// Wraps the command in a Request with Critical priority.
    pub fn critical(self) -> Request {
        self.make(Priority::Critical)
    }
}

/// Commands that can be sent to the model for execution.
#[derive(Clone)]
pub enum Commands {
    /// Load a photo at the specified index.
    LoadPhoto(u32),
    /// Load the RAW data for the photo at the specified index.
    LoadRawPhoto(u32),
    /// Load a directory from the given path string.
    LoadDirectory(String),
    /// Load a thumbnail for the photo at the specified index.
    LoadThumbnail(u32),

    /// Export the image at the specified index to the given path.
    ExportImage(u32, PathBuf),

    /// Adjust image settings (brightness, contrast, etc.) for an image.
    AdjustImagesettings(u32, ImageSettings),
    /// Set the filter state (Accepted, Rejected, etc.) for an image.
    SetNormalFilter(u32, FilterState),
    /// Set the 'scherm' (screen/flag) filter for an image.
    SetSchermFilter(u32, bool),

    /// Save the current application state.
    SaveState,
    /// Command to stop a worker thread.
    KillThread,
}

/// A response returned by a worker thread after processing a request.
pub struct Response {
    /// The original request this response corresponds to.
    pub request: Request,

    /// The result of the operation, containing either data or an error.
    pub value: Result<ResponseData, ModelError>,
}

/// Data payload of a successful response.
#[derive(Debug)]
pub enum ResponseData {
    /// Directory loaded, containing the number of images found.
    LoadedDirectory(u32),
    /// Photo loaded with its image data and state.
    LoadedPhoto(
        u32,
        SharedPixelBuffer<Rgb8Pixel>,
        crate::model::image_container::ImageContainerState,
    ),
    /// Preview image loaded.
    LoadedPreview(SharedPixelBuffer<Rgb8Pixel>, ImageContainerState),
}

pub enum RenderRequest {
    PreloadRaw(ProcessedImage<16>), // ! Might be nice to have a uid to make sure these apply to the same
    ComputeChange(ImageSettings),
}
