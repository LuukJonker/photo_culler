use std::{
    cell::RefCell,
    error::Error,
    io::BufReader,
    path::{Path, PathBuf},
    sync::RwLock,
};

use crate::error::ModelError;
use exif::{Exif, Field};
use image::DynamicImage;
use rawloader::RawImage;
use slint::{Rgb8Pixel, SharedPixelBuffer};

mod jpgfromraw;

enum ImageType {
    Raw,
    Compressed,
}

#[derive(Default, Clone, Debug)]
pub struct ImageSettings {
    pub brightness: f32,
    pub contrast: f32,
}

#[derive(Clone)]
pub enum FilterState {
    Unknown,
    Accepted,
    Rejected,
}

#[derive(Clone)]
pub struct FilterSettings {
    // If the photo is accepted or rejected
    pub filter: FilterState,

    // If it should be saved for the "scherm" in the kelder
    pub scherm: bool,
}

impl Default for FilterSettings {
    fn default() -> Self {
        FilterSettings {
            filter: FilterState::Unknown,
            scherm: false,
        }
    }
}

pub struct ImageContainer {
    // Full path to the image location of the drive
    path: PathBuf,

    // The metadata stored in an exif struct
    metadata: Option<Vec<Field>>,

    // Cached preview
    cached_preview: RwLock<Option<DynamicImage>>,

    // The settings of the image
    settings: ImageSettings,

    // The state of the filter
    filter_state: FilterSettings,
}

fn is_raw(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext {
        Some(ext) => [
            "arw", "cr2", "crw", "dng", "erf", "kdc", "mef", "mrw", "nef", "nrw", "orf", "pef",
            "raf", "raw", "rw2", "rwl", "sr2", "srf", "srw", "x3f",
        ]
        .iter()
        .any(|known| *known == ext),
        None => false,
    }
}

fn load_compressed_image(path: &Path) -> Result<image::DynamicImage, ModelError> {
    if is_raw(path) {
        let jpg_bytes = match jpgfromraw::process_file(path) {
            Ok(val) => val,
            Err(e) => return Err(ModelError::WithMessage(e.to_string())),
        };

        let img = image::load_from_memory(&jpg_bytes)?;
        return Ok(img);
    }

    let img = image::ImageReader::open(path)?.decode()?;

    Ok(img)
}

fn load_full_raw_image(path: &Path) -> Result<RawImage, ModelError> {
    match rawloader::decode_file(path) {
        Ok(val) => Ok(val),
        Err(e) => Err(ModelError::WithMessage(e.to_string())),
    }
}

fn dynamic_image_to_slint_image(dyn_img: &DynamicImage) -> SharedPixelBuffer<Rgb8Pixel> {
    let total_start = std::time::Instant::now();

    let rgb_start = std::time::Instant::now();
    let rgb = dyn_img.as_rgb8().unwrap();
    let rgb_time = rgb_start.elapsed();

    let width = rgb.width();
    let height = rgb.height();

    let mut pixel_buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    pixel_buffer.make_mut_bytes().copy_from_slice(rgb.as_raw());

    let total_time = total_start.elapsed();
    println!(
        "Converting to rgb8 took {:?}, total time: {:?}",
        rgb_time, total_time
    );

    pixel_buffer
}

impl ImageContainer {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            metadata: None,
            cached_preview: RwLock::new(None),
            settings: ImageSettings::default(),
            filter_state: FilterSettings::default(),
        }
    }

    pub fn get_full_preview(&self) -> Result<SharedPixelBuffer<Rgb8Pixel>, ModelError> {
        // Check if we have a cached preview
        if let Ok(guard) = self.cached_preview.read() {
            if let Some(cached) = guard.as_ref() {
                let applied_image = cached
                    .brighten(self.settings().brightness as i32)
                    .adjust_contrast(self.settings.contrast);
                return Ok(dynamic_image_to_slint_image(&applied_image));
            }
        }
        println!("Loading with settings: {:?}", self.settings);
        let image = load_compressed_image(&self.path)?;
        let applied_image = image
            .brighten(self.settings().brightness as i32)
            .adjust_contrast(self.settings.contrast);
        let shared_image = dynamic_image_to_slint_image(&applied_image);

        // Cache the preview. Also deal with the case that the data was poisoned by another thread.
        match self.cached_preview.write() {
            Ok(mut lock) => *lock = Some(image),
            Err(mut p_err) => {
                **p_err.get_mut() = Some(image);
                self.cached_preview.clear_poison();
            }
        }

        Ok(shared_image)
    }

    pub fn get_thumbnail(&self) -> Result<SharedPixelBuffer<Rgb8Pixel>, ModelError> {
        // Check if we have a cached preview
        if let Ok(guard) = self.cached_preview.read() {
            if let Some(cached) = guard.as_ref() {
                return Ok(dynamic_image_to_slint_image(&cached.thumbnail(300, 300)));
            }
        }

        let image = load_compressed_image(&self.path)?;

        Ok(dynamic_image_to_slint_image(&image.thumbnail(300, 300)))
    }

    // Image settings
    pub fn settings(&self) -> ImageSettings {
        self.settings.clone()
    }

    pub fn set_settings(&mut self, settings: ImageSettings) {
        self.settings = settings;
    }

    // Filter settings
    pub fn filter(&self) -> FilterSettings {
        self.filter_state.clone()
    }

    pub fn set_filter(&mut self, filter: FilterSettings) {
        self.filter_state = filter;
    }

    // Metadata methods
    fn load_metadata(&mut self) -> Result<Vec<Field>, Box<dyn Error>> {
        let file = std::fs::File::open(&self.path)?;
        let mut bufreader = BufReader::new(file);
        let exifreader = exif::Reader::new();
        let exif = exifreader.read_from_container(&mut bufreader)?;

        self.metadata = Some(exif.fields().cloned().collect());

        Ok(exif.fields().cloned().collect())
    }

    pub fn is_metadata_loaded(&self) -> bool {
        self.metadata.is_some()
    }

    pub fn fields(&mut self) -> Vec<Field> {
        if let Some(exif) = self.metadata.as_ref() {
            return exif.clone();
        }

        self.load_metadata().unwrap()
    }
}
