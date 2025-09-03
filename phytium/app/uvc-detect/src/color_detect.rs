//! MJPEG颜色检测模块
//!
//! 该模块提供了从MJPEG帧中提取颜色信息的功能，包括：
//! - JPEG解码
//! - RGB颜色均值计算
//! - 主要颜色检测
//! - 颜色名称识别

use mjpeg_decoder::{ColorAnalyzer, MjpegDecoder};
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

/// 颜色变化事件
#[derive(Debug)]
pub struct ColorChangeEvent {
    /// 帧号
    pub frame_number: u64,
    /// 新的主题色
    pub new_color: String,
    /// 平均颜色
    pub average_color: RgbColor,
    /// 亮度等级
    pub brightness_level: String,
    /// 当前FPS
    pub current_fps: f32,
    /// 总帧数
    pub total_frames: u64,
}

/// 检测统计信息
#[derive(Debug)]
pub struct DetectionStatistics {
    /// 总帧数
    pub total_frames: u64,
    /// 当前FPS
    pub current_fps: f32,
    /// 运行时间（秒）
    pub elapsed_time: f32,
    /// 当前主题色
    pub current_dominant_color: Option<String>,
}

/// 颜色检测器
pub struct ColorDetector {
    /// 采样间隔，用于降低计算复杂度
    pub sample_interval: usize,
    /// 检测器类型，用于处理不同的视频格式
    pub detector_type: DetectorType,
    /// MJPEG解码器
    mjpeg_decoder: MjpegDecoder,
    /// 颜色分析器
    color_analyzer: ColorAnalyzer,
    /// 当前主题色
    current_dominant_color: Option<String>,
    /// 帧计数器
    frame_count: u64,
    /// 开始时间
    start_time: Option<std::time::Instant>,
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
            mjpeg_decoder: MjpegDecoder::new(),
            color_analyzer: ColorAnalyzer::new(),
            current_dominant_color: None,
            frame_count: 0,
            start_time: None,
        }
    }
}

impl ColorDetector {
    pub fn new(sample_interval: usize) -> Self {
        Self {
            sample_interval,
            detector_type: DetectorType::Mjpeg,
            mjpeg_decoder: MjpegDecoder::new(),
            color_analyzer: ColorAnalyzer::new(),
            current_dominant_color: None,
            frame_count: 0,
            start_time: None,
        }
    }

    /// 创建专门针对特定格式的颜色检测器
    pub fn new_for_format(sample_interval: usize, detector_type: DetectorType) -> Self {
        Self {
            sample_interval,
            detector_type,
            mjpeg_decoder: MjpegDecoder::new(),
            color_analyzer: ColorAnalyzer::new(),
            current_dominant_color: None,
            frame_count: 0,
            start_time: None,
        }
    }

    /// 从视频帧数据中检测颜色，根据检测器类型选择相应的处理方法
    pub fn detect_color_from_frame(
        &mut self,
        frame_data: &[u8],
    ) -> Result<ColorDetectionResult, String> {
        match &self.detector_type {
            DetectorType::Mjpeg => self.detect_color_from_mjpeg(frame_data),
            _ => panic!("Only MJPEG detection is implemented in this version."),
        }
    }

    /// 检测颜色变化并更新统计信息
    pub fn detect_and_track_color_change(
        &mut self,
        frame_data: &[u8],
        frame_number: u64,
    ) -> Result<Option<ColorChangeEvent>, String> {
        // 初始化开始时间
        if self.start_time.is_none() {
            self.start_time = Some(std::time::Instant::now());
        }

        // 更新帧计数
        self.frame_count += 1;

        // 检测颜色
        let result = self.detect_color_from_frame(frame_data)?;

        // 检查主题色是否发生变化
        let color_changed = match &self.current_dominant_color {
            None => {
                // 第一次检测到颜色
                self.current_dominant_color = Some(result.dominant_color_name.clone());
                true
            }
            Some(current_color) => {
                if current_color != &result.dominant_color_name {
                    // 颜色发生变化
                    self.current_dominant_color = Some(result.dominant_color_name.clone());
                    true
                } else {
                    false
                }
            }
        };

        if color_changed {
            // 计算FPS
            let fps = self.calculate_current_fps();

            Ok(Some(ColorChangeEvent {
                frame_number,
                new_color: result.dominant_color_name.clone(),
                average_color: result.average_color,
                brightness_level: result.brightness_level.clone(),
                current_fps: fps,
                total_frames: self.frame_count,
            }))
        } else {
            Ok(None)
        }
    }

    /// 计算当前FPS
    pub fn calculate_current_fps(&self) -> f32 {
        if let Some(start_time) = self.start_time {
            let elapsed = start_time.elapsed().as_secs_f32();
            if elapsed > 0.0 {
                self.frame_count as f32 / elapsed
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> DetectionStatistics {
        let fps = self.calculate_current_fps();
        let elapsed = if let Some(start_time) = self.start_time {
            start_time.elapsed().as_secs_f32()
        } else {
            0.0
        };

        DetectionStatistics {
            total_frames: self.frame_count,
            current_fps: fps,
            elapsed_time: elapsed,
            current_dominant_color: self.current_dominant_color.clone(),
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
        &mut self,
        mjpeg_data: &[u8],
    ) -> Result<ColorDetectionResult, String> {
        // 使用我们的专业MJPEG解码器
        let rgba_pixels = self
            .mjpeg_decoder
            .decode_to_rgba(mjpeg_data)
            .map_err(|e| format!("MJPEG解码失败: {:?}", e))?;

        // 使用采样间隔减少计算量
        let sampled_pixels: Vec<_> = rgba_pixels
            .iter()
            .step_by(self.sample_interval)
            .cloned()
            .collect();

        // 使用专业的颜色分析器
        let analysis_result = self.color_analyzer.analyze_rgba_colors(&sampled_pixels);

        // 转换为我们的结果格式
        let average_color = RgbColor::new(
            analysis_result.average_color.r,
            analysis_result.average_color.g,
            analysis_result.average_color.b,
        );

        let brightness_level = self.get_brightness_level(&average_color);

        Ok(ColorDetectionResult {
            average_color,
            dominant_color_name: analysis_result.dominant_color_name.to_string(),
            brightness_level,
            sampled_pixels: sampled_pixels.len(),
        })
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
