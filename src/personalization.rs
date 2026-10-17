//! Local-only, bounded image import. Original paths and metadata are never persisted.
use crate::error::{Error, Result};
use image::{ImageDecoder, ImageFormat, ImageReader};
use serde::{Deserialize, Serialize};
use std::{
    io::{Cursor, Read},
    path::Path,
};
pub const MAX_IMAGE_BYTES: u64 = 12 * 1024 * 1024;
pub const MAX_IMAGE_PIXELS: u64 = 16_000_000;
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundFit {
    #[default]
    Cover,
    Contain,
    Stretch,
    Center,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Personalization {
    pub theme: crate::settings::ThemeMode,
    pub fit: BackgroundFit,
    pub position_x: u8,
    pub position_y: u8,
    pub image_opacity: u8,
    pub panel_opacity: u8,
    pub overlay_opacity: u8,
    pub blur: u8,
    pub brightness: u8,
    pub overlay_color: String,
    pub reduced_motion: bool,
}
impl Default for Personalization {
    fn default() -> Self {
        Self {
            theme: Default::default(),
            fit: Default::default(),
            position_x: 50,
            position_y: 50,
            image_opacity: 65,
            panel_opacity: 94,
            overlay_opacity: 20,
            blur: 6,
            brightness: 100,
            overlay_color: "#172039".into(),
            reduced_motion: false,
        }
    }
}
impl Personalization {
    pub fn validate(&self) -> Result<()> {
        if self.position_x > 100
            || self.position_y > 100
            || self.image_opacity > 100
            || !(80..=100).contains(&self.panel_opacity)
            || self.overlay_opacity > 100
            || ![0, 6, 14, 24].contains(&self.blur)
            || !(40..=140).contains(&self.brightness)
            || self.overlay_color.len() != 7
            || !self.overlay_color.starts_with('#')
            || !self.overlay_color[1..]
                .bytes()
                .all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::Invalid("个性化参数超出安全范围"));
        }
        Ok(())
    }
}
pub fn sanitize_image(path: &Path, avatar: bool) -> Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_IMAGE_BYTES {
        return Err(Error::Invalid("图片不能超过 12 MiB"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(Error::Invalid("图片不能超过 12 MiB"));
    }
    let mut reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| Error::Invalid("无法识别图片"))?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP)
    ) {
        return Err(Error::Invalid("仅支持 PNG、JPEG 和 WebP 图片"));
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let (width, height) = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| Error::Invalid("无法识别图片"))?
        .into_dimensions()
        .map_err(|_| Error::Invalid("图片已损坏"))?;
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err(Error::Invalid("图片不能超过 1600 万像素"));
    }
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| Error::Invalid("图片损坏或解码内存超限"))?;
    let orientation = decoder
        .orientation()
        .map_err(|_| Error::Invalid("图片方向信息损坏"))?;
    let mut image = image::DynamicImage::from_decoder(decoder)
        .map_err(|_| Error::Invalid("图片损坏或解码内存超限"))?;
    image.apply_orientation(orientation);
    let image = if avatar {
        image.resize_to_fill(256, 256, image::imageops::FilterType::Triangle)
    } else {
        image.thumbnail(2560, 1600)
    };
    let mut output = Cursor::new(Vec::new());
    if avatar {
        image
            .write_to(&mut output, ImageFormat::Png)
            .map_err(|_| Error::Io)?;
    } else {
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, 85)
            .encode_image(&image.to_rgb8())
            .map_err(|_| Error::Io)?;
    }
    let output = output.into_inner();
    if output.len() as u64 > MAX_IMAGE_BYTES {
        return Err(Error::Invalid("处理后的图片超出容量限制"));
    }
    Ok(output)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_import_is_bounded_and_sanitized() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.png");
        image::RgbaImage::new(600, 400).save(&path).unwrap();
        let output = sanitize_image(&path, true).unwrap();
        let decoded = image::load_from_memory(&output).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (256, 256));
        std::fs::write(&path, b"not an image").unwrap();
        assert!(sanitize_image(&path, false).is_err());
        let file = std::fs::File::create(&path).unwrap();
        file.set_len(MAX_IMAGE_BYTES + 1).unwrap();
        assert!(sanitize_image(&path, false).is_err());
    }
    #[test]
    fn jpeg_webp_and_pixel_limits_are_enforced() {
        let dir = tempfile::tempdir().unwrap();
        for (name, format) in [
            ("photo.jpg", ImageFormat::Jpeg),
            ("photo.webp", ImageFormat::WebP),
        ] {
            let path = dir.path().join(name);
            image::RgbImage::new(40, 30)
                .save_with_format(&path, format)
                .unwrap();
            let bytes = sanitize_image(&path, false).unwrap();
            assert_eq!(image::guess_format(&bytes).unwrap(), ImageFormat::Jpeg);
        }
        let path = dir.path().join("oversized.png");
        image::GrayImage::new(5000, 4000).save(&path).unwrap();
        assert!(matches!(
            sanitize_image(&path, false),
            Err(Error::Invalid("图片不能超过 1600 万像素"))
        ));
    }
    #[test]
    fn appearance_limits_preserve_readability() {
        let mut p = Personalization::default();
        assert!(p.validate().is_ok());
        p.panel_opacity = 0;
        assert!(p.validate().is_err());
        p.panel_opacity = 80;
        p.overlay_color = "url(https://invalid)".into();
        assert!(p.validate().is_err());
    }
}
