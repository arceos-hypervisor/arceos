//! MJPEG颜色检测模块
//!
//! 该模块提供了从MJPEG帧中提取颜色信息的功能，包括：
//! - JPEG解码
//! - RGB颜色均值计算
//! - 主要颜色检测
//! - 颜色名称识别

use std::string::{String, ToString};
use std::vec::Vec;

/// RGB颜色结构体
#[derive(Debug, Clone, Copy)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// 计算颜色的亮度
    pub fn luminance(&self) -> f32 {
        0.299 * self.r as f32 + 0.587 * self.g as f32 + 0.114 * self.b as f32
    }
}

/// 颜色检测结果
#[derive(Debug)]
pub struct ColorDetectionResult {
    /// 平均颜色
    pub average_color: RgbColor,
    /// 主要颜色名称
    pub dominant_color_name: String,
    /// 亮度等级
    pub brightness_level: String,
    /// 采样的像素数量
    pub sampled_pixels: usize,
}

/// 颜色检测器
pub struct ColorDetector {
    /// 采样间隔，用于降低计算复杂度
    pub sample_interval: usize,
    /// 检测器类型，用于处理不同的视频格式
    pub detector_type: DetectorType,
}

/// 检测器类型，针对不同的视频格式
#[derive(Debug, Clone)]
pub enum DetectorType {
    /// MJPEG压缩格式
    Mjpeg,
    /// YUYV未压缩格式
    Yuyv,
    /// RGB24未压缩格式
    Rgb24,
    /// RGB32未压缩格式  
    Rgb32,
    /// 通用格式（使用字节分析）
    Generic,
}

impl Default for ColorDetector {
    fn default() -> Self {
        Self {
            sample_interval: 8, // 每8个像素采样一次
            detector_type: DetectorType::Mjpeg,
        }
    }
}

impl ColorDetector {
    pub fn new(sample_interval: usize) -> Self {
        Self {
            sample_interval,
            detector_type: DetectorType::Mjpeg,
        }
    }

    /// 创建专门针对特定格式的颜色检测器
    pub fn new_for_format(sample_interval: usize, detector_type: DetectorType) -> Self {
        Self {
            sample_interval,
            detector_type,
        }
    }

    /// 从视频帧数据中检测颜色，根据检测器类型选择相应的处理方法
    pub fn detect_color_from_frame(
        &self,
        frame_data: &[u8],
    ) -> Result<ColorDetectionResult, String> {
        match &self.detector_type {
            DetectorType::Mjpeg => self.detect_color_from_mjpeg(frame_data),
            DetectorType::Yuyv => self.detect_color_from_yuyv(frame_data),
            DetectorType::Rgb24 => self.detect_color_from_rgb24(frame_data),
            DetectorType::Rgb32 => self.detect_color_from_rgb32(frame_data),
            DetectorType::Generic => self.detect_color_from_generic(frame_data),
        }
    }

    /// 从YUYV格式数据中检测颜色
    pub fn detect_color_from_yuyv(&self, yuyv_data: &[u8]) -> Result<ColorDetectionResult, String> {
        if yuyv_data.len() < 4 {
            return Err("YUYV数据太短".to_string());
        }

        let mut rgb_pixels = Vec::new();

        // YUYV格式：Y0 U Y1 V（4字节表示2个像素）
        for i in (0..yuyv_data.len()).step_by(4 * self.sample_interval) {
            if i + 3 < yuyv_data.len() {
                let y0 = yuyv_data[i] as i32;
                let u = yuyv_data[i + 1] as i32 - 128;
                let y1 = yuyv_data[i + 2] as i32;
                let v = yuyv_data[i + 3] as i32 - 128;

                // 转换第一个像素 YUV 到 RGB
                let rgb1 = self.yuv_to_rgb(y0, u, v);
                rgb_pixels.push(rgb1);

                // 转换第二个像素 YUV 到 RGB
                let rgb2 = self.yuv_to_rgb(y1, u, v);
                rgb_pixels.push(rgb2);
            }
        }

        if rgb_pixels.is_empty() {
            return Err("无法从YUYV数据中提取像素".to_string());
        }

        self.analyze_rgb_data(&rgb_pixels)
    }

