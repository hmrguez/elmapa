use super::{
    AtlasStyle, ContentBounds, Point,
    limits::{
        MAX_LABEL_CURVATURE, MAX_LABEL_SIZE, MAX_LABEL_SPACING, MAX_LABEL_TEXT_BYTES,
        MIN_LABEL_SIZE, MIN_LABEL_SPACING,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelFont {
    #[default]
    Baskerville,
    Didot,
    Avenir,
}
impl LabelFont {
    pub const ALL: [Self; 3] = [Self::Baskerville, Self::Didot, Self::Avenir];
    pub fn family(self) -> &'static str {
        match self {
            Self::Baskerville => "Baskerville",
            Self::Didot => "Didot",
            Self::Avenir => "Avenir Next",
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelInk {
    #[default]
    Sepia,
    Charcoal,
    Blue,
    Parchment,
}
impl LabelInk {
    pub const ALL: [Self; 4] = [Self::Sepia, Self::Charcoal, Self::Blue, Self::Parchment];
    pub fn name(self) -> &'static str {
        match self {
            Self::Sepia => "Sepia",
            Self::Charcoal => "Charcoal",
            Self::Blue => "Blue",
            Self::Parchment => "Parchment",
        }
    }
    pub fn color_in(self, style: AtlasStyle) -> u32 {
        if style.is_classic() {
            return self.color();
        }
        match self {
            Self::Sepia => 0x5d4b3b,
            Self::Charcoal => 0x37352d,
            Self::Blue => 0x3a625e,
            Self::Parchment => 0xf3e8cd,
        }
    }

    pub fn color(self) -> u32 {
        match self {
            Self::Sepia => 0x554430,
            Self::Charcoal => 0x292b2b,
            Self::Blue => 0x355e72,
            Self::Parchment => 0xf6eed9,
        }
    }
}
/// Center-anchored, single-line map name. Size and spacing use world units.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub id: u64,
    pub text: String,
    pub position: Point,
    pub font: LabelFont,
    pub size: f32,
    pub rotation: f32,
    pub spacing: f32,
    /// Baseline sweep in degrees. Positive values arch upward.
    #[serde(default)]
    pub curvature: f32,
    pub ink: LabelInk,
    pub opacity: f32,
}
impl Default for Label {
    fn default() -> Self {
        Self {
            id: 0,
            text: "Place name".into(),
            position: Point::new(0., 0.),
            font: LabelFont::default(),
            size: 32.,
            rotation: 0.,
            spacing: 0.,
            curvature: 0.,
            ink: LabelInk::default(),
            opacity: 1.,
        }
    }
}
impl Label {
    pub fn validate(&self) -> Result<(), String> {
        if self.text.trim().is_empty()
            || self.text.len() > MAX_LABEL_TEXT_BYTES
            || self.text.chars().any(char::is_control)
            || !(MIN_LABEL_SIZE..=MAX_LABEL_SIZE).contains(&self.size)
            || !(-180. ..=180.).contains(&self.rotation)
            || !(MIN_LABEL_SPACING..=MAX_LABEL_SPACING).contains(&self.spacing)
            || !(-MAX_LABEL_CURVATURE..=MAX_LABEL_CURVATURE).contains(&self.curvature)
            || !(0. ..=1.).contains(&self.opacity)
            || !self.position.is_in_world()
        {
            return Err("A map label has invalid text or typography settings.".into());
        }
        Ok(())
    }
    pub fn bounds(&self) -> ContentBounds {
        crate::scene::label_shapes(self)
            .iter()
            .filter_map(crate::scene::Shape::bounds)
            .reduce(ContentBounds::union)
            .unwrap_or(ContentBounds {
                min: self.position,
                max: self.position,
            })
    }
    pub fn contains(&self, point: Point, tolerance: f32) -> bool {
        // Hit-test in unrotated label space so angled names have a precise target.
        let mut flat = self.clone();
        flat.position = Point::new(0., 0.);
        flat.rotation = 0.;
        let bounds = flat.bounds().padded(tolerance);
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        let x = point.x - self.position.x;
        let y = point.y - self.position.y;
        let p = Point::new(x * cos + y * sin, -x * sin + y * cos);
        p.x >= bounds.min.x && p.x <= bounds.max.x && p.y >= bounds.min.y && p.y <= bounds.max.y
    }
}
