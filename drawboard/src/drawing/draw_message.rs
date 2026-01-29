use crate::error::DrawboardError;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawType {
    Brush = 1,
    Line = 2,
    Rectangle = 3,
    Ellipse = 4,
}

impl TryFrom<i32> for DrawType {
    type Error = DrawboardError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(DrawType::Brush),
            2 => Ok(DrawType::Line),
            3 => Ok(DrawType::Rectangle),
            4 => Ok(DrawType::Ellipse),
            _ => Err(DrawboardError::InvalidDrawType(value)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DrawMessage {
    pub draw_type: DrawType,
    pub color_r: u8,
    pub color_g: u8,
    pub color_b: u8,
    pub color_a: u8,
    pub thickness: f64,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

impl DrawMessage {
    pub fn new(
        draw_type: DrawType,
        color: (u8, u8, u8, u8),
        thickness: f64,
        start: (f64, f64),
        end: (f64, f64),
    ) -> Self {
        Self {
            draw_type,
            color_r: color.0,
            color_g: color.1,
            color_b: color.2,
            color_a: color.3,
            thickness,
            x1: start.0,
            y1: start.1,
            x2: end.0,
            y2: end.1,
        }
    }

    /// Parse from string format: "type,R,G,B,A,thickness,x1,y1,x2,y2"
    pub fn parse(s: &str) -> Result<Self, DrawboardError> {
        let parts: Vec<&str> = s.split(',').collect();

        if parts.len() != 10 {
            return Err(DrawboardError::ParseError(format!(
                "Expected 10 elements, got {}",
                parts.len()
            )));
        }

        let draw_type: DrawType = parts[0]
            .parse::<i32>()
            .map_err(|_| DrawboardError::ParseError("Invalid type".into()))?
            .try_into()?;

        let color_r = parts[1]
            .parse::<i32>()
            .map_err(|_| DrawboardError::ParseError("Invalid colorR".into()))?
            as u8;
        let color_g = parts[2]
            .parse::<i32>()
            .map_err(|_| DrawboardError::ParseError("Invalid colorG".into()))?
            as u8;
        let color_b = parts[3]
            .parse::<i32>()
            .map_err(|_| DrawboardError::ParseError("Invalid colorB".into()))?
            as u8;
        let color_a = parts[4]
            .parse::<i32>()
            .map_err(|_| DrawboardError::ParseError("Invalid colorA".into()))?
            as u8;

        let thickness = parts[5]
            .parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid thickness".into()))?;

        if thickness < 0.0 || thickness > 100.0 || thickness.is_nan() {
            return Err(DrawboardError::ParseError(format!(
                "Thickness out of range: {}",
                thickness
            )));
        }

        let x1 = parts[6]
            .parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid x1".into()))?;
        let y1 = parts[7]
            .parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid y1".into()))?;
        let x2 = parts[8]
            .parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid x2".into()))?;
        let y2 = parts[9]
            .parse::<f64>()
            .map_err(|_| DrawboardError::ParseError("Invalid y2".into()))?;

        // Validate coordinates
        for coord in [x1, y1, x2, y2] {
            if coord.is_nan() {
                return Err(DrawboardError::ParseError("NaN coordinate".into()));
            }
        }

        Ok(Self {
            draw_type,
            color_r,
            color_g,
            color_b,
            color_a,
            thickness,
            x1,
            y1,
            x2,
            y2,
        })
    }
}

impl fmt::Display for DrawMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{},{},{},{},{},{},{},{},{}",
            self.draw_type as i32,
            self.color_r,
            self.color_g,
            self.color_b,
            self.color_a,
            self.thickness,
            self.x1,
            self.y1,
            self.x2,
            self.y2
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_message() {
        let msg = DrawMessage::parse("1,255,0,0,255,5.0,100.5,200.5,150.5,250.5").unwrap();
        assert_eq!(msg.draw_type, DrawType::Brush);
        assert_eq!(msg.color_r, 255);
        assert_eq!(msg.color_g, 0);
        assert_eq!(msg.color_b, 0);
        assert_eq!(msg.color_a, 255);
        assert_eq!(msg.thickness, 5.0);
        assert_eq!(msg.x1, 100.5);
        assert_eq!(msg.y1, 200.5);
        assert_eq!(msg.x2, 150.5);
        assert_eq!(msg.y2, 250.5);
    }

    #[test]
    fn test_parse_invalid_type() {
        let result = DrawMessage::parse("5,255,0,0,255,5.0,100,200,150,250");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_thickness() {
        let result = DrawMessage::parse("1,0,0,0,255,-1.0,0,0,10,10");
        assert!(result.is_err());

        let result = DrawMessage::parse("1,0,0,0,255,101.0,0,0,10,10");
        assert!(result.is_err());
    }

    #[test]
    fn test_roundtrip() {
        let original = DrawMessage::new(
            DrawType::Rectangle,
            (100, 150, 200, 128),
            10.0,
            (50.0, 60.0),
            (200.0, 300.0),
        );
        let serialized = original.to_string();
        let parsed = DrawMessage::parse(&serialized).unwrap();

        assert_eq!(parsed.draw_type, original.draw_type);
        assert_eq!(parsed.color_r, original.color_r);
        assert_eq!(parsed.thickness, original.thickness);
    }
}
