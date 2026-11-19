//! Preserve OS event ordering, including complete gestures/modifier chords that
//! arrive between rendered frames. Rendering cadence never determines input loss.
use super::{
    brush_orbit::BrushPress,
    files::Jobs,
    state::{Action, Studio, Tool},
    ui::{self, Hit, ScrollArea, TextCard},
};
use bevy::{
    ecs::{query::QueryData, system::SystemParam},
    input::{ButtonState, mouse::MouseScrollUnit},
    prelude::*,
    window::{WindowCloseRequested, WindowEvent},
    winit::RawWinitWindowEvent,
};

use std::time::Duration;

mod shortcuts;

#[cfg(target_os = "macos")]
const SHORTCUT_KEYS: [KeyCode; 2] = [KeyCode::SuperLeft, KeyCode::SuperRight];
#[cfg(not(target_os = "macos"))]
const SHORTCUT_KEYS: [KeyCode; 2] = [KeyCode::ControlLeft, KeyCode::ControlRight];

use shortcuts::key_down;

pub(super) fn shortcut_hint(label: &str, key: &str, shift: bool) -> String {
    let modifier = match (cfg!(target_os = "macos"), shift) {
        (true, false) => "⌘",
        (true, true) => "⇧⌘",
        (false, false) => "Ctrl+",
        (false, true) => "Ctrl+Shift+",
    };
    format!("{label} · {modifier}{key}")
}

#[derive(Default)]
pub struct Gesture {
    position: Option<Vec2>,
    left: bool,
    pressed: Option<Action>,
    select_clicks: SelectClicks,
    brush_press: Option<BrushPress>,
    native_modifiers: winit::keyboard::ModifiersState,
    keys: ButtonInput<KeyCode>,
}
#[derive(Default)]
struct SelectClicks {
    press: Option<Vec2>,
    previous: Option<(Vec2, Duration)>,
}

impl SelectClicks {
    const SLOP_SQUARED: f32 = 25.;

    fn begin(&mut self, point: Option<Vec2>) {
        self.press = point;
        if point.is_none() {
            self.previous = None;
        }
    }

    fn moved(&mut self, point: Vec2) {
        if self
            .press
            .is_some_and(|start| start.distance_squared(point) > Self::SLOP_SQUARED)
        {
            *self = Self::default();
        }
    }

    fn release(&mut self, point: Vec2, now: Duration) -> bool {
        let press = self.press.take();
        let previous = self.previous.take();
        if press.is_none_or(|start| start.distance_squared(point) > Self::SLOP_SQUARED) {
            return false;
        }
        if previous.is_some_and(|(last, time)| {
            last.distance_squared(point) <= Self::SLOP_SQUARED
                && now.saturating_sub(time) <= double_click_interval()
        }) {
            return true;
        }
        self.previous = Some((point, now));
        false
    }
}

fn double_click_interval() -> Duration {
    #[cfg(target_os = "macos")]
    if let Ok(interval) = Duration::try_from_secs_f64(objc2_app_kit::NSEvent::doubleClickInterval())
    {
        return interval;
    }
    Duration::from_millis(500)
}

