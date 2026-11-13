//! A single-avatar adapter into the client's existing SpellKitFx model/lifecycle engine.
//! Inputs are real native events plus current aura state, never spell-ID-specific guesses.
use super::*;
use crate::companion_renderer::{
    CompanionActor, CompanionEquipmentFx, CompanionFrame, CompanionFxOptions,
};
use crate::creature_anim::spell_visual::FxSlot;
use crate::creature_anim::{FxClass, FxStage};
use benilla_assets::{AssetSet, LockRecover};
use std::collections::BTreeSet;
#[derive(Resource)]
struct KitData {
    spells: benilla_formats::SpellCatalog,
    visuals: benilla_formats::SpellVisualCatalog,
    recovery_kits: BTreeSet<u32>,
}
impl KitData {
    fn new(
        spells: benilla_formats::SpellCatalog,
        visuals: benilla_formats::SpellVisualCatalog,
    ) -> Self {
        let recovery_kits = spells
            .iter()
            .filter(|(_, s)| recovery(s))
            .filter_map(|(_, s)| visuals.stages(s.visual))
            .flat_map(|s| [s.precast, s.cast, s.state, s.impact, s.channel])
            .filter(|id| *id != 0)
            .collect();
        Self {
            spells,
            visuals,
            recovery_kits,
        }
    }
}
fn recovery(s: &benilla_formats::SpellDisplay) -> bool {
    s.aura_interrupt_flags & 0x40000 != 0
        && s.effect_apply_aura.iter().any(|a| matches!(a, 84 | 85))
}
fn recovery_spell(data: &KitData, id: u32) -> bool {
    data.spells.get(id).is_some_and(recovery)
}
#[derive(Resource)]
struct FxPolicy {
    spell_effects: bool,
    sources: std::collections::BTreeMap<(u32, u32, u16), bool>,
}
impl Default for FxPolicy {
    fn default() -> Self {
        Self {
            spell_effects: true,
            sources: default(),
        }
    }
}
#[derive(Default)]
struct Bridge {
    session: String,
    sequence: u64,
    auras: BTreeSet<u32>,
    cast: Option<u32>,
    now_ms: u64,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<FxTintAnims>()
        .init_resource::<FxPolicy>()
        .add_message::<SpellKitFx>()
        .add_systems(Startup, load.after(AssetSet::Open))
        .add_systems(
            Update,
            (
                attachments,
                bridge,
                resolve_spell_fx,
                attach_spell_fx,
                tend_world_plants,
                tag_layers,
            )
                .chain(),
        )
        .add_systems(Update, (tick_fx_tint, advance_fx_anim));
}
fn load(mut c: Commands, assets: Option<Res<benilla_assets::WorldAssets>>) {
    let Some(a) = assets else { return };
    let mut chain = a.chain.lock_recover();
    if let (Ok(spells), Ok(visuals)) = (
        benilla_formats::load_spell_catalog(&mut chain),
        benilla_formats::load_spell_visual_catalog(&mut chain),
    ) {
        c.insert_resource(KitData::new(spells, visuals));
    }
}
fn attachments(
    mut c: Commands,
    roots: Query<(Entity, Option<&benilla_world::rig_anim::RigPose>), With<CompanionActor>>,
    bake: Res<crate::portrait::GluePreviewBake>,
    creatures: Option<Res<super::super::Creatures>>,
    mut last: Local<Option<u32>>,
) {
    let (Ok((root, Some(_))), Some(creatures)) = (roots.single(), creatures) else {
        return;
    };
    if *last == Some(bake.display_id) {
        return;
    }
    let Some(dm) = creatures.models.get(&bake.display_id) else {
        return;
    };
    c.entity(root).insert((
        super::super::VisualAttached,
        super::super::BoneAttach {
            points: dm
                .attachments
                .iter()
                .map(|a| (a.id, (a.bone, a.offset)))
                .collect(),
            markers: dm
                .markers
                .iter()
                .map(|a| (a.ident, (a.bone, a.offset)))
                .collect(),
        },
    ));
    *last = Some(bake.display_id);
}
fn kit(
    out: &mut MessageWriter<SpellKitFx>,
    policy: &mut FxPolicy,
    data: &KitData,
    root: Entity,
    spell: u32,
    id: u32,
    persistent: bool,
    class: FxClass,
    stage: FxStage,
) {
    let is_recovery = recovery_spell(data, spell) || spell == 0 && data.recovery_kits.contains(&id);
    if !policy.spell_effects && !is_recovery {
        return;
    }
    let effects = kit_slots(data, id);
    for effect in &effects {
        if policy.sources.len() >= 1024 {
            policy.sources.pop_first();
        }
        policy
            .sources
            .insert((spell, effect.effect, effect.tag), is_recovery);
    }
    if effects.is_empty() {
        return;
    }
    info!(
        "companion native visual kit={id} spell={spell} effects={} persistent={persistent}",
        effects.len()
    );
    out.write(SpellKitFx::Begin {
        entity: root,
        spell_id: spell,
        persistent,
        class,
        stage,
        effects,
    });
}
fn kit_slots(data: &KitData, id: u32) -> Vec<FxSlot> {
    data.visuals
        .kit(id)
        .into_iter()
        .flat_map(|k| k.effects())
        .filter_map(|(tag, effect)| {
            data.visuals.effect_path(effect).map(|p| FxSlot {
                tag,
                effect,
                path: p.into(),
            })
        })
        .collect()
}
fn fresh_event(initial: bool, kind: &str, now: u64, at: u64) -> bool {
    let max_age = if initial {
        if kind == "kit" {
            1500
        } else {
            750
        }
    } else {
        2000
    };
    now.saturating_sub(at) <= max_age
}
fn stage(data: &KitData, spell: u32, which: &str) -> u32 {
    let Some(v) = data
        .spells
        .get(spell)
        .and_then(|s| data.visuals.stages(s.visual))
    else {
        return 0;
    };
    match which {
        "precast" => v.precast,
        "cast" => v.cast,
        "state" => v.state,
        "channel" => v.channel,
        _ => 0,
    }
}
fn reap(out: &mut MessageWriter<SpellKitFx>, root: Entity, spell: u32, class: FxClass) {
    out.write(SpellKitFx::Reap {
        entity: root,
        spell_id: spell,
        class,
        force: true,
    });
}
fn bridge(
    frame: Res<CompanionFrame>,
    data: Option<Res<KitData>>,
    roots: Query<Entity, (With<CompanionActor>, With<super::super::VisualAttached>)>,
    mut state: Local<Bridge>,
    mut out: MessageWriter<SpellKitFx>,
    mut c: Commands,
    mut attached: Query<&mut FxAttached>,
    emitters: Query<(Entity, &benilla_world::particles::ParticleEmitter)>,
    options: Res<CompanionFxOptions>,
    mut policy: ResMut<FxPolicy>,
) {
    let (Some(data), Ok(root)) = (data, roots.single()) else {
        return;
    };
    let session = frame.0["visual_session"].as_str().unwrap_or("").to_owned();
    let now = frame.0["now_ms"].as_u64().unwrap_or(0);
    if state.session != session
        || now < state.now_ms
        || frame.0["_local_preview"].as_bool() == Some(true) && state.now_ms > 0
    {
        if let Ok(mut effects) = attached.get_mut(root) {
            for effect in &effects.instances {
                if let Some(root) = effect.root {
                    clear_effect_particles(&mut c, &emitters, root);
                    c.entity(root).try_despawn();
                }
            }
            effects.instances.clear();
        }
        for id in &state.auras {
            reap(&mut out, root, *id, FxClass::AuraState);
        }
        if let Some(id) = state.cast {
            reap(&mut out, root, id, FxClass::Hold);
        }
        *state = Bridge {
            session,
            ..default()
        };
    }
    if policy.spell_effects != options.spell_effects {
        if !options.spell_effects {
            if let Ok(mut effects) = attached.get_mut(root) {
                effects.instances.retain(|i| {
                    if policy.sources.get(&(i.spell_id, i.effect, i.tag)) == Some(&true) {
                        true
                    } else {
                        if let Some(root) = i.root {
                            clear_effect_particles(&mut c, &emitters, root);
                            c.entity(root).try_despawn();
                        }
                        false
                    }
                });
            }
        }
        policy.spell_effects = options.spell_effects;
        state.auras.retain(|id| recovery_spell(&data, *id));
        if state.cast.is_some_and(|id| !recovery_spell(&data, id)) {
            state.cast = None;
        }
        state.sequence = frame.0["visual_sequence"]
            .as_u64()
            .unwrap_or(state.sequence);
    }
    state.now_ms = now;
    let connected = frame.0["connected"].as_bool() == Some(true);
    let auras: BTreeSet<u32> = if connected {
        frame.0["animation"]["auras"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_u64().map(|v| v as u32))
            .filter(|id| options.spell_effects || recovery_spell(&data, *id))
            .collect()
    } else {
        BTreeSet::new()
    };
    for id in auras.difference(&state.auras) {
        kit(
            &mut out,
            &mut policy,
            &data,
            root,
            *id,
            stage(&data, *id, "state"),
            true,
            FxClass::AuraState,
            FxStage::State,
        );
    }
    for id in state.auras.difference(&auras) {
        reap(&mut out, root, *id, FxClass::AuraState);
    }
    state.auras = auras;
    if !connected {
        if let Some(id) = state.cast.take() {
            reap(&mut out, root, id, FxClass::Hold);
        }
        return;
    }
    let initial = state.sequence == 0;
    for event in frame.0["visual_events"].as_array().into_iter().flatten() {
        let seq = event["sequence"].as_u64().unwrap_or(0);
        if seq <= state.sequence {
            continue;
        }
        state.sequence = seq;
        if event["at_ms"]
            .as_u64()
            .is_some_and(|t| !fresh_event(initial, event["kind"].as_str().unwrap_or(""), now, t))
        {
            continue;
        }
        let spell = event["spell_id"].as_u64().unwrap_or(0) as u32;
        match event["kind"].as_str().unwrap_or("") {
            "kit" => kit(
                &mut out,
                &mut policy,
                &data,
                root,
                0,
                event["kit_id"].as_u64().unwrap_or(0) as u32,
                false,
                FxClass::Hold,
                FxStage::OneShot,
            ),
            "spell_start" => {
                if let Some(old) = state.cast.take() {
                    reap(&mut out, root, old, FxClass::Hold);
                }
                state.cast = Some(spell);
                kit(
                    &mut out,
                    &mut policy,
                    &data,
                    root,
                    spell,
                    stage(&data, spell, "precast"),
                    true,
                    FxClass::Hold,
                    FxStage::Relive,
                );
            }
            "spell_go" => {
                if let Some(old) = state.cast.take() {
                    reap(&mut out, root, old, FxClass::Hold);
                }
                kit(
                    &mut out,
                    &mut policy,
                    &data,
                    root,
                    spell,
                    stage(&data, spell, "cast"),
                    false,
                    FxClass::Hold,
                    FxStage::OneShot,
                );
            }
            "spell_cancel" => {
                if let Some(old) = state.cast.take() {
                    reap(&mut out, root, old, FxClass::Hold);
                }
            }
            _ => {}
        }
    }
    // A client joining mid-cast can recover the current hold from its truthful snapshot,
    // without inventing a past GO or replaying old release flashes from the event ring.
    let observed = frame.0["animation"]["cast_spell"]
        .as_u64()
        .filter(|_| {
            frame.0["animation"]["cast_remaining_ms"]
                .as_u64()
                .unwrap_or(0)
                > 0
        })
        .map(|v| v as u32);
    if observed != state.cast {
        if let Some(old) = state.cast.take() {
            reap(&mut out, root, old, FxClass::Hold);
        }
        if let Some(id) = observed {
            kit(
                &mut out,
                &mut policy,
                &data,
                root,
                id,
                stage(&data, id, "precast"),
                true,
                FxClass::Hold,
                FxStage::Relive,
            );
            state.cast = Some(id);
        }
    }
}

