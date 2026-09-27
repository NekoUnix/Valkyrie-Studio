//! Fit decoded texture atlases to the active GPU without edge color bleeding.
use crate::asset_limits as limits;

pub fn fit_texture(mut rgba: image::RgbaImage, maximum: u32) -> image::RgbaImage {
    let [w, h] = limits::texture_size(rgba.width(), rgba.height(), maximum);
    if (w, h) == rgba.dimensions() {
        return rgba;
    }
    for pixel in rgba.pixels_mut() {
        let a = u32::from(pixel[3]);
        for c in &mut pixel.0[..3] {
            *c = ((u32::from(*c) * a + 127) / 255) as u8;
        }
    }
    let mut resized = image::imageops::resize(&rgba, w, h, image::imageops::FilterType::Triangle);
    for pixel in resized.pixels_mut() {
        let a = u32::from(pixel[3]);
        for c in &mut pixel.0[..3] {
            *c = (u32::from(*c) * 255 + a / 2)
                .checked_div(a)
                .unwrap_or(0)
                .min(255) as u8;
        }
    }
    resized
}
