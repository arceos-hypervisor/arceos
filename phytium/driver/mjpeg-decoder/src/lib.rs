#![no_std]

//! MJPEG解码器库
//!
//! 这是一个专为no_std环境设计的MJPEG解码器，支持将MJPEG帧转换为RGBA数组。
//! 现在使用zune-jpeg库进行专业的JPEG解码。

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use zune_jpeg::JpegDecoder;

// 在测试时允许使用std
#[cfg(test)]
extern crate std;

#[macro_use]
extern crate log;

/// RGBA像素结构体
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaPixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaPixel {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// 计算像素亮度
    pub fn luminance(&self) -> f32 {
        0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32
    }
}

/// 图像尺寸
#[derive(Debug, Clone, Copy)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

impl ImageSize {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn pixel_count(&self) -> usize {
        (self.width * self.height) as usize
    }
}

/// MJPEG解码错误类型
#[derive(Debug)]
pub enum MjpegError {
    InvalidFormat,
    UnsupportedFormat,
    InsufficientData,
    DecodingFailed(String),
}

impl core::fmt::Display for MjpegError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MjpegError::InvalidFormat => write!(f, "Invalid MJPEG format"),
            MjpegError::UnsupportedFormat => write!(f, "Unsupported format"),
            MjpegError::InsufficientData => write!(f, "Insufficient data"),
            MjpegError::DecodingFailed(msg) => write!(f, "Decoding failed: {msg}"),
        }
    }
}

/// MJPEG解码器
pub struct MjpegDecoder {
    /// 目标图像尺寸
    pub target_size: Option<ImageSize>,
    /// 质量参数（用于简化解码）
    pub quality_factor: u8,
}

impl Default for MjpegDecoder {
    fn default() -> Self {
        Self {
            target_size: None,
            quality_factor: 75,
        }
    }
}

impl MjpegDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建带有目标尺寸的解码器
    pub fn with_size(width: u32, height: u32) -> Self {
        Self {
            target_size: Some(ImageSize::new(width, height)),
            quality_factor: 75,
        }
    }

    /// 设置质量参数
    pub fn set_quality(&mut self, quality: u8) {
        self.quality_factor = quality.min(100);
    }

    /// 解码MJPEG数据为RGBA数组
    pub fn decode_to_rgba(&self, mjpeg_data: &[u8]) -> Result<Vec<RgbaPixel>, MjpegError> {
        // 验证JPEG魔数
        if mjpeg_data.len() < 2 || mjpeg_data[0] != 0xFF || mjpeg_data[1] != 0xD8 {
            return Err(MjpegError::InvalidFormat);
        }

        // 使用zune-jpeg进行专业解码
        let mut decoder = JpegDecoder::new(mjpeg_data);

        // 解码为RGB数据
        let rgb_data = decoder.decode().map_err(|e| {
            error!("zune-jpeg解码失败: {e:?}");
            MjpegError::DecodingFailed("zune-jpeg解码失败".into())
        })?;

        // 获取图像信息
        let info = decoder.info().ok_or_else(|| {
            error!("无法获取图像信息");
            MjpegError::DecodingFailed("无法获取图像信息".into())
        })?;

        trace!(
            "解码成功: {}x{} 像素, {} bytes",
            info.width,
            info.height,
            rgb_data.len()
        );

        // 转换RGB数据为RGBA格式
        self.convert_rgb_to_rgba(&rgb_data)
    }

    /// 将RGB数据转换为RGBA格式
    fn convert_rgb_to_rgba(&self, rgb_data: &[u8]) -> Result<Vec<RgbaPixel>, MjpegError> {
        if rgb_data.len() % 3 != 0 {
            return Err(MjpegError::DecodingFailed("RGB数据长度不是3的倍数".into()));
        }

        let pixel_count = rgb_data.len() / 3;
        let mut rgba_pixels = Vec::with_capacity(pixel_count);

        for chunk in rgb_data.chunks_exact(3) {
            rgba_pixels.push(RgbaPixel::new(chunk[0], chunk[1], chunk[2], 255));
        }

        trace!(
            "转换完成: {} RGB像素 -> {} RGBA像素",
            pixel_count,
            rgba_pixels.len()
        );
        Ok(rgba_pixels)
    }

    /// 验证解码结果
    pub fn validate_rgba_data(&self, rgba_data: &[RgbaPixel], expected_size: &ImageSize) -> bool {
        rgba_data.len() == expected_size.pixel_count()
    }
}

