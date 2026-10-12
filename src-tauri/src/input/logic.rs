//! Device-independent input logic: hold-to-repeat, stick/trigger thresholds,
//! duplicate-pad suppression and the hold-to-exit combo. Pure and unit-tested;
//! `gamepad.rs` feeds it SDL events and the clock.

use std::collections::HashSet;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// UI actions, matching the frontend's `Action` type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    Up,
    Down,
    Left,
    Right,
    Select,
    Back,
    /// West face button (Square / X): search, space on the on-screen keyboard.
    Secondary,
    PrevPlatform,
    NextPlatform,
    PrevLetter,
    NextLetter,
    Favorite,
    Menu,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Up => "up",
            Action::Down => "down",
            Action::Left => "left",
            Action::Right => "right",
            Action::Select => "select",
            Action::Back => "back",
            Action::Secondary => "secondary",
            Action::PrevPlatform => "prevPlatform",
            Action::NextPlatform => "nextPlatform",
            Action::PrevLetter => "prevLetter",
            Action::NextLetter => "nextLetter",
            Action::Favorite => "favorite",
            Action::Menu => "menu",
        }
    }

    /// Actions that auto-repeat while held.
    pub fn repeats(self) -> bool {
        matches!(
            self,
            Action::Up | Action::Down | Action::Left | Action::Right | Action::PrevLetter | Action::NextLetter
        )
    }
}

pub const REPEAT_DELAY: Duration = Duration::from_millis(380);
pub const REPEAT_SLOW: Duration = Duration::from_millis(110);
pub const REPEAT_FAST: Duration = Duration::from_millis(55);
/// Holding longer than this switches to the fast repeat rate.
pub const ACCELERATE_AFTER: Duration = Duration::from_millis(1200);

/// Hold-to-repeat for one action at a time (the most recently pressed wins,
/// like a keyboard).
#[derive(Debug, Default)]
pub struct Repeater {
    held: Option<(Action, Instant)>,
    next_at: Option<Instant>,
}

impl Repeater {
    pub fn press(&mut self, a: Action, now: Instant) {
        if a.repeats() {
            self.held = Some((a, now));
            self.next_at = Some(now + REPEAT_DELAY);
        }
    }

    pub fn release(&mut self, a: Action) {
        if self.held.is_some_and(|(h, _)| h == a) {
            self.held = None;
            self.next_at = None;
        }
    }

    pub fn release_all(&mut self) {
        self.held = None;
        self.next_at = None;
    }

    /// Returns the action to fire if a repeat is due, scheduling the next one.
    pub fn tick(&mut self, now: Instant) -> Option<Action> {
        let (a, since) = self.held?;
        let due = self.next_at?;
        if now < due {
            return None;
        }
        let step = if now.duration_since(since) > ACCELERATE_AFTER { REPEAT_FAST } else { REPEAT_SLOW };
        // Keep a steady cadence, but after a stall restart from now instead of
        // firing a backlog of repeats.
        let next = due + step;
        self.next_at = Some(if next <= now { now + step } else { next });
        Some(a)
    }

    /// When the next repeat is due, for sizing the event-wait timeout.
    pub fn next_due(&self) -> Option<Instant> {
        self.next_at
    }
}

/// Turns an analog axis into a digital press with hysteresis, so a stick
/// resting near the threshold doesn't chatter.
#[derive(Debug, Default, Clone, Copy)]
pub struct AxisLatch {
    pub negative: bool,
    pub positive: bool,
}

pub const STICK_PRESS: i16 = 16_000;
pub const STICK_RELEASE: i16 = 10_000;
pub const TRIGGER_PRESS: i16 = 20_000;
pub const TRIGGER_RELEASE: i16 = 12_000;

#[derive(Debug, PartialEq, Eq)]
pub enum Edge {
    PressNeg,
    ReleaseNeg,
    PressPos,
    ReleasePos,
}

impl AxisLatch {
    pub fn update(&mut self, value: i16, press: i16, release: i16) -> Vec<Edge> {
        let mut out = Vec::new();
        if self.positive && value < release {
            self.positive = false;
            out.push(Edge::ReleasePos);
        }
        if self.negative && value > -release {
            self.negative = false;
            out.push(Edge::ReleaseNeg);
        }
        if !self.positive && value >= press {
            self.positive = true;
            out.push(Edge::PressPos);
        }
        if !self.negative && value <= -press {
            self.negative = true;
            out.push(Edge::PressNeg);
        }
        out
    }
}

/// Drops an action if another pad sent the same action moments ago. Steam
/// Input / DS4Windows expose a virtual Xbox pad alongside the real PlayStation
/// pad, so one physical press can arrive twice.
#[derive(Debug, Default)]
pub struct Dedupe<D> {
    last: Option<(Action, D, Instant)>,
}

pub const DEDUPE_WINDOW: Duration = Duration::from_millis(45);

impl<D: PartialEq + Copy> Dedupe<D> {
    pub fn accept(&mut self, a: Action, device: D, now: Instant) -> bool {
        if let Some((la, ld, at)) = self.last {
            if la == a && ld != device && now.duration_since(at) < DEDUPE_WINDOW {
                return false;
            }
        }
        self.last = Some((a, device, now));
        true
    }
}

/// Fires once when every button of the combo has been held together on one
/// pad for `hold`. Re-arms only after the combo is released.
#[derive(Debug)]
pub struct Combo<B> {
    pub buttons: Vec<B>,
    pub hold: Duration,
    since: Option<Instant>,
    fired: bool,
}

impl<B: Eq + Hash + Copy> Combo<B> {
    pub fn new(buttons: Vec<B>, hold: Duration) -> Self {
        Self { buttons, hold, since: None, fired: false }
    }

