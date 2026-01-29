use image::{ImageBuffer, ImageFormat, Rgba};
use std::io::Cursor;
use tiny_skia::{Color, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

use crate::drawing::draw_message::DrawType;
use crate::drawing::DrawMessage;

pub struct Canvas {
    pixmap: Pixmap,
    width: u32,
    height: u32,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        let mut pixmap = Pixmap::new(width, height).expect("Failed to create pixmap");

        // Fill with white background
        pixmap.fill(Color::WHITE);

        Self {
            pixmap,
            width,
            height,
        }
    }

    pub fn draw(&mut self, msg: &DrawMessage) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(
            msg.color_r,
            msg.color_g,
            msg.color_b,
            msg.color_a,
        ));
        paint.anti_alias = true;

        let stroke = Stroke {
            width: msg.thickness as f32,
            line_cap: LineCap::Round,
            line_join: LineJoin::Miter,
            ..Default::default()
        };

        match msg.draw_type {
            DrawType::Brush | DrawType::Line => {
                self.draw_line(msg, &paint, &stroke);
            }
            DrawType::Rectangle => {
                self.draw_rectangle(msg, &paint, &stroke);
            }
            DrawType::Ellipse => {
                self.draw_ellipse(msg, &paint, &stroke);
            }
        }
    }

    fn draw_line(&mut self, msg: &DrawMessage, paint: &Paint, stroke: &Stroke) {
        let mut pb = PathBuilder::new();

        if (msg.x1 - msg.x2).abs() < f64::EPSILON && (msg.y1 - msg.y2).abs() < f64::EPSILON {
            // Draw a point (small circle)
            pb.push_circle(msg.x1 as f32, msg.y1 as f32, 0.5);
        } else {
            pb.move_to(msg.x1 as f32, msg.y1 as f32);
            pb.line_to(msg.x2 as f32, msg.y2 as f32);
        }

        if let Some(path) = pb.finish() {
            self.pixmap
                .stroke_path(&path, paint, stroke, Transform::identity(), None);
        }
    }

    fn draw_rectangle(&mut self, msg: &DrawMessage, paint: &Paint, stroke: &Stroke) {
        let (x1, x2) = if msg.x1 > msg.x2 {
            (msg.x2, msg.x1)
        } else {
            (msg.x1, msg.x2)
        };
        let (y1, y2) = if msg.y1 > msg.y2 {
            (msg.y2, msg.y1)
        } else {
            (msg.y1, msg.y2)
        };

        let width = (x2 - x1) as f32;
        let height = (y2 - y1) as f32;

        if width <= 0.0 || height <= 0.0 {
            return;
        }

        let mut pb = PathBuilder::new();
        if let Some(rect) = tiny_skia::Rect::from_xywh(x1 as f32, y1 as f32, width, height) {
            pb.push_rect(rect);
        }

        if let Some(path) = pb.finish() {
            self.pixmap
                .stroke_path(&path, paint, stroke, Transform::identity(), None);
        }
    }

    fn draw_ellipse(&mut self, msg: &DrawMessage, paint: &Paint, stroke: &Stroke) {
        let (x1, x2) = if msg.x1 > msg.x2 {
            (msg.x2, msg.x1)
        } else {
            (msg.x1, msg.x2)
        };
        let (y1, y2) = if msg.y1 > msg.y2 {
            (msg.y2, msg.y1)
        } else {
            (msg.y1, msg.y2)
        };

        let cx = (x1 + x2) / 2.0;
        let cy = (y1 + y2) / 2.0;
        let rx = (x2 - x1) / 2.0;
        let ry = (y2 - y1) / 2.0;

        if rx <= 0.0 || ry <= 0.0 {
            return;
        }

        let mut pb = PathBuilder::new();
        if let Some(rect) = tiny_skia::Rect::from_xywh(
            (cx - rx) as f32,
            (cy - ry) as f32,
            (rx * 2.0) as f32,
            (ry * 2.0) as f32,
        ) {
            pb.push_oval(rect);
        }

        if let Some(path) = pb.finish() {
            self.pixmap
                .stroke_path(&path, paint, stroke, Transform::identity(), None);
        }
    }

    /// Export canvas to PNG bytes
    pub fn to_png(&self) -> Vec<u8> {
        // Convert tiny-skia Pixmap to image crate format
        let data = self.pixmap.data();

        let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(self.width, self.height);

        for (i, pixel) in img.pixels_mut().enumerate() {
            let offset = i * 4;
            // tiny-skia uses RGBA premultiplied, need to unpremultiply
            let a = data[offset + 3] as f32 / 255.0;
            if a > 0.0 {
                *pixel = Rgba([
                    (data[offset] as f32 / a).min(255.0) as u8,
                    (data[offset + 1] as f32 / a).min(255.0) as u8,
                    (data[offset + 2] as f32 / a).min(255.0) as u8,
                    data[offset + 3],
                ]);
            } else {
                *pixel = Rgba([255, 255, 255, 255]); // White for transparent
            }
        }

        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, ImageFormat::Png)
            .expect("Failed to write PNG");
        buffer.into_inner()
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_canvas() {
        let canvas = Canvas::new(900, 600);
        assert_eq!(canvas.width(), 900);
        assert_eq!(canvas.height(), 600);
    }

    #[test]
    fn test_draw_line() {
        let mut canvas = Canvas::new(100, 100);
        let msg = DrawMessage::new(DrawType::Line, (255, 0, 0, 255), 2.0, (10.0, 10.0), (50.0, 50.0));
        canvas.draw(&msg);

        let png = canvas.to_png();
        assert!(!png.is_empty());
    }

    #[test]
    fn test_export_png() {
        let canvas = Canvas::new(100, 100);
        let png = canvas.to_png();

        // PNG magic bytes
        assert_eq!(&png[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    }
}
