use std::{
    cell::RefCell,
    error::Error,
    io::BufReader,
    path::{Path, PathBuf},
};

use crate::error::ModelError;
use exif::{Exif, Field};
use image::DynamicImage;
use rawloader::RawImage;
use slint::{Rgb8Pixel, SharedPixelBuffer};

enum ImageType {
    Raw,
    Compressed,
}

#[derive(Default, Clone)]
pub struct ImageSettings {
    pub brightness: f32,
    pub contrast: f32,
}

pub struct ImageContainer {
    // Full path to the image location of the drive
    path: PathBuf,

    // The metadata stored in an exif struct
    metadata: Option<Exif>,

    // Cached preview
    cached_preview: RefCell<Option<SharedPixelBuffer<Rgb8Pixel>>>,

    //
    settings: ImageSettings,
}

fn load_compressed_image(path: &Path) -> Result<image::DynamicImage, ModelError> {
    let img = image::ImageReader::open(path)?.decode()?;

    Ok(img)
}

fn load_full_raw_image(path: &Path) -> Result<RawImage, Box<dyn Error>> {
    Ok(rawloader::decode_file(path)?)
}

fn load_raw_preview(path: &Path) -> Result<image::DynamicImage, Box<dyn Error>> {
    !todo!()
}

fn dynamic_image_to_slint_image(dyn_img: DynamicImage) -> SharedPixelBuffer<Rgb8Pixel> {
    let rgb = dyn_img.into_rgb8();
    let width = rgb.width();
    let height = rgb.height();

    let mut pixel_buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    pixel_buffer.make_mut_bytes().copy_from_slice(rgb.as_raw());

    pixel_buffer
}

impl ImageContainer {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            metadata: None,
            cached_preview: RefCell::new(None),
            settings: ImageSettings::default(),
        }
    }

    pub fn get_full_preview(&self) -> Result<SharedPixelBuffer<Rgb8Pixel>, ModelError> {
        if let Some(cached) = self.cached_preview.borrow().as_ref() {
            return Ok(cached.clone());
        }

        let image = dynamic_image_to_slint_image(load_compressed_image(&self.path)?);
        self.cached_preview.replace(Some(image.clone()));

        Ok(image)
    }

    pub fn get_thumbnail(&self) -> Result<SharedPixelBuffer<Rgb8Pixel>, ModelError> {
        Ok(dynamic_image_to_slint_image(
            load_compressed_image(&self.path)?.thumbnail(300, 300),
        ))
    }

    // Image settings
    pub fn settings(&self) -> ImageSettings {
        self.settings.clone()
    }

    pub fn set_settings(&mut self, settings: ImageSettings) {
        self.settings = settings;
    }

    // Metadata methods

    pub fn load_metadata(&mut self) -> Result<(), Box<dyn Error>> {
        let file = std::fs::File::open(&self.path)?;
        let mut bufreader = BufReader::new(file);
        let exifreader = exif::Reader::new();
        let exif = exifreader.read_from_container(&mut bufreader)?;

        self.metadata = Some(exif);

        Ok(())
    }

    pub fn is_metadata_loaded(&self) -> bool {
        self.metadata.is_some()
    }

    pub fn fields(&mut self) -> Vec<Field> {
        if let Some(exif) = &self.metadata {
            return exif.fields().cloned().collect();
        }

        self.load_metadata().unwrap();

        self.metadata.as_ref().unwrap().fields().cloned().collect()
    }
}
