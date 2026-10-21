//! Controller thread (SDL3 gamepad API).
//!
//! SDL3 is used instead of gilrs because gilrs' Windows backend
//! (Windows.Gaming.Input) only delivers input to the focused app, which would
//! make the exit combo useless while an emulator has focus. SDL reads XInput,
//! raw input and HIDAPI (native DualShock 4 / DualSense) and can keep
//! delivering events in the background.
//!
//! Poll rate: ~125 Hz while the app is focused (menu navigation), ~60 Hz while
//! a game runs (exit combo only), ~7 Hz when idle in the background.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sdl3::event::Event;
use sdl3::gamepad::{Axis, Button, Gamepad};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::logic::{Action, AxisLatch, Combo, Dedupe, Edge, Repeater};
use super::logic::{STICK_PRESS, STICK_RELEASE, TRIGGER_PRESS, TRIGGER_RELEASE};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExitCombo {
    /// Button names, see [`button_from_name`].
    pub buttons: Vec<String>,
    pub hold_ms: u64,
}

impl Default for ExitCombo {
    fn default() -> Self {
        Self { buttons: vec!["back".into(), "start".into()], hold_ms: 1000 }
    }
}

/// State shared between the input thread, the launcher and window events.
#[derive(Default)]
pub struct InputShared {
    pub app_focused: AtomicBool,
    pub game_running: AtomicBool,
    combo: Mutex<ExitCombo>,
    combo_changed: AtomicBool,
}

impl InputShared {
    pub fn set_combo(&self, combo: ExitCombo) {
        *self.combo.lock().unwrap_or_else(|e| e.into_inner()) = combo;
        self.combo_changed.store(true, Ordering::Release);
    }

