//! Main-loop wake checks without running the renderer or the ECS twenty times a second.
use bevy::prelude::*;
#[derive(Resource, Default)]
pub(crate) struct WakeReady(pub bool);
pub fn install(app: &mut App) {
    app.init_resource::<WakeReady>();
    #[cfg(target_os = "macos")]
    install_mac(app);
}
#[cfg(target_os = "macos")]
struct NativeWake(objc2::rc::Retained<objc2_foundation::NSTimer>);
#[cfg(target_os = "macos")]
impl Drop for NativeWake {
    fn drop(&mut self) {
        unsafe {
            self.0.invalidate();
        }
    }
}
#[cfg(target_os = "macos")]
fn install_mac(app: &mut App) {
    use objc2::{msg_send, runtime::AnyClass};
    use objc2_app_kit::NSApplication;
    use objc2_foundation::{MainThreadMarker, NSPoint, NSTimer};
    use std::ptr::NonNull;
    let Some(_mtm) = MainThreadMarker::new() else {
        return;
    };
    let Some(proxy) = app
        .world()
        .get_resource::<bevy::winit::EventLoopProxyWrapper>()
    else {
        return;
    };
    let proxy = (**proxy).clone();
    let latest = app.world().resource::<crate::Poller>().latest.clone();
    let was_near = std::cell::Cell::new(false);
    let block = block2::RcBlock::new(move |_: NonNull<NSTimer>| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let pending = latest.lock().map(|q| q.is_some()).unwrap_or(false);
        let mut near = false;
        if let Some(event) = AnyClass::get("NSEvent") {
            unsafe {
                let point: NSPoint = msg_send![event, mouseLocation];
                for window in NSApplication::sharedApplication(mtm).windows().iter() {
                    if window.title().to_string() != "Realm Companion" || !window.isVisible() {
                        continue;
                    }
                    let frame = window.frame();
                    if point.x >= frame.origin.x
                        && point.x < frame.origin.x + frame.size.width
                        && point.y >= frame.origin.y
                        && point.y < frame.origin.y + frame.size.height
                    {
                        near = true;
                        break;
                    }
                }
            }
        }
        let previous = was_near.replace(near);
        if should_wake(pending, near, previous) {
            let _ = proxy.send_event(bevy::winit::WinitUserEvent::WakeUp);
        }
    });
    let timer =
        unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(0.05, true, &block) };
    app.insert_non_send_resource(NativeWake(timer));
    app.world_mut().resource_mut::<WakeReady>().0 = true;
}

fn should_wake(pending: bool, near: bool, previous: bool) -> bool {
    pending || near != previous
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_wake_edge_prevents_hover_event_flood() {
        let mut previous = false;
        let events = [false, true, true, true, false, false]
            .into_iter()
            .map(|near| {
                let wake = should_wake(false, near, previous);
                previous = near;
                wake
            })
            .filter(|v| *v)
            .count();
        assert_eq!(events, 2);
        assert!(should_wake(true, false, false));
    }
}
