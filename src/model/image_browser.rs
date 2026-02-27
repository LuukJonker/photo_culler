use crate::model::image_container::{ImageContainer, ImageSettings};
use slint::{Rgb8Pixel, SharedPixelBuffer};
use std::fs;
use std::path::{Path, PathBuf};
use crate::error::ModelError;

pub struct ImageBrowser {
    // Path to the root folder
    root_folder: PathBuf,

    // All images in the folder
    images: Vec<ImageContainer>,
}

fn get_all_files(folder_path: &Path) -> Vec<ImageContainer> {
    // Read in all the images from the folder
    // Maybe need to pass the error through, so the ui can display the error
    let entries = fs::read_dir(folder_path).unwrap();

    entries
        .filter_map(|res| res.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .map(|p| ImageContainer::new(p))
        .collect()
}

impl ImageBrowser {
    pub fn new(folder_path: PathBuf) -> Self {
        let entries = get_all_files(&folder_path);

        Self {
            images: entries,
            root_folder: folder_path,
        }
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn preview_at_index(&self, index: usize) -> Result<SharedPixelBuffer<Rgb8Pixel>, ModelError> {
        self.images[index].get_full_preview()
    }

    pub fn thumbnail_at_index(&self, index: usize) -> Result<SharedPixelBuffer<Rgb8Pixel>, ModelError> {
        self.images[index].get_thumbnail()
    }

    pub fn settings_at_index(&self, index: usize) -> ImageSettings {
        self.images[index].settings()
    }

    pub fn set_imagesettings(&mut self, index: usize, settings: ImageSettings) {
        self.images[index].set_settings(settings);
    }
}
