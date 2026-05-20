use crate::{constants::APP_NAME, error::ModelError};
use appdirs::user_data_dir;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::HashMap,
    error::Error,
    fs::{self, File},
    path::{Path, PathBuf},
};

pub struct StateWriter {
    state_file_path: PathBuf,
}

#[derive(Deserialize, Serialize, Clone)]
struct MasterFileIndex {
    id: usize,
    relative_path: String,
}

// Serialize and deser support Option<> for an optional key in the json structure
#[derive(Deserialize, Serialize)]
struct MasterFileTemplate {
    id_counter: usize,
    files: HashMap<PathBuf, MasterFileIndex>,
}

impl MasterFileTemplate {
    fn new() -> Self {
        Self {
            id_counter: 0,
            files: HashMap::new(),
        }
    }

    fn _get_id(&mut self) -> usize {
        let id = self.id_counter;
        self.id_counter += 1;
        id
    }

    fn has_collection(&self, path: &Path) -> bool {
        self.files.contains_key(path)
    }

    fn add_collection(&mut self, path: &Path) -> MasterFileIndex {
        match self.files.get(path) {
            Some(val) => return val.clone(),
            None => {
                let id = self._get_id();
                let index = MasterFileIndex {
                    id,
                    relative_path: format!("collection_{}.json", id),
                };
                self.files.insert(path.to_path_buf(), index.clone());
                return index;
            }
        }
    }
}

/// Will get or create the file where the collection state lives
///
/// Returns the path to the json file
fn get_or_create_collection_file(collection_folder: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let mut share_dir = user_data_dir(Some(APP_NAME), None, false)
        .map_err(|_e| ModelError::WithMessage("No user data dir in os".into()))?;
    // Try to create the dir if it doesn't exist already
    fs::create_dir_all(&share_dir)?;

    let mut master_file_path = share_dir.clone();
    master_file_path.push("master_file.json");

    if !fs::exists(&master_file_path)? {
        let file = File::create(&master_file_path)?;
        let mut template = MasterFileTemplate::new();
        let index = template.add_collection(collection_folder);
        serde_json::to_writer(file, &template)?;

        share_dir.push(index.relative_path);
        return Ok(share_dir);
    }

    let file = File::open(&master_file_path)?;
    let mut template: MasterFileTemplate = serde_json::from_reader(&file)?;

    let should_update = !template.has_collection(collection_folder);

    share_dir.push(template.add_collection(collection_folder).relative_path);
    
    if should_update {
        let file = File::create(master_file_path)?;
        serde_json::to_writer(file, &template).unwrap();
    }

    Ok(share_dir)
}

impl StateWriter {
    pub fn new(root_folder: &Path) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            state_file_path: get_or_create_collection_file(root_folder)?,
        })
    }

    pub fn write<T>(&self, contents: T) -> Result<(), Box<dyn Error>>
    where
        T: Serialize,
    {
        let file = File::create(&self.state_file_path)?;

        serde_json::to_writer(file, &contents)?;

        Ok(())
    }

    pub fn read_contents<T>(&self) -> Result<T, Box<dyn Error>>
    where
        T: DeserializeOwned,
    {
        let file = File::open(&self.state_file_path)?;
        Ok(serde_json::from_reader::<File, T>(file)?)
    }
}
