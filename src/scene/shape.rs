use crate::document::Point;

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Polygon {
        points: Vec<Point>,
        fill: u32,
        opacity: f32,
    },
    RegionSurface {
        contours: Vec<Vec<Point>>,
        material: Option<usize>,
        granulated: bool,
        fill: u32,
        opacity: f32,
    },
    Sprite {
        points: [Point; 4],
        material: usize,
        frame: u32,
        opacity: f32,
    },
    Line {
        points: Vec<Point>,
        color: u32,
        width: f32,
        opacity: f32,
    },
}

pub(super) fn polygon(out: &mut Vec<Shape>, points: Vec<Point>, fill: u32, opacity: f32) {
    if points.len() > 2 && super::generation::vertices(points.len() + 8) {
        out.push(Shape::Polygon {
            points,
            fill,
            opacity,
        });
    }
}
pub(super) fn line(out: &mut Vec<Shape>, points: Vec<Point>, color: u32, width: f32, opacity: f32) {
    if points.len() > 1 && super::generation::vertices(points.len() + 8) {
        out.push(Shape::Line {
            points,
            color,
            width,
            opacity,
        });
    }
}

impl Shape {
    pub fn points(&self) -> impl Iterator<Item = &Point> {
        let (points, contours): (&[Point], &[Vec<Point>]) = match self {
            Self::Polygon { points, .. } | Self::Line { points, .. } => (points, &[]),
            Self::Sprite { points, .. } => (points, &[]),
            Self::RegionSurface { contours, .. } => (&[], contours),
        };
        points.iter().chain(contours.iter().flatten())
    }

    pub fn texture_index(&self) -> Option<usize> {
        match self {
            Self::Sprite { material, .. } => Some(*material),
            Self::RegionSurface { material, .. } => *material,
            _ => None,
        }
    }

    pub fn granulated(&self) -> bool {
        matches!(
            self,
            Self::RegionSurface {
                granulated: true,
                ..
            }
        )
    }

    pub fn opacity_mut(&mut self) -> &mut f32 {
        match self {
            Self::Polygon { opacity, .. }
            | Self::Line { opacity, .. }
            | Self::Sprite { opacity, .. }
            | Self::RegionSurface { opacity, .. } => opacity,
        }
    }

    pub fn bounds(&self) -> Option<crate::document::ContentBounds> {
        use crate::document::ContentBounds;
        let margin = match self {
            Self::Line { width, .. } => width / 2.,
            _ => 0.,
        };
        let mut points = self.points();
        let first = *points.next()?;
        Some(
            points
                .copied()
                .fold(
                    ContentBounds {
                        min: first,
                        max: first,
                    },
                    |bounds, point| {
                        bounds.union(ContentBounds {
                            min: point,
                            max: point,
                        })
                    },
                )
                .padded(margin),
        )
    }
}
