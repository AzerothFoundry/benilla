use benilla_app::companion_renderer::{CompanionFrame, CompanionVisualState};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
#[derive(Resource, Default)]
pub(crate) struct Reveal {
    prepared_frames: u8,
    pub shown: bool,
}
impl Reveal {
    pub fn reset(&mut self) {
        self.prepared_frames = 0;
        self.shown = false;
    }
}
pub fn local_hour() -> u32 {
    #[cfg(unix)]
    {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as libc::time_t;
        let mut value = std::mem::MaybeUninit::<libc::tm>::uninit();
        // SAFETY: both pointers refer to live values, and localtime_r initializes tm on success.
        unsafe {
            if !libc::localtime_r(&seconds, value.as_mut_ptr()).is_null() {
                return value.assume_init().tm_hour as u32;
            }
        }
    }
    12
}
pub fn preview(cache: Option<crate::settings::AppearanceCache>) -> serde_json::Value {
    let mut frame=cache.map(|c|c.offline_frame(local_hour())).unwrap_or_else(||serde_json::json!({"connected":false,"appearance":null,"animation":{"sleeping":crate::settings::offline_sleeping(local_hour())}}));
    frame["_local_preview"] = true.into();
    frame
}
pub fn reveal_avatar(
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    meshes: Query<&RenderLayers, With<Mesh3d>>,
    frame: Res<CompanionFrame>,
    ui: Res<crate::UiState>,
    visual: Res<CompanionVisualState>,
    mut reveal: ResMut<Reveal>,
) {
    if !reveal.shown {
        if visual.ready
            && visual.appearance == frame.0["appearance"]
            && frame.0["appearance"].is_object()
            && meshes
                .iter()
                .any(|l| l.intersects(&RenderLayers::layer(31)))
        {
            reveal.prepared_frames = reveal.prepared_frames.saturating_add(1);
        } else {
            reveal.prepared_frames = 0;
        }
        if reveal.prepared_frames >= 3 {
            reveal.shown = true;
            info!("companion startup: model prepared; revealing transparent window");
        }
    }
    let visible=reveal.shown&&!ui.chooser;
    if window.visible!=visible{window.visible=visible;}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_window_stays_hidden_until_requested_avatar_is_ready_and_selector_hides_it() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<crate::UiState>()
            .init_resource::<Reveal>();
        let expected = serde_json::json!({"race":8,"sex":1});
        app.insert_resource(CompanionFrame(serde_json::json!({"appearance":expected})));
        app.insert_resource(CompanionVisualState {
            ready: true,
            appearance: serde_json::json!({"race":1,"sex":0}),
        });
        let window = app
            .world_mut()
            .spawn((
                Window {
                    visible: false,
                    ..default()
                },
                PrimaryWindow,
            ))
            .id();
        app.world_mut()
            .spawn((Mesh3d(Handle::default()), RenderLayers::layer(31)));
        app.add_systems(Update, reveal_avatar);
        for _ in 0..5 {
            app.update();
        }
        assert!(
            !app.world().get::<Window>(window).unwrap().visible,
            "old avatar cannot make a new role visible"
        );
        app.world_mut()
            .resource_mut::<CompanionVisualState>()
            .appearance = expected;
        app.update();
        app.update();
        assert!(!app.world().get::<Window>(window).unwrap().visible);
        app.update();
        assert!(app.world().get::<Window>(window).unwrap().visible);
        app.world_mut().resource_mut::<crate::UiState>().chooser = true;
        app.update();
        assert!(!app.world().get::<Window>(window).unwrap().visible);
    }
}