/// 颜色分析工具
#[derive(Default)]
pub struct ColorAnalyzer;

/// 颜色分析结果
#[derive(Debug)]
pub struct ColorAnalysisResult {
    pub average_color: RgbaPixel,
    pub dominant_color_name: &'static str,
}

impl ColorAnalyzer {
    /// 创建新的颜色分析器
    pub fn new() -> Self {
        ColorAnalyzer
    }

    /// 分析RGBA数据的颜色信息
    pub fn analyze_rgba_colors(&self, rgba_data: &[RgbaPixel]) -> ColorAnalysisResult {
        let average_color = Self::analyze_dominant_color(rgba_data);
        let dominant_color_name = Self::get_color_name(&average_color);

        ColorAnalysisResult {
            average_color,
            dominant_color_name,
        }
    }

    /// 分析RGBA数据的主要颜色
    pub fn analyze_dominant_color(rgba_data: &[RgbaPixel]) -> RgbaPixel {
        if rgba_data.is_empty() {
            return RgbaPixel::from_rgb(0, 0, 0);
        }

        let mut r_sum = 0u32;
        let mut g_sum = 0u32;
        let mut b_sum = 0u32;

        for pixel in rgba_data {
            r_sum += pixel.r as u32;
            g_sum += pixel.g as u32;
            b_sum += pixel.b as u32;
        }

        let len = rgba_data.len() as u32;
        RgbaPixel::from_rgb(
            (r_sum / len) as u8,
            (g_sum / len) as u8,
            (b_sum / len) as u8,
        )
    }

    /// 计算颜色分布
    pub fn color_distribution(rgba_data: &[RgbaPixel]) -> (u32, u32, u32) {
        let mut red_pixels = 0;
        let mut green_pixels = 0;
        let mut blue_pixels = 0;

        for pixel in rgba_data {
            let max_channel = pixel.r.max(pixel.g).max(pixel.b);
            if pixel.r == max_channel && pixel.r > 100 {
                red_pixels += 1;
            } else if pixel.g == max_channel && pixel.g > 100 {
                green_pixels += 1;
            } else if pixel.b == max_channel && pixel.b > 100 {
                blue_pixels += 1;
            }
        }

        (red_pixels, green_pixels, blue_pixels)
    }

    /// 获取颜色名称
    pub fn get_color_name(pixel: &RgbaPixel) -> &'static str {
        let r = pixel.r;
        let g = pixel.g;
        let b = pixel.b;

        // 判断是否为灰度
        let is_grayscale = (r as i16 - g as i16).abs() < 30
            && (g as i16 - b as i16).abs() < 30
            && (r as i16 - b as i16).abs() < 30;

        if is_grayscale {
            let brightness = (r as u16 + g as u16 + b as u16) / 3;
            return match brightness {
                0..=50 => "黑色",
                51..=100 => "深灰色",
                101..=180 => "灰色",
                181..=220 => "浅灰色",
                _ => "白色",
            };
        }

        // 找出最大的颜色分量
        let max_val = r.max(g).max(b);
        let min_val = r.min(g).min(b);
        let diff = max_val - min_val;

        // 如果颜色饱和度很低，仍然认为是灰色
        if diff < 40 {
            return "浅灰色";
        }

        // 确定主色调
        if r == max_val && g >= b {
            if g as u16 > (r as u16 * 2 / 3) {
                "黄色"
            } else {
                "红色"
            }
        } else if r == max_val && b > g {
            if b as u16 > (r as u16 * 2 / 3) {
                "品红色"
            } else {
                "红色"
            }
        } else if g == max_val && r >= b {
            if r as u16 > (g as u16 * 2 / 3) {
                "黄色"
            } else {
                "绿色"
            }
        } else if g == max_val && b > r {
            if b as u16 > (g as u16 * 2 / 3) {
                "青色"
            } else {
                "绿色"
            }
        } else if b == max_val && r >= g {
            if r as u16 > (b as u16 * 2 / 3) {
                "品红色"
            } else {
                "蓝色"
            }
        } else if g as u16 > (b as u16 * 2 / 3) {
            "青色"
        } else {
            "蓝色"
        }
    }
}