    /// 从RGB24格式数据中检测颜色
    pub fn detect_color_from_rgb24(
        &self,
        rgb24_data: &[u8],
    ) -> Result<ColorDetectionResult, String> {
        if rgb24_data.len() < 3 {
            return Err("RGB24数据太短".to_string());
        }

        let mut rgb_pixels = Vec::new();

        // RGB24格式：每3个字节表示一个像素 (R, G, B)
        for i in (0..rgb24_data.len()).step_by(3 * self.sample_interval) {
            if i + 2 < rgb24_data.len() {
                let r = rgb24_data[i];
                let g = rgb24_data[i + 1];
                let b = rgb24_data[i + 2];
                rgb_pixels.push(RgbColor::new(r, g, b));
            }
        }

        if rgb_pixels.is_empty() {
            return Err("无法从RGB24数据中提取像素".to_string());
        }

        self.analyze_rgb_data(&rgb_pixels)
    }

    /// 从RGB32格式数据中检测颜色
    pub fn detect_color_from_rgb32(
        &self,
        rgb32_data: &[u8],
    ) -> Result<ColorDetectionResult, String> {
        if rgb32_data.len() < 4 {
            return Err("RGB32数据太短".to_string());
        }

        let mut rgb_pixels = Vec::new();

        // RGB32格式：每4个字节表示一个像素 (R, G, B, A 或 B, G, R, A)
        for i in (0..rgb32_data.len()).step_by(4 * self.sample_interval) {
            if i + 3 < rgb32_data.len() {
                // 假设是 RGBA 格式
                let r = rgb32_data[i];
                let g = rgb32_data[i + 1];
                let b = rgb32_data[i + 2];
                // 忽略 alpha 通道
                rgb_pixels.push(RgbColor::new(r, g, b));
            }
        }

        if rgb_pixels.is_empty() {
            return Err("无法从RGB32数据中提取像素".to_string());
        }

        self.analyze_rgb_data(&rgb_pixels)
    }

    /// 从通用格式数据中检测颜色（字节分析方法）
    pub fn detect_color_from_generic(&self, data: &[u8]) -> Result<ColorDetectionResult, String> {
        if data.len() < 3 {
            return Err("数据太短".to_string());
        }

        let mut rgb_pixels = Vec::new();

        // 使用字节分析方法，假设数据包含某种形式的颜色信息
        for i in (0..data.len()).step_by(3 * self.sample_interval) {
            if i + 2 < data.len() {
                // 简单的字节到RGB映射
                let r = data[i];
                let g = data[(i + 1) % data.len()];
                let b = data[(i + 2) % data.len()];
                rgb_pixels.push(RgbColor::new(r, g, b));
            }
        }

        if rgb_pixels.is_empty() {
            // 如果无法提取像素，创建基于数据统计的颜色
            let avg_byte = data.iter().map(|&b| b as u32).sum::<u32>() / data.len() as u32;
            rgb_pixels.push(RgbColor::new(
                avg_byte as u8,
                avg_byte as u8,
                avg_byte as u8,
            ));
        }

        self.analyze_rgb_data(&rgb_pixels)
    }

    /// YUV到RGB颜色空间转换
    fn yuv_to_rgb(&self, y: i32, u: i32, v: i32) -> RgbColor {
        let r = (y + (1.370705 * v as f32) as i32).clamp(0, 255) as u8;
        let g =
            (y - (0.337633 * u as f32) as i32 - (0.698001 * v as f32) as i32).clamp(0, 255) as u8;
        let b = (y + (1.732446 * u as f32) as i32).clamp(0, 255) as u8;

        RgbColor::new(r, g, b)
    }

