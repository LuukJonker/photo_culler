//! Test harness for `raw_img_to_jpg`.
//!
//! Usage:
//!   cargo run --bin raw2jpg -- <raw_file> [output.jpg] [--quality N] [--half-size]
//!
//! Loads the given RAW file, runs it through `raw_img_to_jpg` (the same pipeline
//! the app's export uses) and writes the resulting JPEG to disk so the output
//! of the function can be inspected.

use std::env;
use std::fs::{File, create_dir_all};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, RgbImage};

use photo_culler::model::image_container::jpgfromraw;
use photo_culler::model::image_container::{
    color_data_from_raw, image_operations::raw_img_to_jpg, jpgfromraw::process_file,
    load_full_raw_image,
};

const DEFAULT_QUALITY: u8 = 90;

const USAGE: &str = "\
Usage: raw2jpg <raw_file> [output.jpg] [options]

Arguments:
  raw_file    Path to the RAW file to process (required)
  output.jpg  Where to write the exported JPEG
              (default: test_out/<raw file name>.jpg)

Options:
  --quality N   JPEG quality 1-100 (default: 90)
  --half-size   Process at half size (faster, lower resolution)
  -h, --help    Show this help
";

fn default_output_path(raw_path: &Path) -> PathBuf {
    let stem = raw_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "export".into());
    Path::new("test_out").join(format!("{stem}.jpg"))
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    let mut raw_path: Option<PathBuf> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut quality: u8 = DEFAULT_QUALITY;
    let mut half_size = false;
    let mut preview = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--quality" => {
                i += 1;
                match args.get(i).and_then(|q| q.parse::<u8>().ok()) {
                    Some(q) if (1..=100).contains(&q) => quality = q,
                    _ => {
                        eprintln!("error: --quality requires a number between 1 and 100");
                        return ExitCode::FAILURE;
                    }
                }
            }
            "--preview" => {
                preview = true;
            }
            "--half-size" => half_size = true,
            other if other.starts_with('-') => {
                eprintln!("error: unknown option {other}");
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            }
            positional => {
                if raw_path.is_none() {
                    raw_path = Some(PathBuf::from(positional));
                } else if out_path.is_none() {
                    out_path = Some(PathBuf::from(positional));
                } else {
                    eprintln!("error: unexpected extra argument {positional}");
                    eprintln!("{USAGE}");
                    return ExitCode::FAILURE;
                }
            }
        }
        i += 1;
    }

    let raw_path = match raw_path {
        Some(p) => p,
        None => {
            eprintln!("error: missing raw file path");
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    if !raw_path.is_file() {
        eprintln!("error: raw file not found: {}", raw_path.display());
        return ExitCode::FAILURE;
    }

    let out_path = out_path.unwrap_or_else(|| default_output_path(&raw_path));

    let jpg_image;
    if preview {
        let (jpg_bytes, orientation) = jpgfromraw::process_file(&raw_path).unwrap();

        let img = image::load_from_memory(&jpg_bytes).unwrap();

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

        jpg_image = img.to_rgb8();
    } else {
        println!("Loading RAW: {}", raw_path.display());
        let mut raw_image = match load_full_raw_image(&raw_path, half_size) {
            Ok(img) => img,
            Err(e) => {
                eprintln!("error: failed to load RAW image: {e}");
                return ExitCode::FAILURE;
            }
        };

        let (wb, ccm, base_ev) = color_data_from_raw(&raw_image);
        println!("White balance: {:?}", wb);
        println!("CCM:\n{ccm}");
        println!("Baseline EV: {base_ev}");

        println!("Processing with raw_img_to_jpg...");
        let started = std::time::Instant::now();
        jpg_image = match raw_img_to_jpg(&mut raw_image, wb, ccm, base_ev) {
            Ok(img) => img,
            Err(e) => {
                eprintln!("error: raw_img_to_jpg failed: {e}");
                return ExitCode::FAILURE;
            }
        };

        println!(
            "Processed {}x{} image in {:?}",
            jpg_image.width(),
            jpg_image.height(),
            started.elapsed()
        );
    }

    if let Some(parent) = out_path.parent() {
        if let Err(e) = create_dir_all(parent) {
            eprintln!(
                "error: failed to create output directory {}: {e}",
                parent.display()
            );
            return ExitCode::FAILURE;
        }
    }

    let file = match File::create(&out_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "error: failed to create output file {}: {e}",
                out_path.display()
            );
            return ExitCode::FAILURE;
        }
    };

    let mut encoder = JpegEncoder::new_with_quality(file, quality);
    if let Err(e) = encoder.encode(
        jpg_image.as_raw(),
        jpg_image.width(),
        jpg_image.height(),
        ExtendedColorType::Rgb8,
    ) {
        eprintln!("error: failed to encode JPEG: {e}");
        return ExitCode::FAILURE;
    }

    let size = std::fs::metadata(&out_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "Saved JPEG (quality {quality}): {} ({} bytes)",
        out_path.display(),
        size
    );

    ExitCode::SUCCESS
}
