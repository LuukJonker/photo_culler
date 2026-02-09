use std::error::Error;

use image::DynamicImage;
use slint::{Image, Rgb8Pixel, SharedPixelBuffer};


pub fn load_compressed_image(path: &str) -> Result<image::DynamicImage, Box<dyn Error>> {
    let img = image::ImageReader::open(path)?.decode()?;

    Ok(img)
}

pub fn dynamic_image_to_slint_image(dyn_img: DynamicImage) -> Image {
    let rgb = dyn_img.into_rgb8();
    let width = rgb.width();
    let height = rgb.height();

    let mut pixel_buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    pixel_buffer.make_mut_bytes().copy_from_slice(rgb.as_raw());

    Image::from_rgb8(pixel_buffer)
}