    /// 从MJPEG帧数据中检测颜色
    pub fn detect_color_from_mjpeg(
        &self,
        mjpeg_data: &[u8],
    ) -> Result<ColorDetectionResult, String> {
        // 尝试从MJPEG数据中提取RGB像素
        let rgb_data = self.decode_mjpeg_simple(mjpeg_data)?;

        // 计算颜色统计信息
        self.analyze_rgb_data(&rgb_data)
    }

    /// 简单的MJPEG解码器（仅适用于基本的JPEG格式）
    /// 注意：这是一个简化版本，实际项目中应该使用专业的JPEG解码库
    fn decode_mjpeg_simple(&self, mjpeg_data: &[u8]) -> Result<Vec<RgbColor>, String> {
        // 查找JPEG标记
        if mjpeg_data.len() < 10 {
            return Err("MJPEG数据太短".to_string());
        }

        // 检查JPEG魔数 (0xFF 0xD8)
        if mjpeg_data[0] != 0xFF || mjpeg_data[1] != 0xD8 {
            return Err("不是有效的JPEG格式".to_string());
        }

        // 由于ArceOS环境限制，我们使用模拟的解码方式
        // 在实际项目中，这里应该调用真正的JPEG解码库
        self.simulate_jpeg_decode(mjpeg_data)
    }

    /// 模拟JPEG解码（用于演示目的）
    /// 实际应用中应该替换为真正的JPEG解码器
    fn simulate_jpeg_decode(&self, mjpeg_data: &[u8]) -> Result<Vec<RgbColor>, String> {
        let mut rgb_pixels = Vec::new();

        // 假设图像尺寸为 640x480（常见的UVC分辨率）
        let width = 640;
        let height = 480;
        let total_pixels = width * height;

        // 从MJPEG数据中提取近似的颜色信息
        // 这是一个简化的方法，通过分析JPEG数据的字节分布来估算颜色
        let data_len = mjpeg_data.len();
        let step = data_len / total_pixels.min(data_len / 3);

        for i in (0..data_len - 2).step_by(step.max(1)) {
            // 从JPEG数据中提取RGB近似值
            let r = mjpeg_data[i];
            let g = mjpeg_data[(i + 1) % data_len];
            let b = mjpeg_data[(i + 2) % data_len];

            rgb_pixels.push(RgbColor::new(r, g, b));

            if rgb_pixels.len() >= total_pixels / (self.sample_interval * self.sample_interval) {
                break;
            }
        }

        if rgb_pixels.is_empty() {
            // 如果无法提取像素，创建一些基于数据的默认颜色
            let avg_byte =
                mjpeg_data.iter().map(|&b| b as u32).sum::<u32>() / mjpeg_data.len() as u32;
            rgb_pixels.push(RgbColor::new(
                avg_byte as u8,
                avg_byte as u8,
                avg_byte as u8,
            ));
        }

        Ok(rgb_pixels)
    }

    /// 分析RGB数据并生成颜色检测结果
    fn analyze_rgb_data(&self, rgb_data: &[RgbColor]) -> Result<ColorDetectionResult, String> {
        if rgb_data.is_empty() {
            return Err("没有RGB数据可分析".to_string());
        }

        // 计算平均颜色
        let mut total_r = 0u32;
        let mut total_g = 0u32;
        let mut total_b = 0u32;

        for pixel in rgb_data {
            total_r += pixel.r as u32;
            total_g += pixel.g as u32;
            total_b += pixel.b as u32;
        }

        let pixel_count = rgb_data.len() as u32;
        let average_color = RgbColor::new(
            (total_r / pixel_count) as u8,
            (total_g / pixel_count) as u8,
            (total_b / pixel_count) as u8,
        );

        // 确定主要颜色名称
        let dominant_color_name = self.get_color_name(&average_color);

        // 计算亮度等级
        let brightness_level = self.get_brightness_level(&average_color);

        Ok(ColorDetectionResult {
            average_color,
            dominant_color_name,
            brightness_level,
            sampled_pixels: rgb_data.len(),
        })
    }

