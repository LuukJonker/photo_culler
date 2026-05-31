use std::{
    error::Error,
    ffi::OsStr,
    fs,
    io::BufReader,
    path::{Path, PathBuf},
    sync::RwLock,
};

use crate::error::ModelError;
use exif::Field;
use image::DynamicImage;
use rsraw::{BIT_DEPTH_8, RawImage};
use serde::{Deserialize, Serialize};
use slint::{Rgb8Pixel, SharedPixelBuffer};

mod image_operations;
mod jpgfromraw;

/// Settings for image adjustment.
#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct ImageSettings {
    /// Brightness offset.
    pub brightness: f32,
    /// Contrast multiplier.
    pub contrast: f32,
}

/// The acceptance state of an image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterState {
    /// Initial state, no action taken.
    Unknown,
    /// Image marked for keeping.
    Accepted,
    /// Image marked for deletion or skipping.
    Rejected,
}

/// Container for all filter-related settings of an image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterSettings {
    /// Whether the photo is accepted, rejected, or unknown.
    pub filter: FilterState,

    /// Whether the image is flagged for a specific use case ("scherm").
    pub scherm: bool,
}

impl Default for FilterSettings {
    /// Returns a default FilterSettings with Unknown state and scherm set to false.
    fn default() -> Self {
        FilterSettings {
            filter: FilterState::Unknown,
            scherm: false,
        }
    }
}

/// Serializable state of an image container, used for persistence.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ImageContainerState {
    /// Original file path of the image.
    pub path: PathBuf,
    /// User-applied image adjustments.
    pub image_settings: ImageSettings,
    /// User-applied filter settings.
    pub filter_settings: FilterSettings,
}

/// Represents a single image, its metadata, and applied settings.
pub struct ImageContainer {
    /// Full path to the image file on disk.
    path: PathBuf,

    /// Metadata extracted from the image's EXIF data.
    metadata: Option<Vec<Field>>,

    /// Cached preview image for faster rendering.
    cached_preview: RwLock<Option<DynamicImage>>,

    /// Applied image adjustments (brightness, contrast).
    settings: ImageSettings,

    /// Current filter state (Accepted/Rejected/Scherm).
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
        let (jpg_bytes, orientation) = match jpgfromraw::process_file(path) {
            Ok(val) => val,
            Err(e) => return Err(ModelError::WithMessage(e.to_string())),
        };

        let img = image::load_from_memory(&jpg_bytes)?;

        // Apply the orientation
        let img = match orientation {
            2 => img.fliph(),
            3 => img.rotate180(),
            4 => img.flipv(),
            5 => img.fliph().rotate270(),
            6 => img.rotate90(), // Camera rotated 90 degrees CW
            7 => img.fliph().rotate90(),
            8 => img.rotate270(), // Camera rotated 90 degrees CCW
            _ => img,             // 1 means normal landscape
        };

        return Ok(img);
    }

    let img = image::ImageReader::open(path)?.decode()?;

    Ok(img)
}

fn load_full_raw_image(path: &Path, use_half_size: bool) -> Result<RawImage, ModelError> {
    let raw_bytes = fs::read(path)?;

    let mut raw_image = match RawImage::open(&raw_bytes) {
        Ok(v) => v,
        Err(e) => return Err(ModelError::WithMessage(e.to_string())),
    };

    raw_image.set_half_size(use_half_size);

    // Unpack it here so the error propagates to your UI gracefully
    if let Err(e) = raw_image.unpack() {
        return Err(ModelError::WithMessage(e.to_string()));
    }

    Ok(raw_image)
}

fn raw_image_to_slint_image(mut raw_img: RawImage) -> SharedPixelBuffer<Rgb8Pixel> {
    let start_total = std::time::Instant::now();
    let processed = raw_img.process::<BIT_DEPTH_8>().unwrap();
    println!("raw_img.process took {:?}", start_total.elapsed());

    let start_copy = std::time::Instant::now();
    let mut shared_buf = SharedPixelBuffer::<Rgb8Pixel>::new(processed.width(), processed.height());
    shared_buf
        .make_mut_bytes()
        .copy_from_slice(processed.iter().as_slice());
    println!("Buffer creation & copy took {:?}", start_copy.elapsed());
    println!(
        "raw_image_to_slint_image total took {:?}",
        start_total.elapsed()
    );

    shared_buf
}

