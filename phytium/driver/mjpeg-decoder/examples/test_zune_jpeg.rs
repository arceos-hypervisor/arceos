//! 使用zune-jpeg解码器测试真实JPEG文件的颜色识别
//!
//! 这个示例测试frame_000000.jpg文件，期望识别出橙色或红色

use mjpeg_decoder::{ColorAnalyzer, MjpegDecoder};
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== zune-jpeg MJPEG解码器颜色识别测试 ===\n");

    // 读取JPEG文件
    let jpeg_path = "examples/frame_000000.jpg";
    println!("读取JPEG文件: {}", jpeg_path);

    let jpeg_data = fs::read(jpeg_path)?;
    println!("文件大小: {} bytes", jpeg_data.len());

    // 验证文件格式
    if jpeg_data.len() >= 2 {
        println!("文件头: {:02X} {:02X}", jpeg_data[0], jpeg_data[1]);
        if jpeg_data[0] == 0xFF && jpeg_data[1] == 0xD8 {
            println!("✓ 文件具有有效的JPEG魔数");
        } else {
            println!("✗ 文件不具有JPEG魔数");
        }
    }

    // 创建解码器和分析器
    let decoder = MjpegDecoder::new();
    let analyzer = ColorAnalyzer::new();

    println!("\n--- 开始zune-jpeg解码 ---");

    // 解码JPEG为RGBA数据
    match decoder.decode_to_rgba(&jpeg_data) {
        Ok(rgba_pixels) => {
            println!("✓ zune-jpeg解码成功!");
            println!("输出像素数: {}", rgba_pixels.len());

            // 估算图像尺寸
            let total_pixels = rgba_pixels.len();
            let width = (total_pixels as f64).sqrt() as u32;
            let height = total_pixels as u32 / width;
            println!("估算图像尺寸: {}x{}", width, height);

            // 使用ColorAnalyzer分析颜色
            println!("\n--- 颜色分析 ---");
            let color_result = analyzer.analyze_rgba_colors(&rgba_pixels);

            println!(
                "平均颜色: RGB({}, {}, {})",
                color_result.average_color.r,
                color_result.average_color.g,
                color_result.average_color.b
            );

            println!("主要颜色: {}", color_result.dominant_color_name);

            // 详细的颜色分析
            let avg_color = &color_result.average_color;
            println!("\n--- 详细分析 ---");
            println!("红色占比: {:.2}", avg_color.r as f32 / 255.0);
            println!("绿色占比: {:.2}", avg_color.g as f32 / 255.0);
            println!("蓝色占比: {:.2}", avg_color.b as f32 / 255.0);

            // 判断是否为橙红色系
            let is_red_dominant = avg_color.r > avg_color.g && avg_color.r > avg_color.b;
            let is_orange_red = is_red_dominant && (avg_color.g as f32 / avg_color.r as f32) > 0.3;
            let is_pure_red = is_red_dominant && (avg_color.g as f32 / avg_color.r as f32) <= 0.3;

            println!("\n--- 颜色判断 ---");
            println!(
                "是否红色主导: {}",
                if is_red_dominant { "是" } else { "否" }
            );
            println!("是否橙红色: {}", if is_orange_red { "是" } else { "否" });
            println!("是否纯红色: {}", if is_pure_red { "是" } else { "否" });

            // 区域采样分析
            println!("\n--- 区域采样分析 ---");
            let sample_size = 1000.min(rgba_pixels.len());
            let step = rgba_pixels.len() / sample_size;

            let mut red_pixels = 0;
            let mut orange_pixels = 0;
            let mut warm_pixels = 0;
            let mut other_pixels = 0;

            for i in (0..rgba_pixels.len()).step_by(step) {
                let pixel = &rgba_pixels[i];

                // 分类像素
                if pixel.r > pixel.g && pixel.r > pixel.b {
                    // 红色主导
                    let green_ratio = pixel.g as f32 / pixel.r as f32;
                    if green_ratio > 0.5 {
                        orange_pixels += 1;
                    } else {
                        red_pixels += 1;
                    }
                } else if (pixel.r + pixel.g) as u16 > pixel.b as u16 * 2 {
                    // 暖色调
                    warm_pixels += 1;
                } else {
                    other_pixels += 1;
                }
            }

            println!("采样像素数: {}", sample_size);
            println!(
                "红色像素: {} ({:.1}%)",
                red_pixels,
                red_pixels as f32 / sample_size as f32 * 100.0
            );
            println!(
                "橙色像素: {} ({:.1}%)",
                orange_pixels,
                orange_pixels as f32 / sample_size as f32 * 100.0
            );
            println!(
                "暖色像素: {} ({:.1}%)",
                warm_pixels,
                warm_pixels as f32 / sample_size as f32 * 100.0
            );
            println!(
                "其他像素: {} ({:.1}%)",
                other_pixels,
                other_pixels as f32 / sample_size as f32 * 100.0
            );

            // 显示部分像素值
            println!("\n--- 像素采样 ---");
            for i in (0..10.min(rgba_pixels.len())).step_by(rgba_pixels.len() / 10 + 1) {
                let pixel = &rgba_pixels[i];
                println!("像素 {}: RGB({}, {}, {})", i, pixel.r, pixel.g, pixel.b);
            }

            // 测试结果
            println!("\n=== 测试结果 ===");
            let red_orange_percentage =
                (red_pixels + orange_pixels) as f32 / sample_size as f32 * 100.0;

            if color_result.dominant_color_name.contains("红")
                || color_result.dominant_color_name.contains("橙")
            {
                println!(
                    "✓ 主色识别: {} - 符合预期!",
                    color_result.dominant_color_name
                );
            } else if red_orange_percentage > 25.0 {
                println!(
                    "✓ 虽然主色识别为'{}', 但红/橙色像素占比{:.1}% - 部分符合预期",
                    color_result.dominant_color_name, red_orange_percentage
                );
            } else {
                println!(
                    "? 主色识别为'{}', 红/橙色像素占比{:.1}% - 不符合预期",
                    color_result.dominant_color_name, red_orange_percentage
                );
            }

            if is_red_dominant {
                if is_orange_red {
                    println!("✓ 颜色特征分析: 橙红色 - 符合预期!");
                } else {
                    println!("✓ 颜色特征分析: 红色 - 符合预期!");
                }
            } else {
                println!("? 颜色特征分析: 非红色系");
            }
        }
        Err(e) => {
            println!("✗ zune-jpeg解码失败: {:?}", e);
            return Err(format!("解码失败: {:?}", e).into());
        }
    }

    println!("\n测试完成!");
    Ok(())
}
