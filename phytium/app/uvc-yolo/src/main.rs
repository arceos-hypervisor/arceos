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
