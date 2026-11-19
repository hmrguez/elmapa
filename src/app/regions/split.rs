//! An editable subdivision held outside the document until Apply.
use super::Selection;
use elmapa::{
    document::{AtlasStyle, Point, RegionAddress, RegionLayer, RegionNode, RegionParent},
    scene::{
        Shape,
        territories::{RegionView, Shoreline, contains, split_land, visible_face},
    },
};

struct Preview {
    view: RegionView,
    washes: Vec<Shape>,
}

impl Preview {
    fn new(data: &RegionLayer, shore: &Shoreline, style: AtlasStyle) -> Option<Self> {
        let view = RegionView::resolve(data, shore)?;
        let washes = view.washes(shore, data, style);
        // Every drafted piece needs visible land, including after a shoreline edit.
        (washes.len() == view.faces.len()).then_some(Self { view, washes })
    }
}

#[derive(Clone, Copy)]
pub(super) struct ChildSplit {
    pub(super) parent: RegionParent,
    pub(super) anchor: Point,
}

pub(super) struct SplitDraft {
    style: AtlasStyle,
    pub(super) count: usize,
    pub(super) child: Option<ChildSplit>,
    pub(super) data: RegionLayer,
    pub(super) first_owner: usize,
    preview: Preview,
    moving: Option<(Vec<Point>, Preview)>,
}

impl SplitDraft {
    pub(super) fn new(
        selection: &Selection,
        source: Option<&RegionLayer>,
        count: usize,
        child: Option<ChildSplit>,
        style: AtlasStyle,
    ) -> Result<Self, String> {
        let (data, first_owner) = if let Some(child) = child {
            let piece = visible_face(&selection.polygon, &selection.shore)
                .into_iter()
                .find(|piece| contains(piece, child.anchor))
                .ok_or("Select visible land inside the parent region.")?;
            let mut data = split_land(&piece, Vec::new(), None, count)?;
            data.parent = Some(child.parent);
            (data, 0)
        } else {
            let names = source.map_or_else(Vec::new, |data| data.names.clone());
            let first_owner = selection.owner.unwrap_or(names.len());
            let mut data = split_land(&selection.polygon, names, selection.owner, count)?;
            if let Some(source) = source {
                data.styles.clone_from(&source.styles);
                data.parent = source.parent;
            }
            (data, first_owner)
        };
        let preview = Preview::new(&data, &selection.shore, style)
            .ok_or("The split needs visible land in every region.")?;
        Ok(Self {
            style,
            count,
            child,
            data,
            first_owner,
            preview,
            moving: None,
        })
    }

    pub(super) fn view(&self) -> &RegionView {
        &self.current().view
    }

    pub(super) fn washes(&self) -> &[Shape] {
        &self.current().washes
    }

    fn current(&self) -> &Preview {
        self.moving.as_ref().map_or(&self.preview, |(_, view)| view)
    }

    fn border_mut(&mut self, address: RegionAddress) -> Option<&mut Vec<Point>> {
        let RegionNode::Cut { path, .. } = self
            .data
            .areas
            .get_mut(address.area)?
            .nodes
            .get_mut(address.node)?
        else {
            return None;
        };
        Some(path)
    }

    pub(super) fn preview_move(
        &mut self,
        address: RegionAddress,
        path: &[Point],
        shore: &Shoreline,
    ) {
        if path.is_empty() {
            self.cancel_move();
            return;
        }
        if self.moving.as_ref().is_some_and(|(last, _)| last == path)
            || path.iter().any(|point| !point.is_in_world())
        {
            return;
        }
        let Some(border) = self.border_mut(address) else {
            return;
        };
        let original = std::mem::replace(border, path.to_vec());
        let preview = Preview::new(&self.data, shore, self.style);
        *self.border_mut(address).expect("draft border") = original;
        if let Some(preview) = preview {
            self.moving = Some((path.to_vec(), preview));
        }
    }

    pub(super) fn finish_move(&mut self, address: RegionAddress) {
        if let Some((path, preview)) = self.moving.take()
            && let Some(border) = self.border_mut(address)
        {
            *border = path;
            self.preview = preview;
        }
    }

    pub(super) fn cancel_move(&mut self) {
        self.moving = None;
    }

    /// Extend the current layer by replacing just the selected leaf with the draft tree.
    pub(super) fn apply_to(&self, target: &mut RegionLayer, selection: &Selection) -> Option<()> {
        let draft = self.data.areas.first()?;
        let address = selection
            .address
            .unwrap_or_else(|| target.claim(draft.land.clone(), self.first_owner));
        let area = target.areas.get_mut(address.area)?;
        if !matches!(area.nodes.get(address.node), Some(RegionNode::Owned(owner)) if *owner == self.first_owner)
        {
            return None;
        }
        let base = area.nodes.len();
        let mut nodes = draft.nodes.iter().cloned().map(|mut node| {
            if let RegionNode::Cut { children, .. } = &mut node {
                *children = children.map(|child| base + child - 1);
            }
            node
        });
        area.nodes[address.node] = nodes.next()?;
        area.nodes.extend(nodes);
        target.names.clone_from(&self.data.names);
        Some(())
    }
}
