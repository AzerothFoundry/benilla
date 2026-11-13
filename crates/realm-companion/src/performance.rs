//! Opt-in, low-volume measurement; no names, URLs or full snapshots in output.
use benilla_app::companion_renderer::CompanionRenderCounters;
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
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
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .set_max_delta(std::time::Duration::from_millis(500));
    if std::env::var("REALM_COMPANION_REFRESH").as_deref() == Ok("legacy") {
        return;
    }
    app.init_resource::<benilla_app::companion_renderer::CompanionDerivedActivity>()
        .init_resource::<benilla_app::companion_renderer::CompanionMaskRate>();
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
    wake: Res<crate::native_wake::WakeReady>,
    derived: Res<benilla_app::companion_renderer::CompanionDerivedActivity>,
    time: Res<Time>,
    mut activity: Local<Activity>,
) {
    let a = &frame.0["animation"];
    let interactive = ui.open || pointer.near || pointer.captured || gesture.dancing;
    let raw_active = derived.0
        || (frame.0["connected"] == true
            && (a["dancing"] == true
                || a["stunned"] == true
                || gesture.dancing
                || a["moving"] == true
                || a["attacking"] == true
                || a["cast_remaining_ms"].as_u64().unwrap_or(0) > 0
                || a["emote_state_id"].as_u64().unwrap_or(0) > 0
                || a["auto_repeat_spell"].as_u64().unwrap_or(0) > 0
                || a["looting"] == true
                || matches!(
                    a["locomotion"].as_str(),
                    Some("fall" | "swim" | "swim_idle" | "swim_backward" | "fly")
                )));
    let now = time.elapsed_secs_f64();
    let seq = frame.0["visual_sequence"].as_u64().unwrap_or(0);
    if activity.sequence != seq {
        activity.sequence = seq;
        activity.until = now + 3.;
    }
    let posture = (
        frame.0["connected"] == true,
        a["stand_state"].as_u64().unwrap_or(0),
        a["dead"] == true,
        frame.0["appearance"]["mount_display_id"]
            .as_u64()
            .unwrap_or(0),
        frame.0["appearance"]["display_id"].as_u64().unwrap_or(0),
        a["sleeping"] == true
            || a["stand_state"] == 3
            || (frame.0["connected"] != true && frame.0["_local_preview"] != true),
    );
    if activity.posture != Some(posture) {
        activity.posture = Some(posture);
        activity.until = activity.until.max(now + 2.);
    }
    if activity.was_active && !raw_active {
        activity.until = activity.until.max(now + 0.6);
    }
    activity.was_active = raw_active;
    let active = raw_active || now < activity.until;
    let sleep = frame.0["connected"] != true || a["sleeping"] == true || a["stand_state"] == 3;
    let fps = if ui.settings.power_saving_idle && wake.0 && !interactive && !active {
        4
    } else {
        refresh_rate(interactive, active, sleep)
    };
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
    let hz = if ui.settings.power_saving_idle
        && !pointer.near
        && !pointer.captured
        && !active
        && !ui.open
    {
        0.
    } else if interactive {
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
pub fn install_scene_executor(app: &mut App) {
    if std::env::var("REALM_COMPANION_PHYSICS").as_deref() != Ok("on") {
        use avian3d::prelude::PhysicsSystems;
        app.configure_sets(
            FixedPostUpdate,
            (
                PhysicsSystems::First,
                PhysicsSystems::Prepare,
                PhysicsSystems::StepSimulation,
                PhysicsSystems::Writeback,
                PhysicsSystems::Last,
            )
                .run_if(|| false),
        );
    }

    app.add_systems(Last, clear_hidden_ui_redraw);
    crate::native_wake::install(app);
    if std::env::var("REALM_COMPANION_EXECUTOR").as_deref() == Ok("parallel") {
        return;
    }
    if std::env::var("REALM_COMPANION_RENDER_EXECUTOR").as_deref() != Ok("parallel") {
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render.edit_schedule(bevy::render::Render, |schedule| {
                schedule.set_executor_kind(bevy::ecs::schedule::ExecutorKind::SingleThreaded);
            });
        }
    }
    for label in [PreUpdate.intern(), Update.intern(), PostUpdate.intern()] {
        app.edit_schedule(label, |schedule| {
            schedule.set_executor_kind(bevy::ecs::schedule::ExecutorKind::SingleThreaded);
        });
    }
}

#[cfg(test)]
mod scene_tests {
    use super::*;
    #[test]
    fn core_time_and_message_lifecycle_are_retained() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        install_scene_executor(&mut app);
        let order = app.world().resource::<bevy::app::MainScheduleOrder>();
        assert!(order.labels.contains(&bevy::app::RunFixedMainLoop.intern()));
        assert!(!app
            .world()
            .resource::<bevy::app::FixedMainScheduleOrder>()
            .labels
            .is_empty());
        assert!(order.labels.contains(&Update.intern()));
        assert!(order.labels.contains(&PostUpdate.intern()));
    }
}

