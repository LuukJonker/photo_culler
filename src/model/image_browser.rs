use crate::model::image_container::{ImageContainer, ImageSettings};
use slint::{Rgb8Pixel, SharedPixelBuffer};
use std::fs::{self, DirEntry};
use std::path::{Path, PathBuf};

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

        println!(
            "Loaded from {:?}, got {} images",
            folder_path,
            entries.len()
        );

        Self {
            images: entries,
            root_folder: folder_path,
        }
    }

    pub fn preview_at_index(&self, index: usize) -> SharedPixelBuffer<Rgb8Pixel> {
        self.images[index].get_full_preview()
    }

    pub fn settings_at_index(&self, index: usize) -> ImageSettings {
        self.images[index].settings()
    }

    pub fn set_imagesettings(&mut self, index: usize, settings: ImageSettings) {
        self.images[index].set_settings(settings);
    }
}
