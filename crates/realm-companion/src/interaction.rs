//! Local gestures never issue game commands or alter the authoritative character state.
use bevy::prelude::*;
use serde_json::Value;

const DRAG_DISTANCE: f32 = 6.0;
const DOUBLE_CLICK_MS: u64 = 500;
#[derive(Default, Resource)]
pub(crate) struct Interaction {
    pub dancing: bool,
    pub request: u64,
    pressed: Option<(Vec2, bool)>,
    last_click: Option<(u64, Vec2)>,
}
impl Interaction {
    pub fn press(&mut self, point: Vec2) {
        self.pressed = Some((point, false));
    }
    /// Start one native window drag only after actual pointer movement.
    pub fn move_pointer(&mut self, point: Vec2) -> bool {
        let Some((start, dragging)) = self.pressed.as_mut() else {
            return false;
        };
        if !*dragging && start.distance(point) >= DRAG_DISTANCE {
            *dragging = true;
            self.last_click = None;
            return true;
        }
        false
    }
    pub fn release(&mut self, now_ms: u64, point: Option<Vec2>, allowed: bool) {
        let Some((start, dragging)) = self.pressed.take() else {
            return;
        };
        let Some(point) = point else { return };
        if dragging || start.distance(point) >= DRAG_DISTANCE {
            return;
        }
        if self.dancing {
            self.cancel();
            return;
        }
        if !allowed {
            self.last_click = None;
            return;
        }
        if self.last_click.is_some_and(|(time, p)| {
            now_ms.saturating_sub(time) <= DOUBLE_CLICK_MS && p.distance(point) < DRAG_DISTANCE
        }) {
            self.dancing = true;
            self.request = (1u64 << 63) | now_ms;
            self.last_click = None;
        } else {
            self.last_click = Some((now_ms, point));
        }
    }
    pub fn cancel(&mut self) {
        self.dancing = false;
        self.last_click = None;
    }
    pub fn server_update(&mut self, before: &Value, after: &Value) {
        // SSE has already filtered unchanged visual state; polling must do that locally.
        if after["_transport"] == "sse" || visual_key(before) != visual_key(after) {
            self.cancel();
        }
    }
}
pub(crate) fn dance_allowed(frame: &Value) -> bool {
    frame["connected"] == true
        && frame["animation"]["dead"] != true
        && frame["animation"]["sleeping"] != true
        && frame["animation"]["stand_state"].as_u64() != Some(3)
}
fn visual_key(frame: &Value) -> Value {
    let mut animation = frame["animation"].clone();
    if let Some(a) = animation.as_object_mut() {
        a.remove("dancing");
        let casting = a
            .remove("cast_remaining_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            > 0;
        a.insert("casting".into(), casting.into());
    }
    serde_json::json!({"connected":frame["connected"],"character":frame["character"],"appearance":frame["appearance"],"animation":animation})
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn click(i: &mut Interaction, t: u64, allowed: bool) {
        i.press(Vec2::ZERO);
        i.release(t, Some(Vec2::ZERO), allowed);
    }
    #[test]
    fn double_click_starts_single_click_stops_and_drag_does_not_count() {
        let mut i = Interaction::default();
        click(&mut i, 100, true);
        assert!(!i.dancing);
        click(&mut i, 300, true);
        assert!(i.dancing);
        click(&mut i, 500, true);
        assert!(!i.dancing);
        i.press(Vec2::ZERO);
        assert!(i.move_pointer(Vec2::new(10., 0.)));
        assert!(!i.move_pointer(Vec2::new(20., 0.)));
        i.release(600, Some(Vec2::new(20., 0.)), true);
        click(&mut i, 700, true);
        assert!(!i.dancing);
        click(&mut i, 800, true);
        assert!(i.dancing);
    }
    #[test]
    fn each_local_gesture_has_new_request_and_server_state_cancels_it() {
        let mut i = Interaction::default();
        click(&mut i, 100, true);
        click(&mut i, 200, true);
        let first = i.request;
        assert!(first > (1u64 << 63));
        i.server_update(
            &json!({"animation":{"moving":false}}),
            &json!({"animation":{"moving":true}}),
        );
        assert!(!i.dancing);
        click(&mut i, 600, true);
        click(&mut i, 700, true);
        assert!(i.dancing);
        assert_ne!(first, i.request);
    }
    #[test]
    fn sleeping_offline_and_dead_cannot_dance() {
        for f in [
            json!({"connected":false}),
            json!({"connected":true,"animation":{"stand_state":3}}),
            json!({"connected":true,"animation":{"dead":true}}),
        ] {
            let mut i = Interaction::default();
            click(&mut i, 100, dance_allowed(&f));
            click(&mut i, 200, dance_allowed(&f));
            assert!(!i.dancing);
        }
    }
    #[test]
    fn relevant_state_stops_dance_but_identical_poll_and_clock_changes_do_not() {
        let base = json!({"connected":true,"animation":{"moving":false,"cast_remaining_ms":0}});
        let mut i = Interaction::default();
        click(&mut i, 100, true);
        click(&mut i, 200, true);
        let mut rendered = base.clone();
        rendered["animation"]["dancing"] = true.into();
        let mut polled = base.clone();
        polled["now_ms"] = 500.into();
        i.server_update(&rendered, &polled);
        assert!(i.dancing);
        polled["animation"]["moving"] = true.into();
        i.server_update(&rendered, &polled);
        assert!(!i.dancing);
        click(&mut i, 600, true);
        click(&mut i, 700, true);
        polled["_transport"] = "sse".into();
        i.server_update(&polled, &polled);
        assert!(!i.dancing);
    }
}
