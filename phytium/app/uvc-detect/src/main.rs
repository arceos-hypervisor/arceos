#![no_std]
#![no_main]

use crab_uvc::{UvcDevice, VideoControlEvent};
use std::string::String;

extern crate axplat_aarch64_dyn;

#[macro_use]
extern crate axstd as std;
#[macro_use]
extern crate log;

mod color_detect;
mod model;

use color_detect::{ColorDetector, DetectorType};

/// 根据视频格式创建相应的颜色检测器
fn create_color_detector_for_format(format: &crab_uvc::VideoFormat) -> ColorDetector {
    let format_str = format!("{:?}", format);
    info!("Creating color detector for format: {format_str}");

    // 根据格式确定检测器类型
    let detector_type = match format.format_type {
        crab_uvc::VideoFormatType::Uncompressed(_) => todo!(),
        crab_uvc::VideoFormatType::Mjpeg => DetectorType::Mjpeg,
        crab_uvc::VideoFormatType::H264 => todo!(),
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
        "Using detector type: {detector_type:?}, sample interval: {sample_interval} for format: {format_str}"
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
        info!("Device info: {device_info}");

        // 获取支持的视频格式
        let formats = uvc.get_supported_formats().await.unwrap();
        info!("Supported formats:");
        for format in &formats {
            info!("  {format:?}");
        }

        let format = formats.first().cloned().unwrap();
        info!("Setting format: {format:?}");
        uvc.set_format(format).await.unwrap();

        // 开始视频流
        info!("Starting video streaming...");
        let mut stream = uvc.start_streaming().await.unwrap();

        // 获取当前视频格式信息
        let current_format = stream.vedio_format.clone();
        info!("Current video format: {current_format:?}", );

        // 根据选择的格式创建颜色检测器
        let mut color_detector = create_color_detector_for_format(&current_format);
        info!(
            "颜色检测器已初始化，针对格式: {current_format:?}",
            
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
        let mut last_print = std::time::Instant::now();

        loop {
            let batch = stream.recv().await.unwrap();
            for frame in batch {
                frame_count += 1;

                // 每帧都进行颜色变化检测
                match color_detector
                    .detect_and_track_color_change(&frame.data, frame.frame_number as u64)
                {
                    Ok(Some(color_change)) => {
                        // 检测到颜色变化，打印详细信息
                        info!("🎨 主题色变化 [帧 #{}]:", color_change.frame_number);
                        info!("  新颜色: {}", color_change.new_color);
                        info!(
                            "  平均RGB: ({}, {}, {})",
                            color_change.average_color.r,
                            color_change.average_color.g,
                            color_change.average_color.b
                        );
                        info!("  亮度: {}", color_change.brightness_level);
                        info!("  当前FPS: {:.2}", color_change.current_fps);
                        info!("  总帧数: {}", color_change.total_frames);
                        info!("  数据大小: {} bytes", frame.data.len());
                    }
                    Ok(None) => {
                        // 颜色没有变化，偶尔打印简要信息
                        if last_print.elapsed().as_secs() >= 10 {
                            let stats = color_detector.get_statistics();
                            info!(
                                "帧 #{}: 当前主题色={}, FPS={:.1}, 总帧数={}",
                                frame.frame_number,
                                stats
                                    .current_dominant_color
                                    .as_ref()
                                    .unwrap_or(&String::from("未知")),
                                stats.current_fps,
                                stats.total_frames
                            );
                            last_print = std::time::Instant::now();
                        }
                    }
                    Err(e) => {
                        if frame_count % 100 == 0 {
                            // 减少错误日志频率
                            warn!("颜色检测失败 (帧 #{}): {}", frame.frame_number, e);
                        }
                    }
                }
            }
            i += 1;

            // 每100批次打印详细统计信息
            if i % 100 == 0 {
                let stats = color_detector.get_statistics();
                info!("=== 颜色检测统计 ===");
                info!("  运行时间: {:.2} 秒", stats.elapsed_time);
                info!("  总帧数: {}", stats.total_frames);
                info!("  平均FPS: {:.2}", stats.current_fps);
                info!(
                    "  当前主题色: {}",
                    stats
                        .current_dominant_color
                        .as_ref()
                        .unwrap_or(&String::from("未检测到"))
                );

                // 重置本地计数器用于下一个统计周期
                frame_count = 0;
            }
        }

        info!("First frame captured successfully, stopping stream...");
        // 停止视频流
        drop(stream);
    });
}
