#![no_std]
#![no_main]

use crab_uvc::{UvcDevice, VideoControlEvent};
use std::vec::Vec;

extern crate axplat_aarch64_dyn;

#[macro_use]
extern crate axstd as std;
#[macro_use]
extern crate log;

mod color_detect;
mod model;

use color_detect::{ColorDetector, DetectorType};

/// 选择最优的视频格式 - 优先选择最小的未压缩格式
fn select_optimal_format(formats: &[crab_uvc::VideoFormat]) -> Option<crab_uvc::VideoFormat> {
    let mut uncompressed_formats = Vec::new();
    let mut compressed_formats = Vec::new();

    for format in formats {
        // 检查是否为未压缩格式
        // 常见的未压缩格式包括 YUYV, RGB24, RGB32 等
        let format_str = format!("{:?}", format);
        if format_str.contains("YUYV")
            || format_str.contains("RGB")
            || format_str.contains("YUV")
            || format_str.contains("UYVY")
            || format_str.contains("NV12")
        {
            uncompressed_formats.push(format.clone());
        } else {
            compressed_formats.push(format.clone());
        }
    }

    // 优先选择未压缩格式中分辨率最小的
    if !uncompressed_formats.is_empty() {
        uncompressed_formats.sort_by_key(|f| {
            // 尝试解析分辨率，如果无法解析则使用较大的默认值
            let format_str = format!("{:?}", f);
            let mut width = 9999u32;
            let mut height = 9999u32;

            // 简单的分辨率解析逻辑
            if format_str.contains("320") && format_str.contains("240") {
                width = 320;
                height = 240;
            } else if format_str.contains("640") && format_str.contains("480") {
                width = 640;
                height = 480;
            } else if format_str.contains("800") && format_str.contains("600") {
                width = 800;
                height = 600;
            } else if format_str.contains("1024") && format_str.contains("768") {
                width = 1024;
                height = 768;
            } else if format_str.contains("1280") && format_str.contains("720") {
                width = 1280;
                height = 720;
            } else if format_str.contains("1920") && format_str.contains("1080") {
                width = 1920;
                height = 1080;
            }

            width * height
        });
        info!(
            "Found {} uncompressed formats, selecting smallest",
            uncompressed_formats.len()
        );
        return uncompressed_formats.first().cloned();
    }

    // 如果没有未压缩格式，选择压缩格式中最小的
    if !compressed_formats.is_empty() {
        compressed_formats.sort_by_key(|f| {
            let format_str = format!("{:?}", f);
            let mut width = 9999u32;
            let mut height = 9999u32;

            if format_str.contains("320") && format_str.contains("240") {
                width = 320;
                height = 240;
            } else if format_str.contains("640") && format_str.contains("480") {
                width = 640;
                height = 480;
            } else if format_str.contains("800") && format_str.contains("600") {
                width = 800;
                height = 600;
            }

            width * height
        });
        info!("No uncompressed formats found, selecting smallest compressed format");
        return compressed_formats.first().cloned();
    }

    None
}

/// 根据视频格式创建相应的颜色检测器
fn create_color_detector_for_format(format: &crab_uvc::VideoFormat) -> ColorDetector {
    let format_str = format!("{:?}", format);
    info!("Creating color detector for format: {}", format_str);

    // 根据格式确定检测器类型
    let detector_type = if format_str.contains("YUYV") {
        DetectorType::Yuyv
    } else if format_str.contains("RGB24") {
        DetectorType::Rgb24
    } else if format_str.contains("RGB32") || format_str.contains("RGBA") {
        DetectorType::Rgb32
    } else if format_str.contains("MJPEG") || format_str.contains("JPEG") {
        DetectorType::Mjpeg
    } else {
        // 对于未知格式，使用通用检测器
        DetectorType::Generic
    };

    // 根据格式调整采样间隔
    let sample_interval = if format_str.contains("320") && format_str.contains("240") {
        // 小分辨率，使用较小的采样间隔获得更好的精度
        4
    } else if format_str.contains("640") && format_str.contains("480") {
        // 中等分辨率，使用中等采样间隔
        6
    } else if format_str.contains("800") && format_str.contains("600")
        || format_str.contains("1024") && format_str.contains("768")
    {
        // 较高分辨率，使用较大的采样间隔以减少计算负担
        8
    } else {
        // 超高分辨率或未知格式，使用最大采样间隔
        12
    };

    info!(
        "Using detector type: {:?}, sample interval: {} for format: {}",
        detector_type, sample_interval, format_str
    );
    ColorDetector::new_for_format(sample_interval, detector_type)
}

