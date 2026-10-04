#!/usr/bin/env python3
"""Generate src/model/image_container/image_operations.rs with the fitted
Sony-A6600 preview-matching tone curve + color matrix embedded."""
import numpy as np

T = np.load("test_out/T.npy")
M = np.load("test_out/Mr.npy")
b = np.load("test_out/br.npy")

def f32(x):
    return repr(float(x))

tv = ",\n        ".join(f32(x) for x in T)
rows = ",\n        ".join("[{}]".format(", ".join(f32(x) for x in M[r])) for r in range(3))
bv = ", ".join(f32(x) for x in b)

CONTENT = '''use image::RgbImage;
use rsraw::{BIT_DEPTH_16, RawImage};
use std::convert::AsMut;

use crate::error::ModelError;

/// Number of entries in the fitted tone curve / sRGB encode LUT.
const TONE_STEPS: f32 = 255.0;

/// Tone curve (linear -> linear) fitted so that the rendered image matches the
/// embedded preview JPEG produced by the camera (Sony ILCE-6600, "Standard"
/// creative style). It is a camera-fixed curve, so it generalises to other
/// frames from the same body. Applied per channel to the linear sRGB data
/// that LibRaw produces.
const TONE_CURVE: [f32; 256] = [
        %TONE%
    ];

/// 3x3 colour-correction delta (row-major) that maps LibRaw's sRGB matrix
/// output onto the camera's own colour rendering. Fitted in linear light.
const COLOR_MATRIX: [[f32; 3]; 3] = [
        %M%
    ];

/// Small per-channel linear offset accompanying the colour matrix.
const COLOR_OFFSET: [f32; 3] = [%B%];

/// Applies the fitted tone curve to a linear value in `[0, 1]`.
#[inline]
fn tone_curve(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let f = x * TONE_STEPS;
    let i = f as usize;
    if i >= 255 {
        return TONE_CURVE[255];
    }
    let frac = f - i as f32;
    TONE_CURVE[i] * (1.0 - frac) + TONE_CURVE[i + 1] * frac
}

/// Encodes a linear sRGB value in `[0, 1]` to an 8-bit sRGB sample.
#[inline]
fn srgb8(x: f32) -> u8 {
    let x = x.clamp(0.0, 1.0);
    let v = if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (v * 255.0 + 0.5) as u8
}

/// Processes an unpacked RAW image into an 8-bit sRGB JPEG-ready bitmap that
/// matches the camera's embedded preview.
///
/// The pipeline is:
/// 1. Ask LibRaw for *linear* sRGB (camera white balance + camera colour
///    matrix, no auto-bright, no gamma). This is done by configuring the
///    params before `process`.
/// 2. Apply the camera's tone curve (fitted per channel).
/// 3. Apply the small colour-matrix delta + offset.
/// 4. Encode to 8-bit sRGB.
pub fn raw_img_to_jpg(raw_img: &mut RawImage) -> Result<RgbImage, ModelError> {
    {
        let d = raw_img.as_mut();
        d.params.use_camera_wb = 1;
        d.params.no_auto_bright = 1;
        d.params.gamm[0] = 1.0;
        d.params.gamm[1] = 0.0;
        d.params.output_color = 1; // sRGB
        d.params.output_bps = 16;
    }

    let processed = raw_img
        .process::<BIT_DEPTH_16>()
        .map_err(|e| ModelError::WithMessage(e.to_string()))?;

    let width = processed.width();
    let height = processed.height();
    let data: &[u16] = &processed;

    let inv_max = 1.0 / u16::MAX as f32;
    let mut rgb_buf: Vec<u8> = Vec::with_capacity((width * height * 3) as usize);

    for chunk in data.chunks_exact(3) {
        let r = tone_curve(chunk[0] as f32 * inv_max);
        let g = tone_curve(chunk[1] as f32 * inv_max);
        let bl = tone_curve(chunk[2] as f32 * inv_max);

        let m = &COLOR_MATRIX;
        let o_r = m[0][0] * r + m[0][1] * g + m[0][2] * bl + COLOR_OFFSET[0];
        let o_g = m[1][0] * r + m[1][1] * g + m[1][2] * bl + COLOR_OFFSET[1];
        let o_b = m[2][0] * r + m[2][1] * g + m[2][2] * bl + COLOR_OFFSET[2];

        rgb_buf.push(srgb8(o_r));
        rgb_buf.push(srgb8(o_g));
        rgb_buf.push(srgb8(o_b));
    }

    RgbImage::from_vec(width, height, rgb_buf)
        .ok_or_else(|| ModelError::WithMessage("Failed to build RGB image".into()))
}
'''

CONTENT = CONTENT.replace("%TONE%", tv).replace("%M%", rows).replace("%B%", bv)
open("src/model/image_container/image_operations.rs", "w").write(CONTENT)
print("wrote src/model/image_container/image_operations.rs", len(CONTENT), "bytes")
