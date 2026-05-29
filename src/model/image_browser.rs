use crate::error::ModelError;
use crate::model::disk_writer::{read_contents, write_contents};
use crate::model::image_container::{ImageContainer, ImageContainerState, ImageSettings};
use mimetype_detector::{MimeType, TEXT_HTML, detect_file};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Manages a collection of images within a directory.
pub struct ImageBrowser {
    /// The root directory containing the images.
    root_folder: PathBuf,

    /// List of image containers for all images found in the directory.
    images: Vec<ImageContainer>,
}

fn get_all_files(folder_path: &Path) -> Result<Vec<ImageContainer>, ModelError> {
    // Read in all the images from the folder
    // Maybe need to pass the error through, so the ui can display the error
    let entries = match fs::read_dir(folder_path) {
        Ok(v) => v,
        Err(_) => return Err(ModelError::DirectoryNotFound),
    };

    Ok(entries
        .filter_map(|res| res.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| match detect_file(p) {
            Ok(mime) => mime.kind().is_image(),
            Err(_) => false,
        })
        .map(ImageContainer::new)
        .collect())
}

/// Template for serializing the collection's state to disk.
#[derive(Serialize, Deserialize)]
struct CollectionTemplate {
    view_state: (),
    images_states: Vec<ImageContainerState>,
}

impl ImageBrowser {
    /// Creates a new ImageBrowser for the specified folder, loading any existing state.
    pub fn new(folder_path: PathBuf) -> Result<Self, ModelError> {
        let mut entries = get_all_files(&folder_path)?;

        // Try and get the settings from the previous session
        if let Ok(temp) = read_contents::<CollectionTemplate>(&folder_path) {
            let mut image_states: HashMap<PathBuf, ImageContainerState> = HashMap::from_iter(
                temp.images_states
                    .iter()
                    .map(|s| (s.path.clone(), s.clone())),
            );
            for entry in entries.iter_mut() {
                if let Some(state) = image_states.remove(&entry.path()) {
                    *entry = ImageContainer::from_state(state);
                }
            }
        }

        Ok(Self {
            images: entries,
            root_folder: folder_path,
        })
    }

    /// Returns the number of images in the collection.
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Returns a reference to the image container at the specified index.
    pub fn at_index(&self, index: usize) -> &ImageContainer {
        &self.images[index]
    }

    /// Returns a mutable reference to the image container at the specified index.
    pub fn mut_at_index(&mut self, index: usize) -> &mut ImageContainer {
        &mut self.images[index]
    }

    /// Persists the current state of all images in the collection to disk.
    pub fn save_to_disk(&self) -> Result<(), ModelError> {
        let mut images_states = Vec::with_capacity(self.len());
        for image in &self.images {
            images_states.push(image.get_state())
        }

        let template = CollectionTemplate {
            view_state: (),
            images_states,
        };

        write_contents(&self.root_folder, template).unwrap();

        Ok(())
    }
}
