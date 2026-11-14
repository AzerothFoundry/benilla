//! Empty stock racial selection stages, using the client's glue renderer; no character/login.
use super::*;
#[derive(Resource, Default)]
pub struct CompanionSelectorBackground {
    pub visible: bool,
    pub race: Option<u8>,
    pub image: Option<Handle<Image>>,
    pub ready: bool,
}
#[derive(Component)]
pub struct CompanionSelectorWindow;
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct CompanionAvatarPass;
#[derive(Resource, Default)]
struct StageInputs {
    preview: GluePreview,
    bake: GluePreviewBake,
}
pub fn scene_token(race: u8) -> Option<&'static str> {
    Some(match race {
        1 => "Human",
        2 | 8 => "Orc",
        3 | 7 => "Dwarf",
        4 => "NightElf",
        5 => "Scourge",
        6 => "Tauren",
        _ => return None,
    })
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<CompanionSelectorBackground>()
        .init_resource::<StageInputs>()
        .init_resource::<PortraitImages>()
        .init_resource::<Booths>()
        .init_resource::<BoothLight>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                enter_stage,
                glue_booth::sync_glue_scene,
                glue_booth::sync_glue_ffx,
                glue_booth::pillarbox_glue_scene,
                glue_booth::pace_glue_clock,
                leave_stage,
            )
                .chain()
                .after(CompanionAvatarPass),
        );
}
fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut portraits: ResMut<PortraitImages>,
    mut booths: ResMut<Booths>,
    mut state: ResMut<CompanionSelectorBackground>,
    mut light: ResMut<BoothLight>,
    device: Res<bevy::render::renderer::RenderDevice>,
    queue: Res<bevy::render::renderer::RenderQueue>,
) {
    let blob = super::light::studio_light();
    let buffer = blob.create(&device, "companion_selector_light");
    blob.write(&queue, &buffer);
    light.studio.buffer = Some(buffer);
    glue_booth::spawn_glue_booth(&mut commands, &mut images, &mut portraits, &mut booths);
    state.image = booths.0.get(GLUE_SLOT).map(|b| b.target.clone());
}
fn enter_stage(
    state: Res<CompanionSelectorBackground>,
    mut saved: ResMut<StageInputs>,
    mut preview: ResMut<GluePreview>,
    mut bake: ResMut<GluePreviewBake>,
) {
    let token = state
        .visible
        .then(|| state.race.and_then(scene_token).unwrap_or("MainMenu"));
    let path = token.map(|t| format!("Interface\\Glues\\Models\\UI_{t}\\UI_{t}.mdx"));
    saved.preview.scene = path.as_deref().map(GlueSceneFile::new);
    saved.preview.camera = path.map(|file| benilla_ui::widget::InstalledCamera { file, index: 0 });
    saved.preview.look = None;
    saved.preview.fog = None;
    saved.preview.hidden = token.is_none();
    // Separate input banks keep the pet's appearance bake untouched by this empty stage.
    std::mem::swap(&mut *preview, &mut saved.preview);
    std::mem::swap(&mut *bake, &mut saved.bake);
}
fn leave_stage(
    mut saved: ResMut<StageInputs>,
    mut preview: ResMut<GluePreview>,
    mut bake: ResMut<GluePreviewBake>,
    mut state: ResMut<CompanionSelectorBackground>,
    assets: Res<AssetServer>,
    models: Res<Assets<benilla_assets::M2Model>>,
    mut cameras: Query<(&BoothCam, &mut Camera)>,
) {
    state.ready = preview
        .scene
        .as_ref()
        .and_then(|file| assets.get_handle::<benilla_assets::M2Model>(file.url().to_owned()))
        .is_some_and(|h| models.contains(&h));
    for (slot, mut camera) in &mut cameras {
        if slot.0 == GLUE_SLOT {
            camera.is_active = state.visible;
        }
    }
    std::mem::swap(&mut *preview, &mut saved.preview);
    std::mem::swap(&mut *bake, &mut saved.bake);
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stock_race_remaps() {
        assert_eq!(scene_token(8), Some("Orc"));
        assert_eq!(scene_token(7), Some("Dwarf"));
        assert_eq!(scene_token(5), Some("Scourge"));
        assert_eq!(scene_token(0), None);
    }
}

#[cfg(test)]
mod isolation_tests {
    use super::*;
    fn restore_inputs(
        mut saved: ResMut<StageInputs>,
        mut preview: ResMut<GluePreview>,
        mut bake: ResMut<GluePreviewBake>,
    ) {
        std::mem::swap(&mut *preview, &mut saved.preview);
        std::mem::swap(&mut *bake, &mut saved.bake);
    }
    #[test]
    fn background_inputs_never_replace_the_pet_inputs() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<StageInputs>()
            .init_resource::<GluePreview>()
            .init_resource::<GluePreviewBake>()
            .insert_resource(CompanionSelectorBackground {
                visible: true,
                race: Some(8),
                image: None,
                ready: false,
            });
        app.world_mut().resource_mut::<GluePreview>().yaw = 1.25;
        app.world_mut().resource_mut::<GluePreviewBake>().revision = 42;
        app.add_systems(
            Update,
            (
                enter_stage,
                |preview: Res<GluePreview>, bake: Res<GluePreviewBake>| {
                    assert!(preview.scene.as_ref().unwrap().path().contains("UI_Orc"));
                    assert!(preview.look.is_none());
                    assert_eq!(bake.revision, 0);
                },
                restore_inputs,
            )
                .chain(),
        );
        app.update();
        assert_eq!(app.world().resource::<GluePreview>().yaw, 1.25);
        assert_eq!(app.world().resource::<GluePreviewBake>().revision, 42);
    }
}