#[unsafe(no_mangle)]
fn main() {
    spin_on::spin_on(async {
        println!("Test UVC camera start...");

        let ls = arceos_usb::dev_list().unwrap();

        info!("USB Device List {}:", ls.len());
        for dev in &ls {
            info!("  {dev}");
        }

        let mut dev_info = ls
            .into_iter()
            .find(|d| UvcDevice::check(d))
            .expect("No UVC device found");

        let dev = dev_info.open().await.unwrap();

        let mut uvc = UvcDevice::new(dev).await.unwrap();

        // 获取设备信息
        let device_info = uvc.get_device_info().await.unwrap();
        info!("Device info: {}", device_info);

        // 获取支持的视频格式
        let formats = uvc.get_supported_formats().await.unwrap();
        info!("Supported formats:");
        for format in &formats {
            info!("  {:?}", format);
        }

        // 选择最小的未压缩格式
        let selected_format = select_optimal_format(&formats);
        match selected_format {
            Some(format) => {
                info!("Selected optimal format: {:?}", format);
                uvc.set_format(format.clone()).await.unwrap();
            }
            None => {
                panic!("No suitable uncompressed format found");
            }
        }

        // 开始视频流
        info!("Starting video streaming...");
        let mut stream = uvc.start_streaming().await.unwrap();

        // 获取当前视频格式信息
        let current_format = stream.vedio_format.clone();
        info!("Current video format: {:?}", current_format);

        // 根据选择的格式创建颜色检测器
        let color_detector = create_color_detector_for_format(&current_format);
        info!(
            "颜色检测器已初始化，采样间隔: {}，针对格式: {:?}",
            color_detector.sample_interval, current_format
        );

        // 设置一些控制参数的示例
        info!("Setting video controls...");

        // 尝试设置亮度（如果失败也继续）
        if let Err(e) = uvc
            .send_control_command(VideoControlEvent::BrightnessChanged(100))
            .await
        {
            warn!("Failed to set brightness: {:?}", e);
        }

        let mut frame_count = 0;
        let mut i = 0;
        let mut start = std::time::Instant::now();

        loop {
            let batch = stream.recv().await.unwrap();
            for frame in batch {
                frame_count += 1;
                info!(
                    "Received frame {}: {} bytes",
                    frame.frame_number,
                    frame.data.len(),
                );

                // 每2帧进行一次颜色检测以减少计算负担
                if frame_count % 2 == 0 {
                    match color_detector.detect_color_from_frame(&frame.data) {
                        Ok(result) => {
                            color_detector.log_color_result(&result, frame.frame_number);
                        }
                        Err(e) => {
                            warn!("颜色检测失败 (帧 #{}): {}", frame.frame_number, e);
                        }
                    }
                }
            }
            i += 1;

            if i % 100 == 0 {
                info!("Total frames received so far: {}", frame_count);
                let elapsed = start.elapsed().as_secs_f32();
                let fps = frame_count as f32 / elapsed;
                info!(
                    "Elapsed time: {:.2} seconds, Approx. FPS: {:.2}",
                    elapsed, fps
                );
                frame_count = 0;
                start = std::time::Instant::now();
            }
        }

        info!("First frame captured successfully, stopping stream...");
        // 停止视频流
        drop(stream);
    });
}
