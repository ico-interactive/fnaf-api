use std::{
    env,
    error::Error,
    io::Cursor,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use ab_glyph::{FontRef, PxScale};
use image::ImageReader;
use image::{DynamicImage, GenericImage, Rgba, RgbaImage, codecs::avif::AvifEncoder};
use imageproc::{
    compose::overlay_mut,
    distance_transform::Norm,
    drawing::{draw_text_mut, text_size},
    morphology::dilate_mut,
};

static FONT: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(include_bytes!("../NotoSerifDisplay.otf")).expect("font to be valid")
});

const DEFAULT_IMAGE: &str = "fnaf.png";
const MARGIN: f32 = 2.0;

pub struct FnafOpts<'a> {
    pub text: &'a str,
    pub bottom_text: &'a str,
    pub top_text: &'a str,

    pub custom_url: Option<&'a String>,
}

fn get_local_image(image: &Path) -> PathBuf {
    let file_name = image.file_name().map_or(DEFAULT_IMAGE, |v| {
        v.to_str().expect("os string to be convertable")
    });
    Path::new(&env::var("FACE_DIR").unwrap_or(".".to_string()))
        .join(file_name)
        .to_path_buf()
}

pub async fn try_image(opts: FnafOpts<'_>) -> Result<Vec<u8>, Box<dyn Error>> {
    let url = opts.custom_url.map_or(DEFAULT_IMAGE, |v| v);

    let mut image = if url.starts_with("http://") || url.starts_with("https://") {
        ImageReader::new(Cursor::new(reqwest::get(url).await?.bytes().await?))
            .with_guessed_format()?
            .decode()?
    } else {
        let path = get_local_image(Path::new(url));
        ImageReader::open(path)?.decode()?
    };

    add_text(
        image.as_mut_rgba8().ok_or("expected rgba8 image")?,
        &FONT,
        opts,
    );

    let mut bytes: Vec<u8> = vec![];
    image.write_with_encoder(AvifEncoder::new_with_speed_quality(
        Cursor::new(&mut bytes),
        env::var("FNAF_ENCODER_SPEED")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8),
        env::var("FNAF_ENCODER_QUALITY")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(70),
    ))?;
    Ok(bytes)
}

fn add_text(image: &mut RgbaImage, font: &FontRef, opts: FnafOpts) {
    let (width, height) = image.dimensions();

    let texts = [opts.text, opts.bottom_text, opts.top_text];
    let naive_scale = PxScale::from(150.0);

    // usize thats either 0, 1 or 2, corresponding to the index of the text which is the longest
    let largest_text = texts
        .iter()
        .enumerate()
        .max_by_key(|&(_, value)| text_size(naive_scale, font, value))
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    let scale = get_correct_scale(texts[largest_text], naive_scale, (width, height), font);

    let mut text_pos: [(i32, i32); 3] = [(0, 0); 3];

    let (middle_text_width, middle_text_height) = {
        let size = text_size(scale, font, opts.text);
        (size.0.min(width), size.1.min(height))
    };
    text_pos[0] = (
        ((width - middle_text_width) / 2) as i32,
        ((height - middle_text_height) / 2) as i32,
    );

    let text_width = {
        let size = text_size(scale, font, opts.bottom_text);
        size.0.min(width)
    };
    text_pos[1] = (
        ((width - text_width) / 2) as i32,
        text_pos[0].1 + middle_text_height as i32,
    );

    let (text_width, text_height) = {
        let size = text_size(scale, font, opts.top_text);
        (size.0.min(width), size.1.min(height))
    };
    text_pos[2] = (
        ((width - text_width) / 2) as i32,
        text_pos[0].1 - text_height as i32,
    );

    text_pos.iter().zip(texts.iter()).for_each(|(pos, text)| {
        draw_text_with_border(
            image,
            Rgba([255, 255, 255, 255]),
            pos.0,
            pos.1,
            scale,
            font,
            text,
            Rgba([0, 0, 0, 255]),
            (scale.x * 0.015) as u8,
        );
    });
}

fn get_correct_scale(
    text: &str,
    scale: PxScale,
    image_size: (u32, u32),
    font: &FontRef,
) -> PxScale {
    let text_width = text_size(scale, font, text).0;
    let factor = image_size.0 as f32 / text_width as f32;
    PxScale::from(scale.x * factor - MARGIN)
}

// a modified version of:
// https://github.com/silvia-odwyer/gdl/blob/421c8df718ad32f66275d178edec56ec653caff9/crate/src/text.rs#L23
#[allow(clippy::too_many_arguments)]
pub fn draw_text_with_border(
    canvas: &mut RgbaImage,
    color: Rgba<u8>,
    x: i32,
    y: i32,
    scale: PxScale,
    font: &FontRef,
    text: &str,
    outline_color: Rgba<u8>,
    outline_width: u8,
) {
    let mut image2: DynamicImage = DynamicImage::new_luma8(canvas.width(), canvas.height());

    draw_text_mut(&mut image2, color, x, y, scale, font, text);

    let mut image2 = image2.to_luma8();
    dilate_mut(&mut image2, Norm::LInf, outline_width);

    let mut precanvas = DynamicImage::new_rgba8(canvas.width(), canvas.height());

    for x in 0..image2.width() {
        for y in 0..image2.height() {
            let pixval = 255 - image2.get_pixel(x, y).0[0];
            if pixval != 255 {
                precanvas.put_pixel(x, y, outline_color);
            }
        }
    }

    precanvas = precanvas.blur(0.7);

    draw_text_mut(&mut precanvas, color, x, y, scale, font, text);
    overlay_mut(
        canvas,
        precanvas.as_rgba8().expect("precanvas to be rgba8"),
        0,
        0,
    );
}
