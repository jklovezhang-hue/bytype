//! 生成 ByType 应用图标:蓝色圆角药丸 + 白色波形五柱(与录音浮窗视觉一致)。
//! 运行:cargo run --example gen_icon
//! 输出:src-tauri/icons/app-icon-source.png(1024×1024,透明背景)
//! 然后:npm run tauri -- icon src-tauri/icons/app-icon-source.png  重新生成全套图标。

use image::{imageops, ImageBuffer, Rgba};

const SS: u32 = 4; // 超采样倍数(先大画再缩小 = 抗锯齿)
const OUT: u32 = 1024;

fn main() {
    let size = OUT * SS;
    let mut img = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(size, size, Rgba([0, 0, 0, 0]));

    let s = size as f32;
    let (cx, cy) = (0.5 * s, 0.5 * s);
    let blue = Rgba([59, 130, 246, 255]); // tailwind blue-500,与界面主色一致

    // 横向药丸:直段半长 0.24,端帽半径 0.24(总宽 0.96、高 0.48,居中)
    draw_capsule_h(&mut img, cx, cy, 0.24 * s, 0.24 * s, blue);

    // 白色波形五柱(竖直小胶囊),中间最高、两侧对称
    let white = Rgba([255, 255, 255, 255]);
    let bar_r = 0.032 * s;
    let gap = 0.10 * s;
    let halves = [0.08_f32, 0.13, 0.17, 0.13, 0.08];
    for (i, h) in halves.iter().enumerate() {
        let x = cx + (i as f32 - 2.0) * gap;
        draw_capsule_v(&mut img, x, cy, h * s, bar_r, white);
    }

    let small = imageops::resize(&img, OUT, OUT, imageops::FilterType::Lanczos3);
    let out = "src-tauri/icons/app-icon-source.png";
    small.save(out).expect("写出 PNG 失败");
    println!("已生成 {out}({OUT}x{OUT})");
}

/// 横向胶囊:水平线段 (cx±half, cy) 膨胀半径 r 的点集。
fn draw_capsule_h(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: f32,
    cy: f32,
    half: f32,
    r: f32,
    c: Rgba<u8>,
) {
    paint(img, c, |x, y| {
        let dx = ((x - cx).abs() - half).max(0.0);
        let dy = y - cy;
        dx * dx + dy * dy <= r * r
    });
}

/// 竖直胶囊:竖直线段 (cx, cy±half) 膨胀半径 r 的点集。
fn draw_capsule_v(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    cx: f32,
    cy: f32,
    half: f32,
    r: f32,
    c: Rgba<u8>,
) {
    paint(img, c, |x, y| {
        let dy = ((y - cy).abs() - half).max(0.0);
        let dx = x - cx;
        dx * dx + dy * dy <= r * r
    });
}

fn paint(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>, c: Rgba<u8>, hit: impl Fn(f32, f32) -> bool) {
    let (w, h) = img.dimensions();
    for y in 0..h {
        for x in 0..w {
            if hit(x as f32 + 0.5, y as f32 + 0.5) {
                img.put_pixel(x, y, c);
            }
        }
    }
}
