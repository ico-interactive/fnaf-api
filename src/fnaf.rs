use std::{error::Error, io::Cursor, sync::LazyLock};

use ab_glyph::{FontRef, PxScale};
use image::{
    DynamicImage, GenericImage, GenericImageView, ImageReader, Rgba, codecs::avif::AvifEncoder,
};
use imageproc::{
    drawing::{draw_text_mut, text_size},
    morphology::dilate_mut,
};

static FONT: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(include_bytes!("../NotoSerifDisplay.otf")).expect("font to be valid")
});

const MARGIN: f32 = 2.0;

pub struct FnafOpts<'a> {
    pub text: &'a str,
    pub bottom: bool,
}

pub fn try_image(opts: FnafOpts) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut image = ImageReader::open("fnaf.png")?.decode()?;

    add_text(&mut image, &*FONT, opts);

    let mut bytes: Vec<u8> = vec![];
    image.write_with_encoder(AvifEncoder::new_with_speed_quality(
        Cursor::new(&mut bytes),
        8,
        70,
    ))?;
    Ok(bytes)
}

fn add_text(image: &mut DynamicImage, font: &FontRef, opts: FnafOpts) {
    let (width, height) = image.dimensions();

    let naive_scale = PxScale::from(150.0);
    let scale = get_correct_scale(&opts.text, naive_scale, (width, height), font);

    let (text_width, text_height) = {
        let size = text_size(scale, font, &opts.text);
        (size.0.min(width), size.1.min(height))
    };
    let text_start_x = ((width - text_width) / 2) as i32;
    let text_start_y = if opts.bottom {
        (height - text_height) as i32
    } else {
        ((height - text_height) / 2) as i32
    };

    draw_text_with_border(
        image,
        Rgba([255, 255, 255, 255]),
        text_start_x,
        text_start_y,
        scale,
        font,
        &opts.text,
        Rgba([0, 0, 0, 255]),
        (scale.x * 0.015) as u8,
    );
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
    canvas: &mut DynamicImage,
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
    dilate_mut(
        &mut image2,
        imageproc::distance_transform::Norm::LInf,
        outline_width,
    );

    for x in 0..image2.width() {
        for y in 0..image2.height() {
            let pixval = 255 - image2.get_pixel(x, y).0[0];
            if pixval != 255 {
                canvas.put_pixel(x, y, outline_color);
            }
        }
    }
    draw_text_mut(canvas, color, x, y, scale, font, text);
}
