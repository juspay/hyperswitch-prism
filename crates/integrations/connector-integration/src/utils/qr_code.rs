//! Shared QR code utilities for connectors
//!
//! This module provides common QR code generation and handling functionality
//! that can be used across different connector implementations.

use base64::Engine;
use error_stack::ResultExt;
use image::{DynamicImage, ImageBuffer, ImageFormat, Luma, Rgba};
use url::Url;

/// QR code information variants for different connector response formats
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(untagged)]
// the enum order shouldn't be changed as this is being used during serialization and deserialization
pub enum QrCodeInformation {
    QrCodeUrl {
        image_data_url: Url,
        qr_code_url: Url,
        display_to_timestamp: Option<i64>,
    },
    QrDataUrl {
        image_data_url: Url,
        display_to_timestamp: Option<i64>,
    },
    QrCodeImageUrl {
        qr_code_url: Url,
        display_to_timestamp: Option<i64>,
    },
    QrColorDataUrl {
        color_image_data_url: Url,
        display_to_timestamp: Option<i64>,
        display_text: Option<String>,
        border_color: Option<String>,
    },
}

/// QR code image with base64 encoded data
#[derive(Debug)]
pub struct QrImage {
    pub data: String,
}

// Qr Image data source starts with this string
// The base64 image data will be appended to it to image data source
pub(crate) const QR_IMAGE_DATA_SOURCE_STRING: &str = "data:image/png;base64";
pub const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

impl QrImage {
    pub fn new_from_data(data: String) -> Result<Self, error_stack::Report<QrCodeError>> {
        let qr_code = qrcode::QrCode::new(data.as_bytes())
            .change_context(QrCodeError::FailedToCreateQrCode)?;

        let qrcode_image_buffer = qr_code.render::<Luma<u8>>().build();
        let qrcode_dynamic_image = DynamicImage::ImageLuma8(qrcode_image_buffer);

        let mut image_bytes = std::io::BufWriter::new(std::io::Cursor::new(Vec::new()));

        // Encodes qrcode_dynamic_image and write it to image_bytes
        let _ = qrcode_dynamic_image.write_to(&mut image_bytes, ImageFormat::Png);

        let image_data_source = format!(
            "{},{}",
            QR_IMAGE_DATA_SOURCE_STRING,
            BASE64_ENGINE.encode(image_bytes.buffer())
        );
        Ok(Self {
            data: image_data_source,
        })
    }

    pub fn new_colored_from_data(
        data: String,
        hex_color: &str,
    ) -> Result<Self, error_stack::Report<QrCodeError>> {
        let qr_code = qrcode::QrCode::new(data.as_bytes())
            .change_context(QrCodeError::FailedToCreateQrCode)?;

        let qrcode_image_buffer = qr_code.render::<Luma<u8>>().build();
        let (width, height) = qrcode_image_buffer.dimensions();
        let mut colored_image = ImageBuffer::new(width, height);
        let rgb = Self::parse_hex_color(hex_color)?;

        for (x, y, pixel) in qrcode_image_buffer.enumerate_pixels() {
            let luminance = pixel.0[0];
            let color = if luminance == 0 {
                Rgba([rgb.0, rgb.1, rgb.2, 255])
            } else {
                Rgba([255, 255, 255, 255])
            };
            colored_image.put_pixel(x, y, color);
        }

        let qrcode_dynamic_image = DynamicImage::ImageRgba8(colored_image);
        let mut image_bytes = std::io::Cursor::new(Vec::new());
        qrcode_dynamic_image
            .write_to(&mut image_bytes, ImageFormat::Png)
            .change_context(QrCodeError::FailedToCreateQrCode)?;

        let image_data_source = format!(
            "{},{}",
            QR_IMAGE_DATA_SOURCE_STRING,
            BASE64_ENGINE.encode(image_bytes.get_ref())
        );

        Ok(Self {
            data: image_data_source,
        })
    }

    pub fn parse_hex_color(hex: &str) -> Result<(u8, u8, u8), QrCodeError> {
        let hex = hex.trim_start_matches('#');
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok();
            let g = u8::from_str_radix(&hex[2..4], 16).ok();
            let b = u8::from_str_radix(&hex[4..6], 16).ok();
            if let (Some(r), Some(g), Some(b)) = (r, g, b) {
                return Ok((r, g, b));
            }
        }
        Err(QrCodeError::InvalidHexColor)
    }
}

/// Errors for Qr code handling
#[derive(Debug, thiserror::Error)]
pub enum QrCodeError {
    /// Failed to encode data into Qr code
    #[error("Failed to create Qr code")]
    FailedToCreateQrCode,
    /// Failed to parse hex color
    #[error("Invalid hex color code supplied")]
    InvalidHexColor,
}
