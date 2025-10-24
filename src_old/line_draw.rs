// src/line_draw.rs
// Simple line drawing utilities for RGBA images (Bresenham algorithm)

use image::{Rgba, RgbaImage};

#[allow(dead_code)]
pub fn draw_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>) {
    let (mut x0, mut y0, mut x1, mut y1) = (x0, y0, x1, y1);
    let steep = (y1 - y0).abs() > (x1 - x0).abs();
    if steep {
        std::mem::swap(&mut x0, &mut y0);
        std::mem::swap(&mut x1, &mut y1);
    }
    if x0 > x1 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
    }
    let dx = x1 - x0;
    let dy = (y1 - y0).abs();
    let mut err = dx / 2;
    let ystep = if y0 < y1 { 1 } else { -1 };
    let mut y = y0;
    for x in x0..=x1 {
        if steep {
            if y >= 0 && y < img.width() as i32 && x >= 0 && x < img.height() as i32 {
                img.put_pixel(y as u32, x as u32, color);
            }
        } else {
            if x >= 0 && x < img.width() as i32 && y >= 0 && y < img.height() as i32 {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
        err -= dy;
        if err < 0 {
            y += ystep;
            err += dx;
        }
    }
}
