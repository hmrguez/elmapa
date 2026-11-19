//! Measurement-only Metal drawable observer. No renderer fork or altered GPU work.
//! The hook is process-local and installed before Bevy starts any render threads.
use elmapa::performance::{FrameEvent, FrameStage};
use objc2::{
    runtime::{AnyClass, AnyObject, Imp, ProtocolObject, Sel},
    sel,
};
use objc2_metal::MTLDrawable;
use std::{
    ptr::NonNull,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
static ORIGINAL: OnceLock<Imp> = OnceLock::new();
static FRAME: AtomicU64 = AtomicU64::new(1);

pub fn observe_presentations() {
    if !elmapa::performance::enabled() {
        return;
    }
    let Some(class) = AnyClass::get(c"CAMetalLayer") else {
        eprintln!("Metal presentation tracing unavailable: CAMetalLayer is not loaded");
        return;
    };
    let Some(method) = class.instance_method(sel!(nextDrawable)) else {
        return;
    };
    if ORIGINAL.set(method.implementation()).is_err() {
        return;
    }
    // SAFETY: nextDrawable has the exact Objective-C ABI implemented below. The
    // original IMP is saved before replacement and called exactly once per call.
    unsafe {
        method.set_implementation(std::mem::transmute::<
            unsafe extern "C-unwind" fn(
                *mut AnyObject,
                Sel,
            ) -> *mut ProtocolObject<dyn MTLDrawable>,
            Imp,
        >(next_drawable));
    }
}
unsafe extern "C-unwind" fn next_drawable(
    layer: *mut AnyObject,
    selector: Sel,
) -> *mut ProtocolObject<dyn MTLDrawable> {
    // SAFETY: the recorded IMP belongs to this selector and preserves its ABI.
    let original = unsafe {
        std::mem::transmute::<
            Imp,
            unsafe extern "C-unwind" fn(
                *mut AnyObject,
                Sel,
            ) -> *mut ProtocolObject<dyn MTLDrawable>,
        >(*ORIGINAL.get().expect("saved nextDrawable"))
    };
    let drawable = unsafe { original(layer, selector) };
    if let Some(pointer) = NonNull::new(drawable) {
        let id = FRAME.fetch_add(1, Ordering::Relaxed);
        let started = Instant::now();
        elmapa::performance::native_frame(FrameEvent {
            id,
            started,
            stage: FrameStage::Started,
        });
        let callback =
            block2::RcBlock::new(move |drawable: NonNull<ProtocolObject<dyn MTLDrawable>>| {
                // SAFETY: Metal supplies a live drawable for the duration of its callback.
                let seconds = unsafe { drawable.as_ref() }.presentedTime();
                elmapa::performance::native_frame(FrameEvent {
                    id,
                    started,
                    stage: FrameStage::Presented { seconds },
                });
            });
        // SAFETY: addPresentedHandler copies the block; Metal owns its captured
        // scalar values after this stack frame exits. No drawable is retained.
        unsafe {
            pointer
                .as_ref()
                .addPresentedHandler(&*callback as *const _ as *mut _);
        }
    }
    drawable
}

static COMMANDS: std::sync::Mutex<std::collections::VecDeque<(u8, bool)>> =
    std::sync::Mutex::new(std::collections::VecDeque::new());
fn queue_command(command: u8) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSEventType};

    let keyboard = command >= 2
        || MainThreadMarker::new()
            .and_then(|mtm| NSApplication::sharedApplication(mtm).currentEvent())
            .is_some_and(|event| {
                matches!(event.r#type(), NSEventType::KeyDown | NSEventType::KeyUp)
            });
    let mut commands = COMMANDS.lock().unwrap_or_else(|e| e.into_inner());
    if commands.len() < 64 {
        commands.push_back((command, keyboard));
    }
}
thread_local! {static MENU_TARGET:std::cell::RefCell<Option<objc2::rc::Retained<AnyObject>>>=const {std::cell::RefCell::new(None)};}
extern "C-unwind" fn quit_action(_: *mut AnyObject, _: Sel, _: *mut AnyObject) {
    queue_command(1);
}
extern "C-unwind" fn settings_action(_: *mut AnyObject, _: Sel, _: *mut AnyObject) {
    queue_command(0);
}
extern "C-unwind" fn tagged_action(_: *mut AnyObject, _: Sel, sender: *mut AnyObject) {
    if sender.is_null() {
        return;
    }
    // SAFETY: this action is installed only on the NSMenuItems created below.
    let tag: isize = unsafe { objc2::msg_send![sender, tag] };
    if let Ok(tag) = u8::try_from(tag) {
        queue_command(tag);
    }
}
/// Native menu actions enter the same checkpointing path as keyboard/window close.
pub fn setup_menu(_: bevy::ecs::system::NonSendMarker) {
    use objc2::{
        ClassType, MainThreadMarker, MainThreadOnly, msg_send, rc::Retained, runtime::ClassBuilder,
    };
    use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
    use objc2_foundation::{NSObject, ns_string};
    let mtm = MainThreadMarker::new().expect("menu on main thread");
    let mut builder =
        ClassBuilder::new(c"ElmapaMenuTarget", NSObject::class()).expect("menu class");
    // SAFETY: selectors have the conventional one-object NSMenu action signature.
    unsafe {
        builder.add_method(
            sel!(elmapaQuit:),
            quit_action as extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject),
        );
        builder.add_method(
            sel!(elmapaSettings:),
            settings_action as extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject),
        );
        builder.add_method(
            sel!(elmapaCommand:),
            tagged_action as extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject),
        );
    }
    let class = builder.register();
    // SAFETY: the registered class inherits NSObject's initializer.
    let target: Retained<AnyObject> = unsafe { msg_send![class, new] };
    let menu = NSMenu::new(mtm);
    let submenu = NSMenu::new(mtm);
    for (title, selector, key) in [
        (
            ns_string!("Settings…"),
            sel!(elmapaSettings:),
            ns_string!(","),
        ),
        (
            ns_string!("Quit Elmapa"),
            sel!(elmapaQuit:),
            ns_string!("q"),
        ),
    ] {
        // SAFETY: the target implements these selectors and remains retained for
        // the process lifetime; NSMenuItem does not retain its target.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                title,
                Some(selector),
                key,
            )
        };
        unsafe {
            item.setTarget(Some(&target));
        }
        submenu.addItem(&item);
        if key == ns_string!(",") {
            submenu.addItem(&NSMenuItem::separatorItem(mtm));
        }
    }
    // Cocoa dispatches Command equivalents through NSMenu before ordinary view
    // key events. Hidden accelerator entries preserve the existing visible menu
    // while making both physical and accessibility-generated chords reliable.
    for (title, key, tag, shift) in [
        (ns_string!("Save"), ns_string!("s"), 2, false),
        (ns_string!("Open"), ns_string!("o"), 3, false),
        (ns_string!("Export SVG"), ns_string!("e"), 4, false),
        (ns_string!("Undo"), ns_string!("z"), 5, false),
        (ns_string!("Redo"), ns_string!("z"), 6, true),
        (ns_string!("Fit artwork"), ns_string!("0"), 7, false),
        (ns_string!("New atlas"), ns_string!("n"), 8, false),
        (ns_string!("Atlases"), ns_string!("h"), 9, true),
        (ns_string!("Select all text"), ns_string!("a"), 10, false),
        (ns_string!("Copy text"), ns_string!("c"), 11, false),
        (ns_string!("Cut text"), ns_string!("x"), 12, false),
        (ns_string!("Paste text"), ns_string!("v"), 13, false),
        (ns_string!("Duplicate label"), ns_string!("d"), 14, false),
        (ns_string!("Replay this view"), ns_string!("r"), 15, true),
    ] {
        // SAFETY: the retained target implements this menu action signature.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                title,
                Some(sel!(elmapaCommand:)),
                key,
            )
        };
        unsafe {
            item.setTarget(Some(&target));
        }
        item.setTag(tag);
        item.setHidden(true);
        item.setAllowsKeyEquivalentWhenHidden(true);
        item.setKeyEquivalentModifierMask(
            NSEventModifierFlags::Command
                | if shift {
                    NSEventModifierFlags::Shift
                } else {
                    NSEventModifierFlags::empty()
                },
        );
        submenu.addItem(&item);
    }
    let root = NSMenuItem::new(mtm);
    root.setTitle(ns_string!("Elmapa"));
    root.setSubmenu(Some(&submenu));
    menu.addItem(&root);
    NSApplication::sharedApplication(mtm).setMainMenu(Some(&menu));
    MENU_TARGET.with(|keep| *keep.borrow_mut() = Some(target));
}
pub fn menu_actions(
    mut studio: bevy::prelude::ResMut<super::state::Studio>,
    mut jobs: bevy::prelude::ResMut<super::files::Jobs>,
) {
    use super::state::Action;
    let commands: Vec<_> = COMMANDS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect();
    for (command, keyboard) in commands {
        if studio.replay_view.is_some() && !matches!(command, 1 | 2 | 15) {
            continue;
        }
        if (10..=14).contains(&command) {
            use bevy::prelude::KeyCode;
            let key = match command {
                10 => KeyCode::KeyA,
                11 => KeyCode::KeyC,
                12 => KeyCode::KeyX,
                13 => KeyCode::KeyV,
                _ => KeyCode::KeyD,
            };
            if !studio.text_shortcut(key)
                && command == 14
                && studio.landmarks.selected.is_some()
                && !studio.home
            {
                studio.act(Action::LandmarkDuplicate, &mut jobs);
            }
            continue;
        }
        if keyboard && studio.replay_view.is_none() && studio.selected_label().is_some() {
            continue;
        }
        let action = match command {
            0 => Action::Settings,
            1 => Action::Quit,
            2 if !studio.home => Action::Save,
            3 if !studio.home || !studio.settings => Action::Open,
            4 if !studio.home => Action::Export,
            5 if !studio.home => Action::Undo,
            6 if !studio.home => Action::Redo,
            7 if !studio.home => Action::Fit,
            8 if studio.home && !studio.settings => Action::Blank,
            9 if !studio.home => Action::Home,
            15 if !studio.home => Action::ReplayView,
            _ => continue,
        };
        studio.act(action, &mut jobs);
    }
}