// egui can retain a repaint request from a now-hidden settings context. The timer
// still advances animation; native input/user events still wake the Winit loop.
fn clear_hidden_ui_redraw(
    ui: Res<crate::UiState>,
    mut redraw: ResMut<Messages<bevy::window::RequestRedraw>>,
) {
    if !ui.open {
        redraw.clear();
    }
}
#[cfg(test)]
mod redraw_tests {
    use super::*;
    #[test]
    fn visible_settings_keep_redraw_but_hidden_settings_use_timer() {
        let mut app = App::new();
        app.init_resource::<crate::UiState>()
            .add_message::<bevy::window::RequestRedraw>()
            .add_systems(Last, clear_hidden_ui_redraw);
        app.world_mut().resource_mut::<crate::UiState>().open = true;
        app.world_mut()
            .resource_mut::<Messages<bevy::window::RequestRedraw>>()
            .write(bevy::window::RequestRedraw);
        app.update();
        assert!(!app
            .world()
            .resource::<Messages<bevy::window::RequestRedraw>>()
            .is_empty());
        app.world_mut().resource_mut::<crate::UiState>().open = false;
        app.update();
        assert!(app
            .world()
            .resource::<Messages<bevy::window::RequestRedraw>>()
            .is_empty());
    }
}

#[derive(Default)]
struct Activity {
    sequence: u64,
    until: f64,
    was_active: bool,
    posture: Option<(bool, u64, bool, u64, u64, bool)>,
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[derive(Message)]
    struct Pulse;
    #[test]
    fn fixed_lifecycle_still_recycles_messages_with_desktop_executor() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<crate::UiState>()
            .add_message::<bevy::window::RequestRedraw>()
            .add_message::<Pulse>();
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(50),
        ));
        install_scene_executor(&mut app);
        app.world_mut()
            .resource_mut::<Messages<Pulse>>()
            .write(Pulse);
        for _ in 0..6 {
            app.update();
        }
        assert!(app.world().resource::<Messages<Pulse>>().is_empty());
    }
}

#[cfg(test)]
mod adaptive_policy_tests {
    use super::*;
    #[test]
    fn quiet_power_mode_is_low_but_activity_and_settling_remain_fast() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<crate::UiState>()
            .init_resource::<crate::Interaction>()
            .init_resource::<crate::hit_test::PointerGate>()
            .init_resource::<benilla_app::companion_renderer::CompanionMaskRate>()
            .init_resource::<benilla_app::companion_renderer::CompanionDerivedActivity>();
        app.insert_resource(crate::CompanionFrame(
            serde_json::json!({"connected":true,"animation":{"stand_state":0}}),
        ))
        .insert_resource(crate::native_wake::WakeReady(true))
        .insert_resource(bevy::winit::WinitSettings::game())
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            std::time::Duration::from_millis(250),
        ))
        .add_systems(Update, refresh);
        for _ in 0..14 {
            app.update();
        }
        let mode = |app: &App| {
            app.world()
                .resource::<bevy::winit::WinitSettings>()
                .focused_mode
        };
        assert_eq!(
            mode(&app),
            bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_secs_f64(0.25))
        );
        app.world_mut().resource_mut::<crate::CompanionFrame>().0["animation"]["moving"] =
            true.into();
        app.update();
        assert_eq!(
            mode(&app),
            bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_secs_f64(
                1. / 30.
            ))
        );
        app.world_mut().resource_mut::<crate::CompanionFrame>().0["animation"]["moving"] =
            false.into();
        app.update();
        assert_eq!(
            mode(&app),
            bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_secs_f64(
                1. / 30.
            ))
        );
        for _ in 0..8 {
            app.update();
        }
        app.world_mut()
            .resource_mut::<crate::UiState>()
            .settings
            .power_saving_idle = false;
        app.update();
        assert_eq!(
            mode(&app),
            bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_secs_f64(
                1. / 20.
            ))
        );
        app.world_mut().resource_mut::<crate::CompanionFrame>().0["connected"] = false.into();
        app.world_mut().resource_mut::<crate::CompanionFrame>().0["animation"]["moving"] =
            true.into();
        for _ in 0..16 {
            app.update();
        }
        assert_eq!(
            mode(&app),
            bevy::winit::UpdateMode::reactive_low_power(std::time::Duration::from_secs_f64(
                1. / 10.
            ))
        );
    }
}