fn dynamic_image_to_slint_image(dyn_img: &DynamicImage) -> SharedPixelBuffer<Rgb8Pixel> {
    // If the underlying image is not rgb, but rgba (like with png), this will fail
    // In that case we can just clone the dynamic image and build the rgb from there, doesn't really matter
    match dyn_img.as_rgb8() {
        Some(rgb) => {
            let width = rgb.width();
            let height = rgb.height();

            let mut pixel_buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
            pixel_buffer.make_mut_bytes().copy_from_slice(rgb.as_raw());

            pixel_buffer
        }
        None => {
            let rgb = dyn_img.to_owned().to_rgb8();
            let width = rgb.width();
            let height = rgb.height();

            let mut pixel_buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
            pixel_buffer.make_mut_bytes().copy_from_slice(rgb.as_raw());

            pixel_buffer
        }
    }
}

impl ImageContainer {
    /// Creates a new ImageContainer for the given path.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            metadata: None,
            cached_preview: RwLock::new(None),
            settings: ImageSettings::default(),
            filter_state: FilterSettings::default(),
        }
    }

    /// Reconstructs an ImageContainer from a stored state.
    pub fn from_state(state: ImageContainerState) -> Self {
        Self {
            path: state.path,
            settings: state.image_settings,
            filter_state: state.filter_settings,
            metadata: None,
            cached_preview: RwLock::new(None),
        }
    }

    /// Returns a clone of the image's file path.
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Generates a preview of the image with settings applied.
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

    /// Loads and processes the full RAW image data.
    pub fn get_raw_image(&self) -> Result<SharedPixelBuffer<Rgb8Pixel>, ModelError> {
        Ok(raw_image_to_slint_image(load_full_raw_image(
            &self.path, false,
        )?))
    }

    /// Generates a small thumbnail of the image.
    pub fn get_thumbnail(&self) -> Result<SharedPixelBuffer<Rgb8Pixel>, ModelError> {
        // Check if we have a cached preview
        if let Ok(guard) = self.cached_preview.read()
            && let Some(cached) = guard.as_ref()
        {
            return Ok(dynamic_image_to_slint_image(&cached.thumbnail(300, 300)));
        }

        let image = load_compressed_image(&self.path)?;

        Ok(dynamic_image_to_slint_image(&image.thumbnail(300, 300)))
    }

    /// Exports the image as a JPEG to the specified directory.
    pub fn export(&self, path: &Path) -> Result<(), ModelError> {
        let image = load_compressed_image(&self.path)?;
        let mut file_path = path.join(self.path.file_stem().ok_or(ModelError::WithMessage(
            "Image path didn't have filename".into(),
        ))?);
        file_path.set_extension("jpg");

        image.save_with_format(file_path, image::ImageFormat::Jpeg)?;

        Ok(())
    }

    /// Returns a clone of the current image adjustments.
    pub fn settings(&self) -> ImageSettings {
        self.settings.clone()
    }

    /// Updates the image adjustments.
    pub fn set_settings(&mut self, settings: ImageSettings) {
        self.settings = settings;
    }

    /// Returns a clone of the current filter settings.
    pub fn filter(&self) -> FilterSettings {
        self.filter_state.clone()
    }

    /// Updates the filter settings.
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

    /// Returns true if metadata has been loaded.
    pub fn is_metadata_loaded(&self) -> bool {
        self.metadata.is_some()
    }

    /// Returns the image's EXIF fields, loading them if necessary.
    pub fn fields(&mut self) -> Vec<Field> {
        if let Some(exif) = self.metadata.as_ref() {
            return exif.clone();
        }

        self.load_metadata().unwrap()
    }

    /// Returns the serializable state of the container.
    pub fn get_state(&self) -> ImageContainerState {
        ImageContainerState {
            path: self.path.clone(),
            image_settings: self.settings(),
            filter_settings: self.filter(),
        }
    }
}
