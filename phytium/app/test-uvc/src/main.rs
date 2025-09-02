#![no_std]
#![no_main]

use std::vec::Vec;

use arceos_usb::{TransferError, err::USBError};
use crab_uvc::{UvcDevice, VideoControlEvent};

extern crate axplat_aarch64_dyn;

#[macro_use]
extern crate axstd as std;
#[macro_use]
extern crate log;

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

        // 设置视频格式 (选择第一个可用格式)
        if let Some(format) = formats.first() {
            info!("Setting format: {:?}", format);
            uvc.set_format(format.clone()).await.unwrap();
        } else {
            panic!("No supported formats available");
        }

        // 开始视频流
        info!("Starting video streaming...");
        let mut stream = uvc.start_streaming().await.unwrap();

        // 获取当前视频格式信息
        let current_format = stream.vedio_format.clone();
        info!("Current video format: {:?}", current_format);

        // 设置一些控制参数的示例
        info!("Setting video controls...");

        // 尝试设置亮度（如果失败也继续）
        if let Err(e) = uvc
            .send_control_command(VideoControlEvent::BrightnessChanged(100))
            .await
        {
            warn!("Failed to set brightness: {:?}", e);
        }

        let mut total_frames = 0;
        let mut first_frame_captured = false;

        // 获取第一帧完整图像后跳出循环
        while !first_frame_captured {
            match stream.recv().await {
                Ok(frames) => {
                    for frame in frames {
                        total_frames += 1;
                        info!(
                            "Received frame {}: {} bytes",
                            total_frames,
                            frame.data.len()
                        );

                        // 检查是否为完整帧（有EOF标志）
                        if frame.eof && !frame.data.is_empty() {
                            println!("=== CAPTURED FIRST COMPLETE FRAME ===");

                            // 输出视频格式信息到串口日志
                            println!("VIDEO_FORMAT_START");
                            println!("VIDEO_FORMAT: {:?}", current_format);
                            println!("VIDEO_FORMAT_END");

                            // 输出原始图像数据到串口日志（分块输出避免日志缓冲区溢出）
                            println!("FRAME_DATA_START");
                            println!("FRAME_SIZE: {}", frame.data.len());
                            println!("FRAME_NUMBER: {}", frame.frame_number);
                            if let Some(pts) = frame.pts_90khz {
                                println!("FRAME_PTS: {}", pts);
                            }

                            // 将数据按4KB块分割输出，使用十六进制编码
                            const CHUNK_SIZE: usize = 4096 * 4;
                            let chunks = frame.data.chunks(CHUNK_SIZE);
                            let total_chunks = chunks.len();

                            for (chunk_idx, chunk) in chunks.enumerate() {
                                // 将每个字节转换为十六进制字符串
                                let hex_data = chunk
                                    .iter()
                                    .map(|b| format!("{b:02x}"))
                                    .collect::<Vec<_>>()
                                    .join("");

                                println!(
                                    "CHUNK_{:04}_{:04}: {}",
                                    chunk_idx, total_chunks, hex_data
                                );
                            }

                            println!("FRAME_DATA_END");

                            first_frame_captured = true;
                            break;
                        }
                    }
                }
                Err(e) => {
                    // 检查是否是暂时性 XHCI 错误
                    if let USBError::TransferError(TransferError::Other(err_msg)) = &e
                        && (err_msg.contains("MissedServiceError") || err_msg.contains("temporary"))
                    {
                        warn!("Temporary XHCI error, retrying: {}", err_msg);
                        continue;
                    }
                    error!("Unrecoverable frame error: {:?}", e);
                    continue;
                }
            }
        }

        info!("First frame captured successfully, stopping stream...");
        // 停止视频流
        drop(stream);
    });
}