    /// 根据RGB值确定颜色名称
    fn get_color_name(&self, color: &RgbColor) -> String {
        let r = color.r;
        let g = color.g;
        let b = color.b;

        // 判断是否为灰度
        let is_grayscale = (r as i16 - g as i16).abs() < 30
            && (g as i16 - b as i16).abs() < 30
            && (r as i16 - b as i16).abs() < 30;

        if is_grayscale {
            let brightness = (r as u16 + g as u16 + b as u16) / 3;
            return match brightness {
                0..=50 => "黑色".to_string(),
                51..=100 => "深灰色".to_string(),
                101..=180 => "灰色".to_string(),
                181..=220 => "浅灰色".to_string(),
                _ => "白色".to_string(),
            };
        }

        // 找出最大的颜色分量
        let max_val = r.max(g).max(b);
        let min_val = r.min(g).min(b);
        let diff = max_val - min_val;

        // 如果颜色饱和度很低，仍然认为是灰色
        if diff < 40 {
            return "浅灰色".to_string();
        }

        // 确定主色调
        if r == max_val && g >= b {
            if g > r * 2 / 3 {
                "黄色".to_string()
            } else {
                "红色".to_string()
            }
        } else if r == max_val && b > g {
            if b > r * 2 / 3 {
                "品红色".to_string()
            } else {
                "红色".to_string()
            }
        } else if g == max_val && r >= b {
            if r > g * 2 / 3 {
                "黄色".to_string()
            } else {
                "绿色".to_string()
            }
        } else if g == max_val && b > r {
            if b > g * 2 / 3 {
                "青色".to_string()
            } else {
                "绿色".to_string()
            }
        } else if b == max_val && r >= g {
            if r > b * 2 / 3 {
                "品红色".to_string()
            } else {
                "蓝色".to_string()
            }
        } else {
            if g > b * 2 / 3 {
                "青色".to_string()
            } else {
                "蓝色".to_string()
            }
        }
    }

    /// 获取亮度等级描述
    fn get_brightness_level(&self, color: &RgbColor) -> String {
        let luminance = color.luminance();

        match luminance as u8 {
            0..=50 => "很暗".to_string(),
            51..=100 => "暗".to_string(),
            101..=150 => "中等".to_string(),
            151..=200 => "亮".to_string(),
            _ => "很亮".to_string(),
        }
    }

    /// 打印颜色检测结果到日志
    pub fn log_color_result(&self, result: &ColorDetectionResult, frame_number: u32) {
        info!(
            "帧 #{}: 颜色检测结果 - 平均颜色: RGB({}, {}, {}), 主色调: {}, 亮度: {}, 采样像素: {}",
            frame_number,
            result.average_color.r,
            result.average_color.g,
            result.average_color.b,
            result.dominant_color_name,
            result.brightness_level,
            result.sampled_pixels
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_name_detection() {
        let detector = ColorDetector::default();

        // 测试红色
        let red = RgbColor::new(255, 0, 0);
        assert_eq!(detector.get_color_name(&red), "红色");

        // 测试绿色
        let green = RgbColor::new(0, 255, 0);
        assert_eq!(detector.get_color_name(&green), "绿色");

        // 测试蓝色
        let blue = RgbColor::new(0, 0, 255);
        assert_eq!(detector.get_color_name(&blue), "蓝色");

        // 测试白色
        let white = RgbColor::new(255, 255, 255);
        assert_eq!(detector.get_color_name(&white), "白色");

        // 测试黑色
        let black = RgbColor::new(0, 0, 0);
        assert_eq!(detector.get_color_name(&black), "黑色");
    }

    #[test]
    fn test_brightness_levels() {
        let detector = ColorDetector::default();

        let dark = RgbColor::new(20, 20, 20);
        assert_eq!(detector.get_brightness_level(&dark), "很暗");

        let bright = RgbColor::new(220, 220, 220);
        assert_eq!(detector.get_brightness_level(&bright), "很亮");
    }
}
