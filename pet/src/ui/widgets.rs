//! 界面部件: 音量指示条 / 语音气泡。

use ply_engine::prelude::*;
use macroquad::text::Font;
use macroquad::color::Color as MacroquadColor;


/// 右上角绘制音量指示条(音量变化后短暂显�?。y=90 避开 Android 状态栏(�?0-66px)�?
pub(crate) fn draw_volume_indicator(volume: f32) {
    let x = screen_width() - 70.0;
    let y = 90.0;
    let w = 46.0;
    let h = 14.0;
    draw_rectangle(x, y, w, h, MacroquadColor::new(0.0, 0.0, 0.0, 0.55));
    let fill = (w - 4.0) * volume.clamp(0.0, 1.0);
    if fill > 1.0 {
        draw_rectangle(x + 2.0, y + 2.0, fill, h - 4.0, MacroquadColor::new(0.3, 0.85, 0.5, 0.95));
    }
}

/// 角色头顶的台词气�? 半透明黑底白字 + 小三角尾巴指向角色�?
/// 字号/内边距随 ui_scale 缩放(Android 2.7x), 长文本自动换行�?
pub(crate) fn draw_speech_bubble(text: &str, font: &macroquad::text::Font, cx: f32, char_top: f32) {
    let ui_scale = if cfg!(any(target_os = "android", target_env = "ohos")) {
        (screen_width() / 400.0).clamp(1.0, 3.5)
    } else {
        1.0
    };
    let font_size = (20.0 * ui_scale).round() as u16;
    let pad_x = 14.0 * ui_scale;
    let pad_y = 9.0 * ui_scale;
    let max_w = (screen_width() * 0.72).max(120.0);
    // 自动换行 + 测多行尺�?行距 1.3)
    let wrapped = macroquad::text::wrap_text(text, Some(font), font_size, 1.0, max_w - pad_x * 2.0);
    let dims = macroquad::text::measure_multiline_text(&wrapped, Some(font), font_size, 1.0, Some(1.3));
    let bw = dims.width + pad_x * 2.0;
    let bh = dims.height + pad_y * 2.0;
    let bx = (cx - bw / 2.0).clamp(6.0, screen_width() - bw - 6.0);
    let by = (char_top - bh - 18.0 * ui_scale).max(4.0);
    let bg = MacroquadColor::new(0.0, 0.0, 0.0, 0.75);
    draw_rectangle(bx, by, bw, bh, bg);
    // 三角尾巴
    let tail_w = 12.0 * ui_scale;
    let tail_h = 10.0 * ui_scale;
    let tail_x = (cx - tail_w / 2.0).clamp(bx + 4.0, bx + bw - tail_w - 4.0);
    draw_triangle(
        macroquad::math::Vec2::new(tail_x, by + bh),
        macroquad::math::Vec2::new(tail_x + tail_w, by + bh),
        macroquad::math::Vec2::new(tail_x + tail_w / 2.0, by + bh + tail_h),
        bg,
    );
    // 多行白字(首行基线 = 气泡�?top + offset_y)
    macroquad::text::draw_multiline_text_ex(
        &wrapped,
        bx + pad_x,
        by + pad_y + dims.offset_y,
        Some(1.3),
        macroquad::text::TextParams {
            font: Some(font),
            font_size,
            color: MacroquadColor::new(1.0, 1.0, 1.0, 1.0),
            ..Default::default()
        },
    );
}
