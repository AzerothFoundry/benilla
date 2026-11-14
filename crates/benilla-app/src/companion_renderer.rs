//! Standalone character-pane renderer. No game connection, terrain or gameplay UI.
use bevy::prelude::*;
use std::path::PathBuf;

/// Local presentation only; never changes the authoritative snapshot or emits game commands.
#[derive(Resource, Default)]
pub struct CompanionLocalGesture {
    pub request: u64,
    pub active: bool,
    pub selected: Option<u16>,
    pub selected_request: u64,
    pub finished: u64,
}
/// Mix clock entropy before reducing: platform clocks often return multiples of 1000ns.
pub fn local_gesture_index(mut seed:u64,len:usize)->usize {
    seed=seed.wrapping_add(0x9e3779b97f4a7c15);
    seed=(seed^(seed>>30)).wrapping_mul(0xbf58476d1ce4e5b9);
    seed=(seed^(seed>>27)).wrapping_mul(0x94d049bb133111eb);
    ((seed^(seed>>31)) % len.max(1) as u64) as usize
}
#[derive(Resource)]
pub struct CompanionMaskRate{pub hz:f64}
impl Default for CompanionMaskRate{fn default()->Self{Self{hz:30.}}}
#[derive(Resource,Default)]
pub struct CompanionDerivedActivity(pub bool);
/// Opt-in counts only, never includes character data or credentials.
#[derive(Resource,Default)]
pub struct CompanionRenderCounters{pub enabled:bool,pub mask_captures:u64,pub mask_readbacks:u64}
#[derive(Resource, Default)]
pub struct CompanionFrame(pub serde_json::Value);
/// Per-window visual preferences; recovery actions and food/drink props are always preserved.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompanionFxOptions {
    pub spell_effects: bool,
    pub enchant_effects: bool,
    pub equipment_effects: bool,
}
impl Default for CompanionFxOptions {
    fn default() -> Self {
        Self {
            spell_effects: true,
            enchant_effects: true,
            equipment_effects: true,
        }
    }
}

/// Local display overrides never alter the incoming appearance or its cache.
#[derive(Resource,Default,Clone,Debug,PartialEq,Eq)]
pub struct CompanionAppearanceOptions{pub show_helm:Option<bool>,pub equipment_visibility:std::collections::BTreeMap<u8,bool>}
impl CompanionAppearanceOptions{pub fn slot_override(&self,slot:u8)->Option<bool>{if slot==0{self.show_helm}else{self.equipment_visibility.get(&slot).copied()}}}

#[derive(Component)]
pub(crate) struct CompanionActor;
/// The exact appearance currently assembled; a previous character cannot satisfy readiness.
#[derive(Resource, Default)]
pub struct CompanionVisualState {
    pub ready: bool,
    pub appearance: serde_json::Value,
}
#[derive(Resource)]
pub struct CompanionScale(pub f32);
impl Default for CompanionScale {
    fn default() -> Self {
        Self(1.0)
    }
}

#[derive(Resource)]
pub struct CompanionDisplayAspect(pub f32);
impl Default for CompanionDisplayAspect {
    fn default() -> Self {
        Self(4.0 / 3.0)
    }
}

#[derive(Component)]
pub(crate) struct CompanionEquipmentFx;
#[derive(Component)]
pub(crate) struct CompanionSpellFx;
/// Counts of actual instantiated authored visual lanes, useful for live verification.
#[derive(Resource, Default, Debug)]
pub struct CompanionVisualStats {
    pub particle_emitters: usize,
    pub billboards: usize,
    pub ribbons: usize,
    pub animated_materials: usize,
}

/// Call before DefaultPlugins to register the read-only MPQ source.
pub fn register_companion_assets(app: &mut App, data_dir: &std::path::Path) -> anyhow::Result<()> {
    std::env::set_var("WOW_DATA", data_dir);
    benilla_assets::register_mpq_source(app, data_dir)
}
/// Call after DefaultPlugins; owns only the engine and one character-preview camera.
pub fn add_companion_renderer(app: &mut App, _data_dir: PathBuf) {
    crate::companion_hit_mask::install(app);
    app.init_resource::<CompanionDerivedActivity>()
        .init_resource::<CompanionMaskRate>()
        .init_resource::<CompanionRenderCounters>()
        .init_resource::<CompanionFrame>()
        .init_resource::<CompanionLocalGesture>()
        .init_resource::<CompanionFxOptions>()
        .init_resource::<CompanionAppearanceOptions>()
        .init_resource::<CompanionVisualState>()
        .init_resource::<CompanionVisualStats>()
        .init_resource::<CompanionScale>()
        .init_resource::<CompanionDisplayAspect>()
        .init_resource::<crate::portrait::GluePreview>()
        .init_resource::<crate::portrait::GluePreviewBake>()
        .add_plugins(benilla_world::world_plugins::WorldPlugins);
    benilla_assets::register_asset_loaders(app);
    app.add_plugins(crate::companion_surface::DesktopSurfacePlugin);
    crate::entities::companion_preview(app);
    crate::entities::companion_effects::install_spell_visuals(app);
    crate::portrait::companion_booth::install(app);
    crate::portrait::companion_selector::install(app);
}

pub use crate::companion_hit_mask::CompanionHitMask;

/// Visual-only card/particle geometry is never a native pointer target.
#[derive(Component)]
pub(crate) struct CompanionHitExcluded;

pub use crate::portrait::companion_selector::{CompanionSelectorBackground,CompanionSelectorWindow,scene_token};
