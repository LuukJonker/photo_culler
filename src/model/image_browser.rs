use crate::error::ModelError;
use crate::model::disk_writer::{read_contents, write_contents};
use crate::model::image_container::{ImageContainer, ImageContainerState, ImageSettings};
use serde::{Serialize, Deserialize};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ImageBrowser {
    // Path to the root folder
    root_folder: PathBuf,

    // All images in the folder
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
        .map(ImageContainer::new)
        .collect())
}

#[derive(Serialize, Deserialize)]
struct CollectionTemplate {
    view_state: (),
    images_states: Vec<ImageContainerState>,
}

impl ImageBrowser {
    pub fn new(folder_path: PathBuf) -> Result<Self, ModelError> {
        if let Ok(temp) = read_contents::<CollectionTemplate>(&folder_path) {
            return Ok(Self {
                images: temp.images_states.iter().map(|state| ImageContainer::from_state(state.clone())).collect(),
                root_folder: folder_path,
            })
        }

        let entries = get_all_files(&folder_path)?;

        Ok(Self {
            images: entries,
            root_folder: folder_path,
        })
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn at_index(&self, index: usize) -> &ImageContainer {
        &self.images[index]
    }

    pub fn mut_at_index(&mut self, index: usize) -> &mut ImageContainer {
        &mut self.images[index]
    }

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
