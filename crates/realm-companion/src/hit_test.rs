//! Native cursor polling still works while our transparent window ignores mouse events.
use benilla_app::companion_renderer::CompanionHitMask;
use bevy::prelude::*;
#[derive(Resource, Default)]
pub(crate) struct PointerGate {
    pub hit: bool,
    pub captured: bool,
    pub point: Option<Vec2>,
    was_down: bool,
    outside_press: bool,
}
impl PointerGate {
    fn update(&mut self, hit: bool, down: bool) -> bool {
        if down && !self.was_down {
            self.outside_press = !hit;
            if hit {
                self.captured = true;
            }
        }
        if !down {
            self.captured = false;
            self.outside_press = false;
        }
        self.hit = hit && !self.outside_press;
        self.was_down = down;
        self.hit || self.captured
    }
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<PointerGate>();
    #[cfg(not(target_os = "macos"))]
    app.insert_resource(PointerGate {
        hit: true,
        ..Default::default()
    });
    #[cfg(target_os = "macos")]
    app.add_systems(Update, poll.before(crate::window_controls));
}
#[cfg(target_os = "macos")]
fn poll(
    _main: bevy::ecs::system::NonSendMarker,
    ui: Res<crate::UiState>,
    mask: Res<CompanionHitMask>,
    time: Res<Time>,
    mut gate: ResMut<PointerGate>,
) {
    use objc2::{
        msg_send,
        runtime::{AnyClass, AnyObject},
    };
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect};
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(event) = AnyClass::get("NSEvent") else {
        return;
    };
    unsafe {
        let point: NSPoint = msg_send![event, mouseLocation];
        let buttons: u64 = msg_send![event, pressedMouseButtons];
        let down = buttons & 1 != 0;
        for window in NSApplication::sharedApplication(mtm).windows().iter() {
            if window.title().to_string() != "Realm Companion" {
                continue;
            }
            if ui.chooser {
                window.setIgnoresMouseEvents(false);
                gate.hit = true;
                gate.captured = false;
                gate.was_down = down;
                return;
            }
            let local: NSPoint = msg_send![&*window,convertPointFromScreen:point];
            let view: *mut AnyObject = msg_send![&*window, contentView];
            if view.is_null() {
                continue;
            }
            let p: NSPoint =
                msg_send![view,convertPoint:local fromView:std::ptr::null::<AnyObject>()];
            let bounds: NSRect = msg_send![view, bounds];
            let flipped: bool = msg_send![view, isFlipped];
            let y = if flipped {
                p.y - bounds.origin.y
            } else {
                bounds.size.height - (p.y - bounds.origin.y)
            };
            gate.point = Some(Vec2::new((p.x - bounds.origin.x) as f32, y as f32));
            let hit = time.elapsed_secs_f64() - mask.captured_at < 0.5
                && mask.contains(
                    Vec2::new((p.x - bounds.origin.x) as f32, y as f32),
                    Vec2::new(bounds.size.width as f32, bounds.size.height as f32),
                );
            window.setIgnoresMouseEvents(!gate.update(hit, down));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_body_press_retains_capture_outside_until_release() {
        let mut g = PointerGate::default();
        assert!(g.update(true, false));
        assert!(g.update(true, true));
        assert!(g.update(false, true));
        assert!(!g.update(false, false));
        assert!(!g.captured);
    }
    #[test]
    fn a_background_drag_is_never_stolen_on_crossing_the_character() {
        let mut g = PointerGate::default();
        assert!(!g.update(false, true));
        assert!(!g.update(true, true));
        assert!(g.update(true, false));
        assert!(g.update(true, true));
    }
}
