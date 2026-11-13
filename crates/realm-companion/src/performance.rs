//! Opt-in, low-volume measurement; no names, URLs or full snapshots in output.
use benilla_app::companion_renderer::CompanionRenderCounters;
use bevy::prelude::*;
use bevy::ecs::schedule::ScheduleLabel;
use std::time::Instant;
#[derive(Resource)]
struct Meter {
    start: Instant,
    frames: u64,
    captures: u64,
    readbacks: u64,
}
pub fn install(app: &mut App) {
    if std::env::var_os("REALM_COMPANION_PERF").is_some() {
        app.init_resource::<CompanionRenderCounters>();
        app.world_mut()
            .resource_mut::<CompanionRenderCounters>()
            .enabled = true;
        app.insert_resource(Meter {
            start: Instant::now(),
            frames: 0,
            captures: 0,
            readbacks: 0,
        })
        .add_systems(Update, measure);
    }
}
fn measure(mut meter: ResMut<Meter>, counts: Res<CompanionRenderCounters>) {
    meter.frames += 1;
    let secs = meter.start.elapsed().as_secs_f64();
    if secs >= 15. {
        info!("companion performance: seconds={secs:.2} update_fps={:.2} mask_fps={:.2} readback_fps={:.2}",meter.frames as f64/secs,(counts.mask_captures-meter.captures) as f64/secs,(counts.mask_readbacks-meter.readbacks) as f64/secs);
        meter.frames = 0;
        meter.captures = counts.mask_captures;
        meter.readbacks = counts.mask_readbacks;
        meter.start = Instant::now();
    }
}

pub fn install_refresh(app: &mut App) {
    if std::env::var("REALM_COMPANION_REFRESH").as_deref() == Ok("legacy") {
        return;
    }
    app.init_resource::<benilla_app::companion_renderer::CompanionMaskRate>();
    let mode =
        bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_secs_f64(1. / 20.));
    app.insert_resource(bevy::winit::WinitSettings {
        focused_mode: mode,
        unfocused_mode: mode,
    })
    .add_systems(Update, refresh.after(crate::apply_local_actions));
}
fn refresh(
    ui: Res<crate::UiState>,
    frame: Res<crate::CompanionFrame>,
    gesture: Res<crate::Interaction>,
    pointer: Res<crate::hit_test::PointerGate>,
    mut settings: ResMut<bevy::winit::WinitSettings>,
    mut mask: ResMut<benilla_app::companion_renderer::CompanionMaskRate>,
    mut previous: Local<u32>,
) {
    let a = &frame.0["animation"];
    let interactive = ui.open || pointer.captured || gesture.dancing;
    let active = a["moving"] == true
        || a["attacking"] == true
        || a["cast_remaining_ms"].as_u64().unwrap_or(0) > 0;
    let sleep = frame.0["connected"] != true || a["sleeping"] == true || a["stand_state"] == 3;
    let fps = refresh_rate(interactive, active, sleep);
    if *previous != fps {
        if std::env::var_os("REALM_COMPANION_PERF").is_some() {
            info!("companion refresh target: fps={fps} interactive={interactive} moving_or_casting={active} sleeping_or_offline={sleep}");
        }
        *previous = fps;
    }
    let mode = bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_secs_f64(
        1. / fps as f64,
    ));
    if settings.focused_mode != mode || settings.unfocused_mode != mode {
        settings.focused_mode = mode;
        settings.unfocused_mode = mode;
    }
    let hz = if interactive {
        30.
    } else if active {
        15.
    } else if pointer.hit {
        10.
    } else {
        5.
    };
    if mask.hz != hz {
        mask.hz = hz;
    }
}
fn refresh_rate(interactive: bool, active: bool, sleep: bool) -> u32 {
    if interactive || active {
        30
    } else if sleep {
        10
    } else {
        20
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_and_motion_override_sleep_and_idle() {
        assert_eq!(refresh_rate(false, false, false), 20);
        assert_eq!(refresh_rate(false, true, false), 30);
        assert_eq!(refresh_rate(true, false, true), 30);
        assert_eq!(refresh_rate(false, false, true), 10);
    }
}

/// A one-character scene has many tiny systems; avoid waking workers for each batch.
/// Asset IO and renderer task pools remain independent; game clients are unaffected.
pub fn install_scene_executor(app:&mut App){
    if std::env::var("REALM_COMPANION_EXECUTOR").as_deref()==Ok("parallel"){return;}
    for label in [PreUpdate.intern(),Update.intern(),PostUpdate.intern()]{
        app.edit_schedule(label,|schedule|{schedule.set_executor_kind(bevy::ecs::schedule::ExecutorKind::SingleThreaded);});
    }
}
