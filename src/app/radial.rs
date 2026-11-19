//! Reusable action tree and clockwise orbit choreography, independent of text tools.
use bevy::prelude::*;

pub const RING_GAP: f32 = 46.;
const BUBBLE_STEP: f32 = 44.;
const RING_SWEEP: f32 = std::f32::consts::TAU - 0.55;

pub struct Item<A> {
    pub title: String,
    pub icon: String,
    pub action: Option<A>,
    pub children: Vec<Self>,
    pub active: bool,
    pub swatch: Option<u32>,
}
impl<A> Item<A> {
    pub fn action(title: impl Into<String>, icon: impl Into<String>, action: A) -> Self {
        Self {
            title: title.into(),
            icon: icon.into(),
            action: Some(action),
            children: Vec::new(),
            active: false,
            swatch: None,
        }
    }
    pub fn branch(title: impl Into<String>, icon: impl Into<String>, children: Vec<Self>) -> Self {
        Self {
            title: title.into(),
            icon: icon.into(),
            action: None,
            children,
            active: false,
            swatch: None,
        }
    }
}
#[derive(Default)]
pub struct Navigation {
    pub path: Vec<usize>,
}
impl Navigation {
    pub fn items<'a, A>(&self, root: &'a [Item<A>]) -> &'a [Item<A>] {
        let mut items = root;
        for &index in &self.path {
            let Some(item) = items.get(index) else {
                return root;
            };
            items = &item.children;
        }
        items
    }
    pub fn enter<A>(&mut self, root: &[Item<A>], index: usize) {
        if self
            .items(root)
            .get(index)
            .is_some_and(|i| !i.children.is_empty())
        {
            self.path.push(index);
        }
    }
    /// Clicking an ancestor opens or closes its own branch. Deeper rings are
    /// discarded, while all earlier ancestors remain visible.
    pub fn enter_at<A>(&mut self, root: &[Item<A>], depth: usize, index: usize) {
        if depth > self.path.len() {
            return;
        }
        let closing = self.path.get(depth) == Some(&index);
        self.path.truncate(depth);
        if !closing {
            self.enter(root, index);
        }
    }
    pub fn back(&mut self) {
        self.path.pop();
    }
    pub fn reset(&mut self) {
        self.path.clear();
    }
}

/// Pack bubbles from twelve o'clock with a modest eight-pixel gap. Walking
/// ellipse arc length keeps spacing consistent around wide or rotated targets;
/// spare circumference stays empty instead of distributing a few actions.
/// Large action sets continue on an outer ring without overlapping the first.
pub fn offset(index: usize, radius: Vec2, progress: f32) -> Vec2 {
    let mut distance = index as f32 * BUBBLE_STEP;
    let mut radius = radius;
    let mut angle = -std::f32::consts::FRAC_PI_2;
    let mut previous = Vec2::new(angle.cos(), angle.sin()) * radius;
    loop {
        let next_angle = angle + 0.012;
        let next = Vec2::new(next_angle.cos(), next_angle.sin()) * radius;
        let segment = next.distance(previous);
        if distance <= segment {
            angle += 0.012 * distance / segment;
            break;
        }
        distance -= segment;
        angle = next_angle;
        previous = next;
        if angle >= RING_SWEEP - std::f32::consts::FRAC_PI_2 {
            radius += Vec2::splat(RING_GAP);
            angle = -std::f32::consts::FRAC_PI_2;
            previous = Vec2::new(0., -radius.y);
        }
    }
    let angle = angle - (1. - progress) * 0.16;
    Vec2::new(angle.cos(), angle.sin()) * radius * (0.86 + 0.14 * progress)
}

/// A circle of this radius fits the last bubble before the wrap point.
pub fn single_ring_radius(bubbles: usize) -> f32 {
    (bubbles.saturating_sub(1) as f32 * BUBBLE_STEP / RING_SWEEP).ceil()
}

pub fn reveal(elapsed: f32, index: usize, reduced: bool) -> f32 {
    if reduced {
        return 1.;
    }
    let t = ((elapsed - index as f32 * 0.038) / 0.24).clamp(0., 1.);
    1. - (1. - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arbitrary_nested_actions_stay_hidden_until_entered_and_back_restores_parent() {
        let tree = vec![
            Item::branch(
                "More",
                "settings",
                vec![Item::branch(
                    "Deeper",
                    "settings",
                    vec![Item::action("Do", "plus", 42)],
                )],
            ),
            Item::action("Other", "plus", 7),
        ];
        let mut nav = Navigation::default();
        assert_eq!(nav.items(&tree).len(), 2);
        nav.enter(&tree, 1); // leaf cannot be opened
        assert!(nav.path.is_empty());
        nav.enter(&tree, 0);
        assert_eq!(nav.items(&tree)[0].title, "Deeper");
        nav.enter(&tree, 0);
        assert_eq!(nav.items(&tree)[0].action, Some(42));
        nav.back();
        assert_eq!(nav.items(&tree)[0].title, "Deeper");
        nav.enter_at(&tree, 1, 0);
        assert_eq!(nav.path, vec![0, 0]);
        nav.enter_at(&tree, 0, 0);
        assert!(nav.path.is_empty());
        nav.reset();
        assert_eq!(nav.items(&tree).len(), 2);
    }
    #[test]
    fn choreography_starts_at_top_and_staggers_clockwise_with_immediate_reduced_motion() {
        let r = Vec2::splat(80.);
        assert!(offset(0, r, 1.).distance(Vec2::new(0., -80.)) < 0.001);
        let second = offset(1, r, 1.);
        assert!(second.x > 0. && second.y < 0.);
        assert!((second.distance(offset(0, r, 1.)) - 44.).abs() < 1.);
        // The fifth action stays in a short arc; changing the number of
        // actions never redistributes the actions already in that arc.
        assert!(offset(4, r, 1.).x > 0.);
        assert!(reveal(0.08, 0, false) > reveal(0.08, 1, false));
        assert_eq!(reveal(0., 3, false), 0.);
        assert_eq!(reveal(0., 8, true), 1.);
        assert_eq!(reveal(1., 8, false), 1.);
    }
}