fn clear_effect_particles(
    c: &mut Commands,
    emitters: &Query<(Entity, &benilla_world::particles::ParticleEmitter)>,
    root: Entity,
) {
    for (entity, emitter) in emitters {
        if emitter.anchor() == Some(root) {
            c.entity(entity).try_despawn();
        }
    }
}
fn tag_layers(
    mut c: Commands,
    spell_hosts: Query<&FxAttached>,
    mut particles: Query<(Entity, &mut benilla_world::particles::ParticleEmitter)>,
    parents: Query<&ChildOf>,
    parts: Query<(
        Entity,
        &benilla_world::model_render::ModelPart,
        Option<&bevy::camera::visibility::RenderLayers>,
    )>,
    emitters: Query<
        (Entity, Option<&bevy::camera::visibility::RenderLayers>),
        Or<(
            With<benilla_world::particles::ParticleEmitter>,
            With<benilla_world::ribbons::RibbonTrail>,
        )>,
    >,
) {
    let spell_roots: BTreeSet<_> = spell_hosts
        .iter()
        .flat_map(|s| s.instances.iter().filter_map(|i| i.root))
        .collect();
    for (entity, mut particle) in &mut particles {
        if let Some(anchor) = particle.anchor() {
            if spell_roots.contains(&anchor) {
                particle.set_frozen(false);
                c.entity(entity).remove::<CompanionEquipmentFx>();
            } else {
                c.entity(entity).insert(CompanionEquipmentFx);
            }
        }
    }
    let layer = bevy::camera::visibility::RenderLayers::layer(31);
    for (e, p, l) in &parts {
        let mut owner = e;
        let mut spell_owned = false;
        for _ in 0..64 {
            if spell_roots.contains(&owner) {
                spell_owned = true;
                break;
            }
            let Ok(parent) = parents.get(owner) else {
                break;
            };
            owner = parent.parent();
        }
        if spell_owned {
            c.entity(e)
                .insert(crate::companion_renderer::CompanionSpellFx);
        } else {
            c.entity(e)
                .remove::<crate::companion_renderer::CompanionSpellFx>();
        }
        if p.kind == benilla_world::model_render::ModelKind::Creature && l != Some(&layer) {
            c.entity(e).insert(layer.clone());
        }
    }
    for (e, l) in &emitters {
        if l != Some(&layer) {
            c.entity(e).insert(layer.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_snapshot_does_not_replay_old_flashes_but_regular_polling_keeps_new_events() {
        assert!(!fresh_event(true, "spell_go", 10_000, 8_000));
        assert!(fresh_event(true, "kit", 10_000, 9_000));
        assert!(fresh_event(false, "spell_go", 10_000, 9_001));
        assert!(!fresh_event(false, "kit", 10_000, 1_000));
    }
    #[test]
    fn actual_spell_data_resolves_drink_cast_and_food_state_without_special_case() {
        let Some(path) = std::env::var_os("WOW_DATA").filter(|p| !p.is_empty()) else {
            return;
        };
        let mut chain = benilla_formats::Chain::open(std::path::Path::new(&path)).unwrap();
        let data = KitData::new(
            benilla_formats::load_spell_catalog(&mut chain).unwrap(),
            benilla_formats::load_spell_visual_catalog(&mut chain).unwrap(),
        );
        let drink = stage(&data, 1137, "cast");
        assert_eq!(drink, 438);
        assert_eq!(stage(&data, 1137, "state"), 0);
        assert!(kit_slots(&data, drink)
            .iter()
            .any(|s| s.tag == 22 && s.path.to_lowercase().contains("tankard")));
        let food = stage(&data, 1131, "state");
        assert_eq!(food, 409);
        assert!(kit_slots(&data, food)
            .iter()
            .any(|s| s.tag == 22 && s.path.to_lowercase().contains("bread")));
    }
}