    /// Call whenever this pad's held set changes.
    pub fn update(&mut self, held: &HashSet<B>, now: Instant) {
        let all = !self.buttons.is_empty() && self.buttons.iter().all(|b| held.contains(b));
        if all {
            self.since.get_or_insert(now);
        } else {
            self.since = None;
            self.fired = false;
        }
    }

    /// True exactly once per hold, when the hold time has elapsed.
    pub fn poll(&mut self, now: Instant) -> bool {
        match self.since {
            Some(s) if !self.fired && now.duration_since(s) >= self.hold => {
                self.fired = true;
                true
            }
            _ => false,
        }
    }

    pub fn due_at(&self) -> Option<Instant> {
        self.since.filter(|_| !self.fired).map(|s| s + self.hold)
    }

    pub fn reset(&mut self) {
        self.since = None;
        self.fired = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn repeater_delay_then_slow_then_fast() {
        let t0 = Instant::now();
        let mut r = Repeater::default();
        r.press(Action::Down, t0);
        assert_eq!(r.tick(t0 + ms(100)), None, "still in initial delay");
        assert_eq!(r.tick(t0 + REPEAT_DELAY), Some(Action::Down));
        assert_eq!(r.tick(t0 + REPEAT_DELAY + ms(50)), None);
        assert_eq!(r.tick(t0 + REPEAT_DELAY + REPEAT_SLOW), Some(Action::Down));
        // Count repeats over 1 s once accelerated.
        let start = t0 + ACCELERATE_AFTER + ms(10);
        let mut n = 0;
        let mut t = start;
        while t < start + ms(1000) {
            if r.tick(t).is_some() {
                n += 1;
            }
            t += ms(5);
        }
        assert!((16..=20).contains(&n), "fast rate ~18/s, got {n}");
        r.release(Action::Down);
        assert_eq!(r.tick(t + ms(500)), None);
    }

    #[test]
    fn repeater_ignores_non_repeating_and_stale_release() {
        let t0 = Instant::now();
        let mut r = Repeater::default();
        r.press(Action::Select, t0);
        assert_eq!(r.tick(t0 + ms(1000)), None);
        r.press(Action::Left, t0);
        r.press(Action::Up, t0 + ms(10)); // newest wins
        r.release(Action::Left); // releasing the old one doesn't stop Up
        assert_eq!(r.tick(t0 + ms(10) + REPEAT_DELAY), Some(Action::Up));
    }

    #[test]
    fn repeater_does_not_burst_after_a_stall() {
        let t0 = Instant::now();
        let mut r = Repeater::default();
        r.press(Action::Right, t0);
        // Thread stalled for 2 s: one repeat, not a backlog of 18.
        assert_eq!(r.tick(t0 + ms(2000)), Some(Action::Right));
        assert_eq!(r.tick(t0 + ms(2001)), None);
    }

    #[test]
    fn axis_latch_hysteresis() {
        let mut l = AxisLatch::default();
        assert!(l.update(12_000, STICK_PRESS, STICK_RELEASE).is_empty());
        assert_eq!(l.update(17_000, STICK_PRESS, STICK_RELEASE), [Edge::PressPos]);
        assert!(l.update(12_000, STICK_PRESS, STICK_RELEASE).is_empty(), "between thresholds: held");
        assert_eq!(l.update(9_000, STICK_PRESS, STICK_RELEASE), [Edge::ReleasePos]);
        // Flick straight across.
        l.update(20_000, STICK_PRESS, STICK_RELEASE);
        assert_eq!(l.update(-20_000, STICK_PRESS, STICK_RELEASE), [Edge::ReleasePos, Edge::PressNeg]);
    }

    #[test]
    fn dedupe_drops_same_action_from_twin_pad_only() {
        let t0 = Instant::now();
        let mut d = Dedupe::default();
        assert!(d.accept(Action::Select, 1u32, t0));
        assert!(!d.accept(Action::Select, 2u32, t0 + ms(10)), "virtual twin");
        assert!(d.accept(Action::Select, 1u32, t0 + ms(20)), "same pad pressing again");
        assert!(d.accept(Action::Back, 2u32, t0 + ms(25)), "different action");
        assert!(d.accept(Action::Back, 1u32, t0 + ms(200)), "outside the window");
    }

    #[test]
    fn combo_fires_once_after_hold_and_rearms_on_release() {
        let t0 = Instant::now();
        let mut c = Combo::new(vec!['s', 't'], ms(1000));
        let mut held = HashSet::from(['s']);
        c.update(&held, t0);
        assert!(!c.poll(t0 + ms(2000)), "only one button");
        held.insert('t');
        c.update(&held, t0 + ms(100));
        assert_eq!(c.due_at(), Some(t0 + ms(1100)));
        assert!(!c.poll(t0 + ms(1000)));
        assert!(c.poll(t0 + ms(1100)));
        assert!(!c.poll(t0 + ms(3000)), "fires once per hold");
        held.remove(&'t');
        c.update(&held, t0 + ms(3100));
        held.insert('t');
        c.update(&held, t0 + ms(3200));
        assert!(c.poll(t0 + ms(4200)), "re-armed");
    }

    #[test]
    fn combo_release_before_hold_cancels() {
        let t0 = Instant::now();
        let mut c = Combo::new(vec![1, 2], ms(1000));
        c.update(&HashSet::from([1, 2]), t0);
        c.update(&HashSet::from([1]), t0 + ms(900));
        assert!(!c.poll(t0 + ms(1500)));
        assert!(Combo::<u8>::new(vec![], ms(1)).due_at().is_none());
    }
}
