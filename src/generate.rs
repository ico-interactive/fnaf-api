use image::{DynamicImage, ImageFormat, ImageReader, RgbaImage};
use tracing::{info, warn};

use crate::fnaf::FACE_PATH;
use std::{
    cmp::min,
    fs,
    path::{Path, PathBuf},
    result::Result,
};

const DEFAULT_WIDTH: u32 = 510;
const DEFAULT_HEIGHT: u32 = 510;

fn check_if_exists(path: &PathBuf) -> bool {
    let decode_result = ImageReader::open(path);
    decode_result.is_ok()
}

fn create_test_filetype(path: PathBuf, file_type: ImageFormat) -> Result<(), image::ImageError> {
    if check_if_exists(&path) {
        return Ok(());
    };
    let size = match file_type {
        ImageFormat::Ico => [min(255, DEFAULT_WIDTH), min(255, DEFAULT_HEIGHT)],
        _ => [DEFAULT_WIDTH, DEFAULT_HEIGHT],
    };
    let mut rgba = RgbaImage::new(size[0], size[1]);
    for p in rgba.enumerate_pixels_mut() {
        let (x, y, pixel) = p;
        pixel.0 = [
            (x + y % 255) as u8,
            (x % 255) as u8,
            (y % 255) as u8,
            (x - y % 255) as u8,
        ]
    }
    if let Err(e) = DynamicImage::ImageRgba8(rgba).save_with_format(&path, file_type) {
        fs::remove_file(path)?;
        return Err(e);
    }
    Ok(())
}

pub async fn try_create_test_images() -> Result<(), image::ImageError> {
    for image_format in ImageFormat::all() {
        let file_name = "fnaf.".to_owned() + image_format.extensions_str()[0];
        let path = Path::new(&*FACE_PATH).join(&file_name).to_path_buf();
        if let Err(e) = create_test_filetype(path, image_format) {
            warn!("could not create {}, reason: {}", file_name, e);
        } else {
            info!("successfully created {}", file_name)
        }
    }
    Ok(())
}