#[derive(SystemParam)]
pub struct NativeInput<'w, 's> {
    window: Single<'w, 's, (Entity, &'static Window)>,
    commands: Commands<'w, 's>,
    cursor: Local<'s, Option<bevy::window::SystemCursorIcon>>,
    events: MessageReader<'w, 's, WindowEvent>,
    native_events: MessageReader<'w, 's, RawWinitWindowEvent>,
    gesture: Local<'s, Gesture>,
    offsets: ResMut<'w, ui::ScrollOffsets>,
    time: Res<'w, Time<Real>>,
}
#[derive(QueryData)]
pub struct HitRegion {
    hit: &'static Hit,
    node: &'static ComputedNode,
    transform: &'static UiGlobalTransform,
    clip: Option<&'static CalculatedClip>,
    card: Option<&'static TextCard>,
}
#[derive(QueryData)]
#[query_data(mutable)]
pub struct ScrollRegion {
    node: &'static ComputedNode,
    transform: &'static UiGlobalTransform,
    clip: Option<&'static CalculatedClip>,
    offset: &'static mut ScrollPosition,
    kind: &'static ScrollArea,
}
pub fn dispatch(
    input: NativeInput,
    mut studio: ResMut<Studio>,
    mut jobs: ResMut<Jobs>,
    mut replay: ResMut<super::replay::Replay>,
    hits: Query<HitRegion>,
    mut scroll: Query<ScrollRegion>,
) {
    let NativeInput {
        window,
        mut commands,
        mut cursor,
        mut events,
        mut native_events,
        mut gesture,
        mut offsets,
        time,
    } = input;
    let now = time.elapsed();
    let (window_entity, window) = *window;
    let at = |point| {
        hits.iter()
            .filter(|region| ui::contains(point, region.node, region.transform, region.clip))
            .max_by_key(|region| region.hit.order)
    };
    // Bevy forwards the click but drops Winit's modifier snapshots. Modifier
    // changes can arrive on the click itself, and both edges can share a frame.
    let mut click_modifiers =
        modifiers_at_left_presses(native_events.read(), &mut gesture.native_modifiers).into_iter();
    for event in events.read() {
        match event {
            WindowEvent::Ime(bevy::window::Ime::Commit { window, value })
                if *window == window_entity && studio.replay_view.is_none() =>
            {
                studio.text_insert(value);
            }
            WindowEvent::Ime(bevy::window::Ime::Preedit { window, value, .. })
                if *window == window_entity && studio.replay_view.is_none() =>
            {
                studio.text_preedit(value);
            }
            WindowEvent::KeyboardInput(event) if event.window == window_entity => {
                if event.state == ButtonState::Pressed {
                    gesture.select_clicks = SelectClicks::default();
                    gesture.keys.press(event.key_code);
                    studio.text.snap = gesture
                        .keys
                        .any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
                    let command = gesture.keys.any_pressed(SHORTCUT_KEYS);
                    let shift = gesture
                        .keys
                        .any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
                    if studio.replay_view.is_none() && studio.text_key(event, command, shift) {
                        continue;
                    }
                    if event.key_code == KeyCode::Escape {
                        gesture.brush_press = None;
                        if studio.replay_view.is_some() {
                            gesture.left = false;
                            gesture.pressed = None;
                        }
                    }
                    key_down(
                        event.key_code,
                        command,
                        shift,
                        &mut studio,
                        &mut jobs,
                        &mut replay,
                    );
                } else {
                    gesture.keys.release(event.key_code);
                    studio.text.snap = gesture
                        .keys
                        .any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
                }
            }
            WindowEvent::CursorMoved(event) if event.window == window_entity => {
                let point = event.position;
                gesture.select_clicks.moved(point);
                studio.text.snap = gesture
                    .keys
                    .any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
                gesture.position = Some(point);
                if studio.replay_view.is_some() {
                    continue;
                }
                let blocked =
                    at(point).is_some() || studio.home || studio.installing || jobs.switching();
                elmapa::performance::input();
                if let Some(press) = gesture.brush_press.take_if(|press| press.dragged(point))
                    && press.valid(&studio)
                    && !jobs.switching()
                {
                    studio.down(press.start);
                }
                if gesture.left || studio.drag.is_some() {
                    studio.moved(point, blocked);
                } else {
                    studio.pointer = (!blocked).then_some(point);
                }
            }
            WindowEvent::MouseButtonInput(event)
                if event.window == window_entity && event.button == MouseButton::Middle =>
            {
                gesture.select_clicks = SelectClicks::default();
                if event.state == ButtonState::Released {
                    studio.drag = None;
                    continue;
                }
                let Some(point) = gesture.position.or_else(|| window.cursor_position()) else {
                    continue;
                };
                if at(point).is_none() && !jobs.switching() && studio.replay_view.is_none() {
                    elmapa::performance::input();
                    if let Some(press) = gesture.brush_press.take()
                        && press.valid(&studio)
                    {
                        studio.down(press.start);
                    }
                    studio.begin_pan(point);
                    if studio.drag.is_some() {
                        gesture.left = false;
                        gesture.pressed = None;
                    }
                }
            }
            WindowEvent::MouseButtonInput(event)
                if event.window == window_entity && event.button == MouseButton::Left =>
            {
                let modifiers = (event.state == ButtonState::Pressed)
                    .then(|| click_modifiers.next())
                    .flatten();
                if studio.drag.is_some() {
                    continue;
                }
                let Some(point) = gesture.position.or_else(|| window.cursor_position()) else {
                    continue;
                };
                let target = at(point);
                let action = target.as_ref().and_then(|region| region.hit.action);
                if studio.replay_view.is_some() {
                    if event.state == ButtonState::Pressed {
                        gesture.brush_press = None;
                        gesture.pressed = action.filter(|action| {
                            matches!(action, Action::ReplayView | Action::SaveReplayVideo)
                        });
                    } else {
                        gesture.left = false;
                        if let Some(pressed) = gesture.pressed.take()
                            && Some(pressed) == action
                        {
                            studio.act(pressed, &mut jobs);
                        }
                    }
                    continue;
                }
                let blocked =
                    target.is_some() || studio.home || studio.installing || jobs.switching();
                elmapa::performance::input();
                if event.state == ButtonState::Pressed {
                    let alt = modifiers.map_or_else(
                        || {
                            gesture
                                .keys
                                .any_pressed([KeyCode::AltLeft, KeyCode::AltRight])
                        },
                        |keys| keys.alt_key(),
                    );
                    let shift = modifiers.map_or_else(
                        || {
                            gesture
                                .keys
                                .any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
                        },
                        |keys| keys.shift_key(),
                    );
                    gesture.left = true;
                    gesture.pressed = action;
                    let mut empty_select = None;
                    if studio.text.draft.is_none()
                        && action.is_none()
                        && target.is_some_and(|region| region.card.is_none())
                    {
                        studio.commit_text_field();
                        studio.dismiss_text_control();
                    }
                    if !blocked && !studio.close_brush_orbit() {
                        if !studio.can_open_brush_orbit() {
                            let can_start_text = studio.tool == Tool::Select
                                && studio.text.draft.is_none()
                                && studio.opening.is_none()
                                && !studio.menu;
                            studio.down_with_shift(point, shift);
                            if can_start_text
                                && studio.text.selected.is_none()
                                && studio.landmarks.selected.is_none()
                                && studio.path_selection.is_none()
                            {
                                empty_select = Some(point);
                            }
                        } else if alt {
                            studio.open_brush_orbit(point);
                        } else {
                            gesture.brush_press = Some(BrushPress::new(point, studio.brush, now));
                        }
                    }
                    gesture.select_clicks.begin(empty_select);
                } else {
                    gesture.left = false;
                    if let Some(press) = gesture.brush_press.take()
                        && press.valid(&studio)
                        && !jobs.switching()
                    {
                        if !blocked && !press.dragged(point) && press.held(now) {
                            studio.open_brush_orbit(press.start);
                        } else {
                            studio.down(press.start);
                        }
                    }
                    studio.up(point, blocked);
                    if blocked || studio.tool != Tool::Select || studio.text.draft.is_some() {
                        gesture.select_clicks = SelectClicks::default();
                    } else if gesture.select_clicks.release(point, now) {
                        let position = studio.map_point(point);
                        studio.place_text(position);
                    }
                    if let Some(action) = gesture.pressed.take()
                        && let Some(HitRegionItem {
                            hit,
                            node,
                            transform,
                            ..
                        }) = target
                        && hit.action == Some(action)
                    {
                        if matches!(action, Action::Enter(_)) {
                            let rect = Rect::from_center_size(
                                transform.translation * node.inverse_scale_factor(),
                                node.size() * node.inverse_scale_factor(),
                            );
                            let compact =
                                studio.bounds.size.x < 1100. || studio.bounds.size.y < 760.;
                            studio.opening_from = Rect::from_corners(
                                rect.min + Vec2::splat(10.),
                                rect.min
                                    + Vec2::new(
                                        rect.width() - 10.,
                                        10. + if compact { 190. } else { 232. },
                                    ),
                            );
                        }
                        studio.act(action, &mut jobs);
                    }
                }
            }
            WindowEvent::MouseWheel(event) if event.window == window_entity => {
                gesture.select_clicks = SelectClicks::default();
                if studio.replay_view.is_some() {
                    continue;
                }
                let Some(point) = gesture.position.or_else(|| window.cursor_position()) else {
                    continue;
                };
                let delta = Vec2::new(event.x, event.y)
                    * if event.unit == MouseScrollUnit::Line {
                        20.
                    } else {
                        1.
                    };
                let mut scrolled = false;
                for item in &mut scroll {
                    let ScrollRegionItem {
                        node,
                        transform,
                        clip,
                        mut offset,
                        kind,
                    } = item;
                    if studio.text.style_open
                        || studio.settings && !matches!(kind, ScrollArea::Packs)
                    {
                        continue;
                    }
                    if ui::contains(point, node, transform, clip) {
                        let max =
                            (node.content_size().y - node.size().y) * node.inverse_scale_factor();
                        offset.y = (offset.y - delta.y).clamp(0., max.max(0.));
                        match kind {
                            ScrollArea::Shelf => offsets.shelf = offset.y,
                            ScrollArea::Layers => offsets.layers = offset.y,
                            ScrollArea::Packs => offsets.packs = offset.y,
                        }
                        scrolled = true;
                    }
                }
                if scrolled
                    || at(point).is_some()
                    || studio.home
                    || studio.installing
                    || jobs.switching()
                {
                    continue;
                }
                elmapa::performance::input();
                gesture.brush_press = None;
                studio.close_brush_orbit();
                let bounds = studio.bounds;
                studio
                    .viewport
                    .zoom_at(point, (delta.y * 0.005).exp(), bounds);
            }
            WindowEvent::CursorLeft(event) if event.window == window_entity => {
                gesture.position = None;
                gesture.select_clicks = SelectClicks::default();
                gesture.brush_press = None;
                studio.pointer = None;
            }
            WindowEvent::WindowFocused(event)
                if event.window == window_entity && !event.focused =>
            {
                studio.finish();
                studio.regions.cancel_gesture();
                if studio.text.draft.is_none() {
                    studio.commit_text_field();
                }
                gesture.left = false;
                gesture.pressed = None;
                gesture.select_clicks = SelectClicks::default();
                gesture.brush_press = None;
                studio.close_brush_orbit();
                gesture.keys.reset_all();
                studio.pointer = None;
            }
            _ => {}
        }
    }
    gesture
        .brush_press
        .take_if(|press| !press.valid(&studio) || jobs.switching() || studio.replay_view.is_some());
    if gesture.position.is_none_or(|point| at(point).is_none())
        && let Some(press) = gesture.brush_press.take_if(|press| press.held(now))
    {
        studio.open_brush_orbit(press.start);
    }
    let Some(point) = gesture.position.or_else(|| window.cursor_position()) else {
        return;
    };
    let hit = at(point);
    let blocked = hit.is_some()
        || studio.replay_view.is_some()
        || studio.home
        || studio.installing
        || jobs.switching()
        || studio.brush_orbit.anchor.is_some();
    let map_point = studio.map_point(point);
    let icon = if studio.drag.is_some() || studio.path_dragging() {
        bevy::window::SystemCursorIcon::Grabbing
    } else if hit.is_some_and(|region| region.hit.action.is_some()) {
        bevy::window::SystemCursorIcon::Pointer
    } else if !blocked && studio.regions.active {
        if studio.regions.is_dragging() {
            bevy::window::SystemCursorIcon::Grabbing
        } else if studio.region_border_at(map_point) {
            bevy::window::SystemCursorIcon::Grab
        } else {
            bevy::window::SystemCursorIcon::Crosshair
        }
    } else if !blocked && studio.landmarks.moving.is_some() {
        bevy::window::SystemCursorIcon::Grabbing
    } else if !blocked && (studio.tool == Tool::Text || studio.text.draft.is_some()) {
        bevy::window::SystemCursorIcon::Text
    } else if !blocked && studio.tool == Tool::Select {
        if studio.text.transforming.is_some() {
            bevy::window::SystemCursorIcon::Grabbing
        } else if let Some(handle) = studio.text_handle(map_point) {
            match handle {
                super::lettering::Handle::Rotate => bevy::window::SystemCursorIcon::Grab,
                super::lettering::Handle::Corner(x, y) => {
                    let angle = (studio.current_label().rotation
                        + if x * y > 0. { 45. } else { -45. })
                    .rem_euclid(180.);
                    if !(22.5..157.5).contains(&angle) {
                        bevy::window::SystemCursorIcon::EwResize
                    } else if angle < 67.5 {
                        bevy::window::SystemCursorIcon::NwseResize
                    } else if angle < 112.5 {
                        bevy::window::SystemCursorIcon::NsResize
                    } else {
                        bevy::window::SystemCursorIcon::NeswResize
                    }
                }
            }
        } else if studio.text.moving.is_some() {
            bevy::window::SystemCursorIcon::Grabbing
        } else if studio.path_node_at(map_point).is_some()
            || studio
                .editor
                .label_at(map_point, 6. / studio.viewport.zoom)
                .is_some()
            || studio
                .editor
                .landmark_at(map_point, 4. / studio.viewport.zoom)
                .is_some()
        {
            bevy::window::SystemCursorIcon::Grab
        } else {
            bevy::window::SystemCursorIcon::Default
        }
    } else if !blocked {
        bevy::window::SystemCursorIcon::Crosshair
    } else {
        bevy::window::SystemCursorIcon::Default
    };
    if *cursor != Some(icon) {
        commands
            .entity(window_entity)
            .insert(bevy::window::CursorIcon::System(icon));
        *cursor = Some(icon);
    }
}
fn modifiers_at_left_presses<'a>(
    events: impl Iterator<Item = &'a RawWinitWindowEvent>,
    held: &mut winit::keyboard::ModifiersState,
) -> Vec<winit::keyboard::ModifiersState> {
    use winit::event::{ElementState, MouseButton, WindowEvent};
    events
        .filter_map(|event| match &event.event {
            WindowEvent::ModifiersChanged(modifiers) => {
                *held = modifiers.state();
                None
            }
            WindowEvent::Focused(false) => {
                *held = winit::keyboard::ModifiersState::empty();
                None
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => Some(*held),
            _ => None,
        })
        .collect()
}

pub fn close(
    mut close: MessageReader<WindowCloseRequested>,
    mut studio: ResMut<Studio>,
    mut jobs: ResMut<Jobs>,
) {
    if close.read().next().is_some() {
        studio.act(Action::Quit, &mut jobs);
    }
}

#[cfg(test)]
mod tests;
