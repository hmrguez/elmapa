//! Named regions and their shared cuts in map coordinates.
use super::{Point, limits::MAX_WORLD_COORDINATE};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type RegionPolygon = Vec<Vec<[f64; 2]>>;

pub const MAX_BORDER_POINTS: usize = 512;
const MAX_REGIONS: usize = 256;
const MAX_AREAS: usize = 2048;
const MAX_REGION_POINTS: usize = 200_000;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RegionLayer {
    pub names: Vec<String>,
    pub areas: Vec<RegionArea>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<usize, RegionStyle>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RegionStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegionArea {
    /// The assigned footprint survives coastline changes. The scene clips
    /// it to current land when drawing and hit testing.
    pub land: RegionPolygon,
    /// An arena rooted at zero. Children always follow their parent.
    pub nodes: Vec<RegionNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionNode {
    Owned(usize),
    Cut {
        path: Vec<Point>,
        children: [usize; 2],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionAddress {
    pub area: usize,
    pub node: usize,
}

impl RegionLayer {
    pub fn new_region(&mut self) -> usize {
        let owner = self.names.len();
        self.names.push(format!("Region {}", owner + 1));
        owner
    }

    pub fn claim(&mut self, land: RegionPolygon, owner: usize) -> RegionAddress {
        let area = self.areas.len();
        self.areas.push(RegionArea {
            land,
            nodes: vec![RegionNode::Owned(owner)],
        });
        RegionAddress { area, node: 0 }
    }

    pub fn split(&mut self, address: RegionAddress, path: Vec<Point>, new_owner: usize) -> bool {
        let Some(area) = self.areas.get_mut(address.area) else {
            return false;
        };
        let Some(&RegionNode::Owned(owner)) = area.nodes.get(address.node) else {
            return false;
        };
        let first = area.nodes.len();
        area.nodes[address.node] = RegionNode::Cut {
            path,
            children: [first, first + 1],
        };
        area.nodes
            .extend([RegionNode::Owned(owner), RegionNode::Owned(new_owner)]);
        true
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.names.len() > MAX_REGIONS || self.areas.len() > MAX_AREAS {
            return Err("This layer has reached its region limit.".into());
        }
        if self
            .names
            .iter()
            .any(|name| !super::limits::valid_name(name))
        {
            return Err("A region needs a valid name.".into());
        }
        if self.styles.iter().any(|(&owner, style)| {
            owner >= self.names.len() || style.color.is_some_and(|color| color > 0xffffff)
        }) {
            return Err("A region has an invalid style.".into());
        }
        let mut points = 0;
        for area in &self.areas {
            if area.land.is_empty() || area.nodes.is_empty() || area.nodes.len() > 2 * MAX_REGIONS {
                return Err("A region area has invalid geometry.".into());
            }
            for ring in &area.land {
                points += ring.len();
                if ring.len() < 3
                    || ring.iter().flatten().any(|q| {
                        !q.is_finite() || q.abs() > f64::from(MAX_WORLD_COORDINATE + 2048.)
                    })
                {
                    return Err("A region area has invalid coordinates.".into());
                }
            }
            let mut reached = vec![false; area.nodes.len()];
            reached[0] = true;
            for (index, node) in area.nodes.iter().enumerate() {
                if !reached[index] {
                    return Err("A region area has a disconnected border.".into());
                }
                match node {
                    RegionNode::Owned(owner) if *owner < self.names.len() => {}
                    RegionNode::Cut { path, children }
                        if (2..=MAX_BORDER_POINTS).contains(&path.len())
                            && path.iter().all(|point| point.is_in_world()) =>
                    {
                        points += path.len();
                        for &child in children {
                            if child <= index || child >= reached.len() || reached[child] {
                                return Err("A region border has invalid sides.".into());
                            }
                            reached[child] = true;
                        }
                    }
                    _ => return Err("A region has an invalid owner or border.".into()),
                }
            }
        }
        if points > MAX_REGION_POINTS {
            return Err("This layer has reached its region geometry limit.".into());
        }
        Ok(())
    }

    pub(crate) fn bytes(&self) -> usize {
        self.names.iter().map(String::capacity).sum::<usize>()
            + self.styles.len() * size_of::<(usize, RegionStyle)>()
            + self
                .areas
                .iter()
                .map(|area| {
                    area.land
                        .iter()
                        .map(|ring| ring.capacity() * size_of::<[f64; 2]>())
                        .sum::<usize>()
                        + area.nodes.capacity() * size_of::<RegionNode>()
                        + area
                            .nodes
                            .iter()
                            .filter_map(|node| match node {
                                RegionNode::Cut { path, .. } => {
                                    Some(path.capacity() * size_of::<Point>())
                                }
                                RegionNode::Owned(_) => None,
                            })
                            .sum::<usize>()
                })
                .sum::<usize>()
    }
}