    fn combo(&self) -> ExitCombo {
        self.combo.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

pub fn button_from_name(name: &str) -> Option<Button> {
    Some(match name {
        "south" => Button::South,
        "east" => Button::East,
        "west" => Button::West,
        "north" => Button::North,
        "back" => Button::Back,
        "guide" => Button::Guide,
        "start" => Button::Start,
        "leftStick" => Button::LeftStick,
        "rightStick" => Button::RightStick,
        "leftShoulder" => Button::LeftShoulder,
        "rightShoulder" => Button::RightShoulder,
        "dpadUp" => Button::DPadUp,
        "dpadDown" => Button::DPadDown,
        "dpadLeft" => Button::DPadLeft,
        "dpadRight" => Button::DPadRight,
        "touchpad" => Button::Touchpad,
        _ => return None,
    })
}

fn button_action(b: Button) -> Option<Action> {
    Some(match b {
        Button::DPadUp => Action::Up,
        Button::DPadDown => Action::Down,
        Button::DPadLeft => Action::Left,
        Button::DPadRight => Action::Right,
        Button::South => Action::Select,  // A / Cross
        Button::East => Action::Back,     // B / Circle
        Button::North => Action::Favorite, // Y / Triangle
        Button::West => Action::Secondary, // X / Square
        Button::LeftShoulder => Action::PrevPlatform,
        Button::RightShoulder => Action::NextPlatform,
        Button::Start => Action::Menu,
        Button::RightStick => Action::ToggleGroups,
        Button::LeftStick => Action::ToggleTv,
        Button::Back => Action::Filter, // Select / Share / View

        _ => return None,
    })
}

struct Pad {
    _handle: Gamepad, // kept open; dropping closes it
    held: HashSet<Button>,
    lx: AxisLatch,
    ly: AxisLatch,
    rx: AxisLatch,
    ry: AxisLatch,
    lt: AxisLatch,
    rt: AxisLatch,
    combo: Combo<Button>,
}

fn build_combo(c: &ExitCombo) -> Combo<Button> {
    let buttons = c.buttons.iter().filter_map(|n| button_from_name(n)).collect();
    Combo::new(buttons, Duration::from_millis(c.hold_ms.clamp(200, 5000)))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PadInfo {
    name: String,
    /// "playstation" | "xbox" | "nintendo" | "generic", for button glyphs.
    family: &'static str,
}

/// Starts the controller thread. `on_exit_combo` runs (on the input thread)
/// when the combo is held while a game is running.
pub fn spawn(app: AppHandle, shared: Arc<InputShared>, on_exit_combo: Arc<dyn Fn() + Send + Sync>) {
    let result = std::thread::Builder::new().name("gamepad".into()).spawn(move || {
        if let Err(e) = run(app, shared, on_exit_combo) {
            log::error!("controller input disabled: {e}");
        }
    });
    if let Err(e) = result {
        log::error!("could not start controller thread: {e}");
    }
}

fn run(app: AppHandle, shared: Arc<InputShared>, on_exit_combo: Arc<dyn Fn() + Send + Sync>) -> Result<(), String> {
    use sdl3::hint;
    // Keep receiving input while an emulator has focus.
    hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
    // Joystick message handling on SDL's own thread (Windows raw input).
    hint::set("SDL_JOYSTICK_THREAD", "1");
    // Don't switch PlayStation pads into enhanced report mode or touch their
    // LEDs: emulators may read the same pad and expect its default state.
    hint::set("SDL_JOYSTICK_ENHANCED_REPORTS", "0");
    hint::set("SDL_JOYSTICK_HIDAPI_PS4_REPORT_INTERVAL", "0");
    hint::set("SDL_JOYSTICK_HIDAPI_PS5_PLAYER_LED", "0");
    // libusb is only for exotic USB adapters; skipping it avoids probing
    // (and warning about) unrelated USB devices.
    hint::set("SDL_HIDAPI_LIBUSB", "0");

    let sdl = sdl3::init().map_err(|e| e.to_string())?;
    let gamepads = sdl.gamepad().map_err(|e| e.to_string())?;
    let mut pump = sdl.event_pump().map_err(|e| e.to_string())?;

    let mut combo_cfg = shared.combo();
    let mut pads: HashMap<u32, Pad> = HashMap::new();
    let mut repeater = Repeater::default();
    let mut dedupe: Dedupe<u32> = Dedupe::default();
    let mut nav_was_enabled = false;
    log::info!("controller thread started");

    loop {
        let running = shared.game_running.load(Ordering::Acquire);
        let nav_enabled = shared.app_focused.load(Ordering::Acquire) && !running;
        if nav_enabled != nav_was_enabled {
            repeater.release_all();
            nav_was_enabled = nav_enabled;
        }
        if shared.combo_changed.swap(false, Ordering::AcqRel) {
            combo_cfg = shared.combo();
            for p in pads.values_mut() {
                p.combo = build_combo(&combo_cfg);
            }
        }
        if !running {
            for p in pads.values_mut() {
                p.combo.reset();
            }
        }

        // Fires an action at the frontend (deduped across twin pads).
        let mut fire = |a: Action, which: u32, now: Instant, repeater: &mut Repeater| {
            if nav_enabled && dedupe.accept(a, which, now) {
                repeater.press(a, now);
                let _ = app.emit("nav", a.as_str());
            }
        };

        pump.pump_events();
        for event in pump.poll_iter() {
            let now = Instant::now();
            match event {
                Event::GamepadAdded { which, .. } => match gamepads.open(which.into()) {
                    Ok(handle) => {
                        let info = PadInfo { name: handle.name().unwrap_or_default(), family: family(&handle) };
                        log::info!("gamepad connected: {} ({})", info.name, info.family);
                        let _ = app.emit("gamepad-connected", &info);
                        pads.insert(
                            which.into(),
                            Pad {
                                _handle: handle,
                                held: HashSet::new(),
                                lx: AxisLatch::default(),
                                ly: AxisLatch::default(),
                                rx: AxisLatch::default(),
                                ry: AxisLatch::default(),
                                lt: AxisLatch::default(),
                                rt: AxisLatch::default(),
                                combo: build_combo(&combo_cfg),
                            },
                        );
                    }
                    Err(e) => log::warn!("couldn't open gamepad: {e}"),
                },
                Event::GamepadRemoved { which, .. } => {
                    pads.remove(&which.into());
                    repeater.release_all();
                    let _ = app.emit("gamepad-disconnected", pads.len());
                }
                Event::GamepadButtonDown { which, button, .. } => {
                    let id: u32 = which.into();
                    if let Some(p) = pads.get_mut(&id) {
                        p.held.insert(button);
                        p.combo.update(&p.held, now);
                    }
                    if let Some(a) = button_action(button) {
                        fire(a, id, now, &mut repeater);
                    }
                }
                Event::GamepadButtonUp { which, button, .. } => {
                    let id: u32 = which.into();
                    if let Some(p) = pads.get_mut(&id) {
                        p.held.remove(&button);
                        p.combo.update(&p.held, now);
                    }
                    if let Some(a) = button_action(button) {
                        repeater.release(a);
                    }
                }
                Event::GamepadAxisMotion { which, axis, value, .. } => {
                    let id: u32 = which.into();
                    let Some(p) = pads.get_mut(&id) else { continue };
                    let (latch, press, release, neg, pos) = match axis {
                        Axis::LeftX => (&mut p.lx, STICK_PRESS, STICK_RELEASE, Some(Action::Left), Some(Action::Right)),
                        // SDL's Y axis is positive downward.
                        Axis::LeftY => (&mut p.ly, STICK_PRESS, STICK_RELEASE, Some(Action::Up), Some(Action::Down)),
                        Axis::RightX => (&mut p.rx, STICK_PRESS, STICK_RELEASE, Some(Action::TurnLeft), Some(Action::TurnRight)),
                        Axis::RightY => (&mut p.ry, STICK_PRESS, STICK_RELEASE, Some(Action::ZoomIn), Some(Action::ZoomOut)),
                        Axis::TriggerLeft => (&mut p.lt, TRIGGER_PRESS, TRIGGER_RELEASE, None, Some(Action::PrevLetter)),
                        Axis::TriggerRight => (&mut p.rt, TRIGGER_PRESS, TRIGGER_RELEASE, None, Some(Action::NextLetter)),
                    };
                    for edge in latch.update(value, press, release) {
                        match edge {
                            Edge::PressNeg => {
                                if let Some(a) = neg {
                                    fire(a, id, now, &mut repeater)
                                }
                            }
                            Edge::PressPos => {
                                if let Some(a) = pos {
                                    fire(a, id, now, &mut repeater)
                                }
                            }
                            Edge::ReleaseNeg => neg.into_iter().for_each(|a| repeater.release(a)),
                            Edge::ReleasePos => pos.into_iter().for_each(|a| repeater.release(a)),
                        }
                    }
                }
                _ => {}
            }
        }

        let now = Instant::now();
        if nav_enabled {
            if let Some(a) = repeater.tick(now) {
                let _ = app.emit("nav", a.as_str());
            }
        }
        if running && pads.values_mut().any(|p| p.combo.poll(now)) {
            log::info!("exit combo held");
            on_exit_combo();
        }

        let interval = if running {
            Duration::from_millis(16)
        } else if nav_enabled {
            Duration::from_millis(8)
        } else {
            Duration::from_millis(150)
        };
        // Wake early for a pending repeat or combo deadline.
        let deadline = [repeater.next_due(), pads.values().filter_map(|p| p.combo.due_at()).min()]
            .into_iter()
            .flatten()
            .min()
            .map(|d| d.saturating_duration_since(now));
        std::thread::sleep(deadline.map_or(interval, |d| d.min(interval)).max(Duration::from_millis(1)));
    }
}

fn family(pad: &Gamepad) -> &'static str {
    use sdl3::gamepad::GamepadType as T;
    match pad.r#type() {
        T::PS3 | T::PS4 | T::PS5 => "playstation",
        T::Xbox360 | T::XboxOne => "xbox",
        T::NintendoSwitchPro | T::NintendoSwitchJoyconLeft | T::NintendoSwitchJoyconRight | T::NintendoSwitchJoyconPair => {
            "nintendo"
        }
        _ => "generic",
    }
}
