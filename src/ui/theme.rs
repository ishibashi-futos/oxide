use ratatui::style::Color;

use crate::core::ColorRgb;

pub fn to_color(color: ColorRgb) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}
