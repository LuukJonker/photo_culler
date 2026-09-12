use image::RgbImage;
use nalgebra::{Matrix3, Vector3};
use rsraw::{BIT_DEPTH_16, RawImage};

use crate::error::ModelError;

pub fn raw_img_to_jpg(
    raw_img: &mut RawImage,
    white_balance: Vector3<f32>,
    ccm: Matrix3<f32>,
    baseline_env: f32,
) -> Result<RgbImage, ModelError> {
    let processed = raw_img.process::<BIT_DEPTH_16>().unwrap();

    let float_values: Vec<f32> = processed
        .iter()
        .map(|v| (*v as f32) / u16::MAX as f32)
        .collect();

    let baseline_multiplier = 1.0;

    let mut balanced_pixels: Vec<Vector3<f32>> = float_values
        .chunks_exact(3)
        .map(|chunk| {
            Vector3::<f32>::new(chunk[0], chunk[1], chunk[2]) //.component_mul(&white_balance)
        })
        .map(|v| (ccm * v) * baseline_multiplier)
        .collect();

    let mut rgb_buf = Vec::with_capacity(balanced_pixels.len() * 3);

    for pixel in balanced_pixels.iter_mut() {
        // Clamp the value in the matrix between 0 and 1

        // Apply sRGB Gamma curve
        pixel.apply(|cell| {
            *cell = if *cell <= 0.0031308 {
                12.92 * *cell
            } else {
                1.055 * (*cell).powf(1.0 / 2.4) - 0.055
            }
        });

        // Apply s-curve for contrast
        //pixel.apply(|cell| *cell = (*cell) * (*cell) * (3.0 - 2.0 * (*cell)));
        pixel.apply(|cell| *cell = cell.clamp(0.0, 1.0));

        for i in 0..=2 {
            // for some reason had to be in reverse order we got it
            rgb_buf.push((pixel[i] * 255.0) as u8);
        }
    }

    Ok(RgbImage::from_vec(processed.width(), processed.height(), rgb_buf).unwrap())
}
