//! The desktop companion uses the same assembled character and authored characterinfo camera.
#[path="companion_motion.rs"]
mod motion;
#[path="companion_animation_player.rs"]
mod animation_player;
#[path="companion_spell_motion.rs"]
mod spell_motion;
#[path="companion_mount.rs"]
mod mounted;
use super::booth::{
    spawn_booth_effects, spawn_booth_model, BoothBillboardSpec, BoothEffects, BoothInstance,
    BoothMotion, BoothPart, BoothRider,
};
use super::{GlueLook, GluePreview, GluePreviewBake, SelectLook};
use crate::companion_renderer::{
    CompanionDisplayAspect, CompanionFrame, CompanionScale, CompanionVisualState,
    CompanionVisualStats,
};
use bevy::{camera::visibility::RenderLayers, prelude::*};
#[derive(Component)]
pub(crate) struct PetRoot;
#[derive(Component)]
pub(crate) struct PetCamera;
#[derive(Resource, Default)]
struct CompanionPresentation {
    sleep_camera: bool,
    model_revision: u64,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<CompanionPresentation>();
    mounted::install(app);
    app.add_systems(Update, (convert_fx_billboards, exclude_billboards_from_hit_mask));
    app.add_systems(PostUpdate, apply_equipment_fx_options);
    app.add_systems(
        Startup,
        (
            setup,
            load_animation_data.after(benilla_assets::AssetSet::Open),
        ),
    )
    .add_systems(Update, (feed, sync, animate).chain())
    .add_systems(
        PostUpdate,
        super::booth::face_booth_billboards.after(bevy::transform::TransformSystems::Propagate),
    )
    .add_systems(
        Update,
        super::booth::push_booth_mat_alpha.after(benilla_world::doodad_anim::sample_mat_anim),
    );
}
#[derive(Resource)]
struct CompanionAnimationData {
    animations: benilla_formats::AnimDataCatalog,
    spells: benilla_formats::SpellCatalog,
    emotes:benilla_formats::EmoteSoundCatalog,
    visuals:Option<benilla_formats::SpellVisualCatalog>,
    item_displays:Option<benilla_formats::ItemDisplayCatalog>,
}
fn load_animation_data(mut commands: Commands, assets: Option<Res<benilla_assets::WorldAssets>>) {
    use benilla_assets::LockRecover;
    let Some(assets) = assets else { return };
    let mut chain = assets.chain.lock_recover();
    if let (Ok(animations), Ok(spells)) = (
        benilla_formats::load_anim_data_catalog(&mut chain),
        benilla_formats::load_spell_catalog(&mut chain),
    ) {
        let emotes=benilla_formats::load_emote_sound_catalog(&mut chain).unwrap_or_default();
        let visuals=benilla_formats::load_spell_visual_catalog(&mut chain).ok();
        let item_displays=benilla_formats::load_item_display_catalog(&mut chain).ok();
        commands.insert_resource(CompanionAnimationData { animations, spells,emotes,visuals,item_displays });
    }
}
fn setup(mut c: Commands) {
    c.spawn((
        PetRoot,
        crate::companion_renderer::CompanionActor,
        Transform::default(),
        Visibility::default(),
        bevy::animation::transition::AnimationTransitions::default(),
    ));
    c.spawn((
        PetCamera,
        crate::companion_surface::DesktopSurface,
        super::BoothCam("companion".into()),
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(Color::NONE),
            ..default()
        },
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        RenderLayers::layer(31),
        Transform::from_xyz(5.5, 1.6, 0.).looking_at(Vec3::Y, Vec3::Y),
    ));
}
fn feed(
    frame: Res<CompanionFrame>,
    options:Res<crate::companion_renderer::CompanionAppearanceOptions>,
    mut preview: ResMut<GluePreview>,
    mut visual: ResMut<CompanionVisualState>,
) {
    let a = &frame.0["appearance"];
    if !a.is_object() {
        visual.ready = false;
        return;
    }
    let u = |key: &str| a[key].as_u64().unwrap_or(0) as u8;
    let mut equipment = [benilla_protocol::CharEnumItem::default(); 19];
    if let Some(items) = a["equipment"].as_array() {
        for item in items {
            let slot = item["slot"].as_u64().unwrap_or(99) as usize;
            if slot >= 19 || options.slot_override(slot as u8)==Some(false) {
                continue;
            }
            let inv = match slot {
                0 => 1,
                2 => 3,
                3 => 4,
                4 => 5,
                5 => 6,
                6 => 7,
                7 => 8,
                8 => 9,
                9 => 10,
                14 => 16,
                15 => 13,
                16 => 22,
                17 => 26,
                18 => 19,
                _ => 0,
            };
            equipment[slot] = benilla_protocol::CharEnumItem {
                display_id: item["display_id"].as_u64().unwrap_or(0) as u32,
                inventory_type: item["inventory_type"].as_u64().unwrap_or(inv) as u8,
            };
        }
    }
    let flags=appearance_flags(a,&options);
    let look = GlueLook::Select(SelectLook {
        race: u("race"),
        sex: u("sex"),
        skin: u("skin"),
        face: u("face"),
        hair_style: u("hair_style"),
        hair_color: u("hair_color"),
        facial_hair: u("facial_hair"),
        flags,
        equipment,
        pet_display_id: 0,
        pet_level: 0,
        pet_family: 0,
    });
    if preview.look != Some(look) {
        visual.ready = false;
        preview.look = Some(look);
    }
}
fn appearance_flags(a:&serde_json::Value,options:&crate::companion_renderer::CompanionAppearanceOptions)->u32{
    let hidden=options.show_helm.map(|show|!show).unwrap_or(a["hide_helm"].as_bool()==Some(true));
    (if hidden{benilla_protocol::CHARACTER_FLAG_HIDE_HELM}else{0})
        |if options.slot_override(14).map(|show|!show).unwrap_or(a["hide_cloak"].as_bool()==Some(true)){benilla_protocol::CHARACTER_FLAG_HIDE_CLOAK}else{0}
}
fn sync(
    mut c: Commands,
    bake: Res<GluePreviewBake>,
    appearance_state: (
        Res<GluePreview>,
        Res<CompanionFrame>,
        ResMut<CompanionVisualState>,
    ),
    creatures: Option<Res<crate::entities::Creatures>>,
    mut palettes: ResMut<benilla_world::rig_palette::RigPalettes>,
    roots: Query<(Entity, Option<&benilla_world::rig_anim::RigPose>), With<PetRoot>>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<PetCamera>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    scale: Res<CompanionScale>,
    display_aspect: Res<CompanionDisplayAspect>,
    mut view: ResMut<CompanionPresentation>,
    time: Res<Time>,
    mut sleep_blend: Local<f32>,
    mut last: Local<u64>,
    mut framing: Local<Option<super::PortraitAnchors>>,
    mut camera_key: Local<Option<(u64,u32,u32,u32,u32)>>,
    render_data: (
        crate::entities::companion_effects::CompanionAttachedModels,
        Res<benilla_world::doodad_anim::UvAnimMaterials>,
        Res<benilla_world::doodad_anim::TintAnimMaterials>,
        Res<benilla_world::lighting::SharedLightBuffer>,
        ResMut<CompanionVisualStats>,
    ),
) {
    let (preview, frame, mut visual) = appearance_state;
    let (attached, uv, tint, light, mut stats) = render_data;
    let animated = |h: &Handle<benilla_assets::materials::WowModelMaterial>| {
        uv.0.contains_key(&h.id()) || tint.0.contains_key(&h.id())
    };
    let Some(creatures) = creatures else { return };
    let Ok((root, pose)) = roots.single() else {
        return;
    };
    if *last != bake.revision && bake.look.is_some() {
        let Some(dm) = creatures.display_rig(bake.display_id) else {
            return;
        };
        c.entity(root).despawn_related::<Children>();
        let parts: Vec<_> = bake
            .parts
            .iter()
            .map(|p| BoothPart {
                skinned: p.skinned_mesh.clone(),
                static_mesh: p.static_mesh.clone(),
                material: p.material.clone(),
                alpha_anim: p.alpha_anim.clone(),
                twins: p.twins.clone(),
                mat_anim: animated(&p.material),
            })
            .collect();
        let riders: Vec<_> = bake
            .riders
            .iter()
            .map(|r| BoothRider {
                mesh: r.mesh.clone(),
                material: r.material.clone(),
                bone: r.bone,
                offset: r.offset,
                twins: r.twins.clone(),
                mat_anim: animated(&r.material),
            })
            .collect();
        let cards: Vec<_> = bake
            .billboards
            .iter()
            .map(|b| BoothBillboardSpec {
                mesh: b.mesh.clone(),
                material: b.material.clone(),
                bone: b.bone,
                offset: b.offset,
                kind: b.kind,
                twins: b.twins.clone(),
                mat_anim: animated(&b.material),
            })
            .collect();
        let effects: Vec<_> = bake
            .effects
            .iter()
            .map(|e| BoothEffects {
                bone: e.bone,
                offset: e.offset,
                emitters: e.emitters.clone(),
                rig: e.rig.clone(),
            })
            .collect();
        let mut booth_rig = spawn_booth_model(
            &mut c,
            &mut palettes,
            root,
            RenderLayers::layer(31),
            &parts,
            &riders,
            dm.inverse_bindposes
                .as_ref()
                .map(|ib| (dm.skeleton, ib, dm.animations)),
            None,
            BoothMotion::Loop,
            bake.grip,
            &cards,
            BoothInstance::default(),
        );
        let (emitter_count, _) = spawn_booth_effects(
            &mut c,
            &mut booth_rig,
            &RenderLayers::layer(31),
            Some(&light.0),
            &effects,
            BoothInstance::default(),
        );
        let mut ribbons = 0;
        for attached_model in attached.ribbons(&bake.riders) {
            let Some(joint) = booth_rig.anchor(&mut c, attached_model.bone) else {
                continue;
            };
            let host = c
                .spawn((
                    Transform::from_translation(attached_model.offset),
                    Visibility::default(),
                    ChildOf(joint),
                    RenderLayers::layer(31),
                    crate::companion_renderer::CompanionEquipmentFx,
                    benilla_world::model_fade::ModelFade(1.0),
                ))
                .id();
            for ribbon in attached_model.ribbons {
                if let Some(e) = benilla_world::ribbons::spawn_ribbon(
                    &mut c,
                    &ribbon,
                    host,
                    false,
                    scale.0,
                    benilla_world::ribbons::RibbonSeq::Fixed(0),
                    Some(host),
                    None,
                ) {
                    c.entity(e).insert(RenderLayers::layer(31));
                    ribbons += 1;
                }
            }
        }
        *stats = CompanionVisualStats {
            particle_emitters: emitter_count,
            billboards: cards.len(),
            ribbons,
            animated_materials: parts.iter().filter(|p| p.mat_anim).count()
                + riders.iter().filter(|r| r.mat_anim).count()
                + cards.iter().filter(|b| b.mat_anim).count(),
        };
        info!("companion authored visual lanes: {:?}", *stats);
        booth_rig.finish(&mut c);
        *framing = creatures.display_anchors(bake.display_id);
        *last = bake.revision;
        view.model_revision = bake.revision;
    }
    visual.ready = bake.look.is_some()
        && bake.look == preview.look
        && !bake.parts.is_empty()
        && pose.is_some();
    if visual.ready && visual.appearance!=frame.0["appearance"] {
        visual.appearance = frame.0["appearance"].clone();
    }
    // Mount framing currently resets then elevates each frame; keep that path intact.
    // Sleep framing follows evaluated bones and must remain dynamic through transitions.
    let can_cache=frame.0["appearance"]["mount_display_id"].as_u64().unwrap_or(0)==0&&!view.sleep_camera&&*sleep_blend<=0.001;
    let key=windows.single().ok().map(|w|(view.model_revision,w.width().to_bits(),w.height().to_bits(),scale.0.to_bits(),display_aspect.0.to_bits()));
    if can_cache&&key.is_some()&&*camera_key==key{return;}
    *camera_key=if can_cache{key}else{None};
    if let Some(a) = framing.as_ref() {
        let aspect = windows
            .single()
            .map(|w| w.width() / (w.height() * 224.0 / 268.0))
            .unwrap_or(0.7);
        let (mut tf, mut proj) = super::framing::body_frame(a, aspect);
        let model_scale = scale.0 * super::framing::pane_root_scale(a, display_aspect.0);
        let yaw = super::framing::pane_camera(a)
            .eye
            .x
            .atan2(-super::framing::pane_camera(a).eye.z)
            * -1.0;
        let target = if view.sleep_camera { 1.0 } else { 0.0 };
        *sleep_blend = (*sleep_blend
            + (target - *sleep_blend) * (1.0 - (-time.delta_secs() * 8.0).exp()))
        .clamp(0.0, 1.0);
        if *sleep_blend > 0.001 {
            let rot = Quat::from_rotation_y(yaw);
            // Sequence header bounds need not cover the animated bone translations. Fit the
            // actual evaluated pose instead, then pad for skin, horns and equipment riders.
            let points: Vec<Vec3> = pose
                .into_iter()
                .flat_map(|p| p.model.iter())
                .map(|m| rot * (Vec3::from(m.translation) * model_scale))
                .filter(|p| p.is_finite())
                .collect();
            let lo = points
                .iter()
                .copied()
                .reduce(Vec3::min)
                .unwrap_or(Vec3::new(-1.5, 0.0, -1.5));
            let hi = points
                .iter()
                .copied()
                .reduce(Vec3::max)
                .unwrap_or(Vec3::new(1.5, 1.0, 1.5));
            let center = (lo + hi) * 0.5;
            let ext = hi - lo;
            let side = if ext.z > ext.x { Vec3::X } else { Vec3::Z };
            let width = ext.x.max(ext.z) + 1.0 * model_scale;
            let height =
                (width / aspect).max(ext.y + 0.4 * ext.x.min(ext.z) + 1.0 * model_scale) * 1.35;
            let eye = center + side * 10.0 + Vec3::Y * 2.5;
            let fit_tf = Transform::from_translation(eye).looking_at(center, Vec3::Y);
            tf.translation = tf.translation.lerp(fit_tf.translation, *sleep_blend);
            tf.rotation = tf.rotation.slerp(fit_tf.rotation, *sleep_blend);
            proj = Projection::Orthographic(OrthographicProjection {
                scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                    viewport_height: height,
                },
                near: 0.01,
                far: 5000.0,
                ..OrthographicProjection::default_3d()
            });
        }
        for (mut t, mut p) in &mut cameras {
            *t = tf;
            *p = Projection::custom(crate::companion_projection::SpeechProjection{inner:proj.clone()});
        }
        c.entity(root).insert(
            Transform::from_scale(Vec3::splat(
                scale.0 * super::framing::pane_root_scale(a, display_aspect.0),
            ))
            .with_rotation(Quat::from_rotation_y(
                super::framing::pane_camera(a)
                    .eye
                    .x
                    .atan2(-super::framing::pane_camera(a).eye.z)
                    * -1.0,
            )),
        );
    }
}
fn animate(
    frame: Res<CompanionFrame>,
    mut roots: Query<
        (
            &benilla_assets::ModelAnimations,
            &mut AnimationPlayer,
            &mut bevy::animation::transition::AnimationTransitions,
        ),
        With<PetRoot>,
    >,
    mut playback: Local<CompanionPlayback>,
    mut gesture: ResMut<crate::companion_renderer::CompanionLocalGesture>,
    data: Option<Res<CompanionAnimationData>>,
    mut presentation: ResMut<CompanionPresentation>,
    time:Res<Time>,
    mut replay:Local<motion::Replay>,
    mount:Res<mounted::MountPresentation>,
) {
    let mut state = frame.0["animation"].clone();
    if frame.0["connected"].as_bool() == Some(false)
        && frame.0["_local_preview"].as_bool() != Some(true)
    {
        state = serde_json::json!({"sleeping":true});
    }
    if let Some(data) = data.as_deref() {
        let recovery = state["auras"].as_array().is_some_and(|a| {
            a.iter().filter_map(|v| v.as_u64()).any(|id| {
                data.spells.get(id as u32).is_some_and(|spell| {
                    spell.aura_interrupt_flags & 0x40000 != 0
                        && spell
                            .effect_apply_aura
                            .iter()
                            .any(|a| matches!(*a, 84 | 85))
                })
            })
        });
        if recovery {
            state["eating"] = true.into();
        }
    }
    let cast_hold=if state["cast_remaining_ms"].as_u64().unwrap_or(0)>0{data.as_ref().and_then(|d|d.visuals.as_ref().and_then(|v|spell_motion::hold(&d.spells,v,state["cast_spell"].as_u64()? as u32,state["channeling"]==true)))}else{None};
    let ranged_item=frame.0["appearance"]["equipment"].as_array().and_then(|items|items.iter().find(|i|i["slot"]==17));
    let ranged_ready=data.as_ref().and_then(|d|spell_motion::ranged_ready(&d.spells,state["auto_repeat_spell"].as_u64()? as u32,ranged_item.and_then(|i|i["item_class"].as_u64()).map(|n|n as u32),ranged_item.and_then(|i|i["item_subclass"].as_u64()).map(|n|n as u32)));
    let desired = animation_name(&state);
    let mut motion_frame=frame.0.clone();
    if let Some(d)=data.as_deref(){
        if let Some(id)=state["emote_state_id"].as_u64().and_then(|id|d.emotes.state_anim(id as u32)){state["state_animation_id"]=id.into();}
        if let Some(events)=motion_frame["visual_events"].as_array_mut(){for e in events{
            let id=match e["kind"].as_str(){
                Some("emote")=>e["emote_id"].as_u64().and_then(|id|d.emotes.anim(id as u32)).map(|id|id as u16),
                Some("attack")=>Some(match e["hand"].as_str(){Some("off")=>87,Some("unarmed")=>16,_=>frame.0["appearance"]["equipment"].as_array().and_then(|items|items.iter().find(|i|i["slot"]==15)).map_or(16,|i|if i["inventory_type"]==17{18}else{17})}),
                Some("spell_go")=>d.visuals.as_ref().and_then(|v|{let spell=e["spell_id"].as_u64()? as u32;spell_motion::release(&d.spells,v,spell).or_else(||spell_motion::ranged_release(&d.spells,v,d.item_displays.as_ref()?,spell,ranged_item?["display_id"].as_u64()? as u32))}),
                Some("kit")=>d.visuals.as_ref().and_then(|v|spell_motion::kit(v,e["kit_id"].as_u64()? as u32)),
                Some("wound")=>Some(if e["critical"]==true{10}else{9}),Some("jump")=>Some(37),Some("land")=>Some(39),_=>None};
            if let Some(id)=id{if matches!(e["kind"].as_str(),Some("spell_go"|"kit")){e["spell_motion"]=true.into();}if e["kind"]=="emote"{e["looping"]=e["emote_id"].as_u64().is_some_and(|v|d.emotes.spec_proc(v as u32)==Some(2)).into();}e["kind"]="animation".into();e["animation_id"]=id.into();}
        }}
    }

    let resolve = |name: &str| {
        data.as_deref()
            .and_then(|d| (0..208u16).find(|&id| d.animations.name(id) == Some(name)))
    };
    // Start edge age clocks even while the avatar rig is loading.
    replay.select(&motion_frame,time.elapsed_secs_f64(),false);
    for (anims, mut player, mut transitions) in &mut roots {
        let fresh = player.is_added() || playback.model_revision != presentation.model_revision;
        if fresh {
            reset_model_animation(
                &mut player,
                &mut transitions,
                &mut playback,
                presentation.model_revision,
            );
        }
        let finished = playback
            .node
            .and_then(|n| player.animation(n))
            .is_none_or(|a| a.is_finished());
        if gesture.active && gesture.selected_request != gesture.request {
            // Only exact clips supported by this model; no fallback to attack or idle.
            let choices: Vec<u16> = [60,64,65,66,67,68,69,70,73,74,75,76,77,78,79,80,81,82,83,84,113]
                .into_iter().filter(|id| Some(*id) != gesture.selected && anims.find(*id).is_some()).collect();
            let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(gesture.request, |v|v.as_nanos() as u64);
            gesture.selected = choices.get(crate::companion_renderer::local_gesture_index(seed,choices.len())).copied();
            gesture.selected_request = gesture.request;
            if gesture.selected.is_none() { gesture.finished = gesture.request; gesture.active = false; }
        }
        if gesture.active && playback.explicit_sequence == gesture.request && finished {
            gesture.finished = gesture.request;
            gesture.active = false;
        }
        let local_gesture = gesture.active.then_some(gesture.selected).flatten();
        let event_finished=finished&&replay.active.is_some_and(|e|e.sequence==playback.explicit_sequence);
        let event=replay.select(&motion_frame,time.elapsed_secs_f64(),event_finished);
        let base=if state["locomotion"]=="fall"&&replay.jumping(time.elapsed_secs_f64()){Some(38)}else if state["moving"]==true {motion::base(&state).or_else(||resolve(desired))}else{state["state_animation_id"].as_u64().filter(|id|*id<208).map(|id|id as u16).or_else(||motion::base(&state))};
        let high=state["dead"]==true||state["stand_state"]==7||state["sleeping"]==true||state["stand_state"]==3||state["stunned"]==true||state["dancing"]==true;
        let casting=state["cast_remaining_ms"].as_u64().unwrap_or(0)>0;
        let moving=state["moving"]==true;
        let base=if !moving&&!casting&&ranged_ready.is_some(){ranged_ready}else if !moving&&!casting&&state["attacking"]==true&&state["stand_state"].as_u64().unwrap_or(0)==0&&state["emote_state_id"].as_u64().unwrap_or(0)==0 {
            Some(frame.0["appearance"]["equipment"].as_array().and_then(|items|items.iter().find(|i|i["slot"]==15&&i["item_id"].as_u64().unwrap_or(0)>0)).map_or(25,|i|if i["inventory_type"]==17{27}else{26}))
        }else{base};
        // Full-body emotes cannot freeze locomotion; movement always retains ownership.
        let overlay_event=event.filter(|e|motion::upper_body_event(e.id)&&data.as_ref().and_then(|d|anims.find(anims.resolve(e.id,&d.animations).id)).is_some_and(|c|c.upper_node.is_some()));
        // A channel can begin with a cast gesture (e.g. throw the fishing line), then hold.
        let channel_release=if state["channeling"]==true{event.filter(|e|frame.0["visual_events"].as_array().is_some_and(|rows|rows.iter().any(|row|row["sequence"]==e.sequence&&row["kind"]=="spell_go"&&row["spell_id"]==state["cast_spell"]))).map(|e|e.id)}else{None};
        let explicit=if high{None}else if local_gesture.is_some(){local_gesture}else if mount.ready{overlay_event.map(|e|e.id).or(channel_release).or(cast_hold).or(Some(91))}else if casting{overlay_event.filter(|e|matches!(e.id,8..=10)).map(|e|e.id).or(channel_release).or(cast_hold)}else if event.is_some_and(|e|matches!(e.id,37|39)){event.map(|e|e.id)}else if moving{overlay_event.map(|e|e.id).or(base)}else{event.map(|e|e.id).or(base)};
        let explicit_sequence=if local_gesture.is_some(){gesture.request}else if explicit.is_some(){event.filter(|e|Some(e.id)==explicit).map_or(0,|e|e.sequence)}else{0};
        let name = next_animation(playback.name, desired, finished);
        presentation.sleep_camera = matches!(name, "SleepDown" | "Sleep" | "SleepUp");
        if !fresh && playback.name == name && playback.explicit_id==explicit && playback.explicit_sequence==explicit_sequence && playback.motion_session==frame.0["visual_session"].as_str().unwrap_or("") && playback.node.is_some() {
            continue;
        }
        let resolved=explicit.or_else(||resolve(name)).map(|id|data.as_ref().map_or(id,|d|anims.resolve(id,&d.animations).id));
        let Some(clip) = resolved
            .and_then(|id| anims.find(id))
            .or_else(|| resolve(desired).and_then(|id| anims.find(id)))
            .or_else(|| anims.find(0))
        else {
            continue;
        };
        let overlay=(local_gesture.is_some_and(motion::upper_body_event)&&(moving||casting||mount.ready||state["stand_state"]==1))||(explicit.is_some_and(|id|event.is_some_and(|e|e.id==id)&&motion::upper_body_event(id))&&(moving||casting||mount.ready||state["stand_state"]==1))||(explicit.is_some()&&explicit==cast_hold&&(moving||mount.ready));
        let new_edge=motion::new_edge(&playback.motion_session,playback.explicit_sequence,frame.0["visual_session"].as_str().unwrap_or(""),explicit_sequence);
        let lower_before=transitions.get_main_animation().and_then(|n|player.animation(n)).map(|a|a.seek_time());
        let using_upper=(name=="EmoteEat"||overlay)&&clip.upper_node.is_some();
        if playback.overlay&&(!using_upper||playback.node!=clip.upper_node){if let Some(old)=playback.node{player.stop(old);}}
        if using_upper {
            let lower=if mount.ready{Some(91)}else if name=="EmoteEat"&&!overlay{Some(97)}else{resolve(name)};
            if let Some(sit) = lower.and_then(|id|data.as_ref().map_or(Some(id),|d|Some(anims.resolve(id,&d.animations).id))).and_then(|id|anims.find(id)) {
                if transitions.get_main_animation()!=Some(sit.node)||player.animation(sit.node).is_none(){
                    transitions.play(&mut player,sit.node,std::time::Duration::from_millis(120)).repeat();
                }else if let Some(active)=player.animation_mut(sit.node){active.repeat();}
            }
            let node = clip.upper_node.unwrap();
            animation_player::upper(&mut player,node,new_edge,local_gesture.is_none()&&(!overlay||event.is_some_and(|e|e.looping)));
            playback.node = Some(node);
        } else {
            // The completed overlay is gone, but its running lower body keeps its clock.
            let keep_lower=playback.overlay&&!new_edge&&transitions.get_main_animation()==Some(clip.node)&&player.animation(clip.node).is_some();
            let active=if keep_lower{player.animation_mut(clip.node).unwrap()}else{transitions.play(&mut player,clip.node,std::time::Duration::from_millis(120))};
            let repeat=if local_gesture.is_some(){false}else if explicit.is_some()&&event.is_some_and(|e|Some(e.id)==explicit){event.is_some_and(|e|e.looping)}else{!matches!(name,"Death"|"SitGroundDown"|"SitGroundUp"|"SleepDown"|"SleepUp")};
            if local_gesture.is_some() { active.set_repeat(bevy::animation::RepeatAnimation::Count(1)); }
            if repeat {
                active.repeat();
            }
            playback.node = Some(clip.node);
        }
        if std::env::var_os("REALM_COMPANION_ANIMATION_TRACE").is_some(){
            let lower_after=transitions.get_main_animation().and_then(|n|player.animation(n)).map(|a|a.seek_time());
            let edge_seek=playback.node.and_then(|n|player.animation(n)).map(|a|a.seek_time());
            info!("companion motion seq={} anim={:?} new_edge={} upper={} lower_before={:?} lower_after={:?} edge_seek={:?}",explicit_sequence,explicit,new_edge,using_upper,lower_before,lower_after,edge_seek);
        }
        playback.name = name;
        playback.explicit_id=explicit;playback.explicit_sequence=explicit_sequence;playback.overlay=using_upper;playback.motion_session=frame.0["visual_session"].as_str().unwrap_or("").into();
    }
}
#[derive(Default)]
struct CompanionPlayback {
    motion_session:String,
    overlay:bool,
    explicit_id:Option<u16>,
    explicit_sequence:u64,
    model_revision: u64,
    name: &'static str,
    node: Option<bevy::animation::graph::AnimationNodeIndex>,
}
fn reset_model_animation(
    player: &mut AnimationPlayer,
    transitions: &mut bevy::animation::transition::AnimationTransitions,
    playback: &mut CompanionPlayback,
    model_revision: u64,
) {
    // The booth's bootstrap Stand is not tracked by AnimationTransitions. Stop it explicitly,
    // also when a new graph replaces the player on an existing ECS component.
    player.stop_all();
    *transitions = default();
    *playback = CompanionPlayback {
        model_revision,
        ..default()
    };
}
fn next_animation(current: &str, desired: &'static str, finished: bool) -> &'static str {
    let sitting = |s: &str| matches!(s, "SitGround" | "EmoteEat");
    if matches!(desired, "Death" | "Run" | "Walk") {
        return desired;
    }
    match current {
        "SitGroundDown" if sitting(desired) => {
            return if finished { desired } else { "SitGroundDown" }
        }
        "SleepDown" if desired == "Sleep" => return if finished { "Sleep" } else { "SleepDown" },
        "SitGroundUp" if !finished => return "SitGroundUp",
        "SleepUp" if !finished => return "SleepUp",
        "SitGround" | "EmoteEat" | "SitGroundDown" if !sitting(desired) => return "SitGroundUp",
        "Sleep" | "SleepDown" if desired != "Sleep" => return "SleepUp",
        _ => {}
    }
    if sitting(desired) && !sitting(current) {
        return "SitGroundDown";
    }
    if desired == "Sleep" && current != "Sleep" {
        return "SleepDown";
    }
    desired
}

fn animation_name(a: &serde_json::Value) -> &'static str {
    if a["sleeping"].as_bool() == Some(true) {
        "Sleep"
    } else if a["dead"].as_bool() == Some(true)||a["stand_state"]==7 {
        "Death"
    } else if a["stand_state"].as_u64() == Some(3) {
        "Sleep"
    } else if a["stunned"].as_bool()==Some(true){
        "Stun"
    } else if a["dancing"].as_bool() == Some(true) {
        "EmoteDance"
    } else if a["moving"].as_bool() == Some(true) {
        if a["walking"].as_bool() == Some(true) {
            "Walk"
        } else {
            "Run"
        }
    } else if a["stand_state"].as_u64() == Some(1) {
        if a["eating"].as_bool() == Some(true) || a["drinking"].as_bool() == Some(true) {
            "EmoteEat"
        } else {
            "SitGround"
        }
    } else if a["cast_remaining_ms"].as_u64().unwrap_or(0) > 0 {
        "SpellPrecast"
    } else {
        "Stand"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn local_head_visibility_only_changes_helm_flag_and_leaves_server_appearance_untouched(){
        let raw=json!({"hide_helm":true,"hide_cloak":true,"hair_style":3,"equipment":[{"slot":0,"item_id":16533}]});let original=raw.clone();
        assert_eq!(appearance_flags(&raw,&Default::default()),benilla_protocol::CHARACTER_FLAG_HIDE_HELM|benilla_protocol::CHARACTER_FLAG_HIDE_CLOAK);
        assert_eq!(appearance_flags(&raw,&crate::companion_renderer::CompanionAppearanceOptions{show_helm:Some(true),..Default::default()}),benilla_protocol::CHARACTER_FLAG_HIDE_CLOAK);
        assert_eq!(appearance_flags(&raw,&crate::companion_renderer::CompanionAppearanceOptions{show_helm:Some(false),..Default::default()}),benilla_protocol::CHARACTER_FLAG_HIDE_HELM|benilla_protocol::CHARACTER_FLAG_HIDE_CLOAK);assert_eq!(raw,original);
    }
    #[test]
    fn local_show_all_overrides_server_helm_and_cloak_flags(){let raw=json!({"hide_helm":true,"hide_cloak":true});let mut options=crate::companion_renderer::CompanionAppearanceOptions{show_helm:Some(true),..Default::default()};options.equipment_visibility.insert(14,true);assert_eq!(appearance_flags(&raw,&options),0);options.equipment_visibility.insert(14,false);assert_eq!(appearance_flags(&raw,&options),benilla_protocol::CHARACTER_FLAG_HIDE_CLOAK);}
    #[test]
    fn sitting_with_buffs_is_not_eating() {
        assert_eq!(
            animation_name(&json!({"stand_state":1,"auras":[1459]})),
            "SitGround"
        );
        assert_eq!(
            animation_name(&json!({"stand_state":1,"drinking":true})),
            "EmoteEat"
        );
    }
    #[test]
    fn death_overrides_movement_and_cast() {
        assert_eq!(
            animation_name(&json!({"dead":true,"moving":true,"cast_remaining_ms":500})),
            "Death"
        );
        assert_eq!(
            animation_name(&json!({"moving":true,"walking":true})),
            "Walk"
        );
        assert_eq!(
            animation_name(&json!({"moving":true,"walking":false})),
            "Run"
        );
    }
    #[test]
    fn sit_and_wake_transitions_finish_once() {
        assert_eq!(next_animation("Stand", "SitGround", false), "SitGroundDown");
        assert_eq!(
            next_animation("SitGroundDown", "EmoteEat", false),
            "SitGroundDown"
        );
        assert_eq!(
            next_animation("SitGroundDown", "EmoteEat", true),
            "EmoteEat"
        );
        assert_eq!(next_animation("EmoteEat", "Stand", false), "SitGroundUp");
        assert_eq!(next_animation("SitGroundUp", "Stand", false), "SitGroundUp");
        assert_eq!(next_animation("SitGroundUp", "Stand", true), "Stand");
        assert_eq!(next_animation("SitGround", "EmoteEat", false), "EmoteEat");
    }
    #[test]
    fn sleep_transitions_and_urgent_interrupts() {
        assert_eq!(next_animation("Stand", "Sleep", false), "SleepDown");
        assert_eq!(next_animation("SleepDown", "Sleep", false), "SleepDown");
        assert_eq!(next_animation("SleepDown", "Sleep", true), "Sleep");
        assert_eq!(next_animation("Sleep", "Stand", false), "SleepUp");
        assert_eq!(next_animation("SleepUp", "Stand", true), "Stand");
        assert_eq!(next_animation("SitGroundDown", "Run", false), "Run");
        assert_eq!(next_animation("SleepDown", "Death", false), "Death");
    }
    #[test]
    fn graph_bootstrap_stand_is_removed_before_sit_and_after_rebuild() {
        use bevy::animation::{graph::AnimationNodeIndex, transition::AnimationTransitions};
        let stand = AnimationNodeIndex::new(0);
        let sit = AnimationNodeIndex::new(1);
        let mut player = AnimationPlayer::default();
        let mut transitions = AnimationTransitions::default();
        player.play(stand).repeat();
        transitions.play(&mut player, sit, std::time::Duration::ZERO);
        assert!(
            player.animation(stand).is_some(),
            "untracked bootstrap Stand reproduces the mix bug"
        );
        let mut state = CompanionPlayback {
            model_revision: 4,
            name: "SitGround",
            node: Some(sit),
            ..Default::default()
        };
        reset_model_animation(&mut player, &mut transitions, &mut state, 5);
        transitions.play(&mut player, sit, std::time::Duration::ZERO);
        assert!(player.animation(stand).is_none());
        assert_eq!(player.playing_animations().count(), 1);
        assert_eq!(state.model_revision, 5);
        assert_eq!(state.name, "");
        player.play(stand).repeat();
        reset_model_animation(&mut player, &mut transitions, &mut state, 6);
        assert_eq!(player.playing_animations().count(), 0);
        assert_eq!(state.model_revision, 6);
    }
}

// The shared world effect spawner emits world-facing cards. A desktop has a pane camera,
// so attach those same authored batches to the pane's existing billboard driver.
fn convert_fx_billboards(
    mut c: Commands,
    models: crate::entities::companion_effects::CompanionFxModels,
    cards: Query<(Entity, &benilla_world::billboard::BillboardCard, &Mesh3d)>,
    joints: Query<(), With<bevy::animation::AnimatedBy>>,
) {
    for (entity, card, mesh) in &cards {
        let (Some(owner), Some(info)) = (card.follows(), models.billboard(&mesh.0)) else {
            continue;
        };
        let offset = if joints.contains(owner) {
            Vec3::ZERO
        } else {
            info.pivot
        };
        c.entity(entity)
            .remove::<benilla_world::billboard::BillboardCard>()
            .insert((
                ChildOf(owner),
                Transform::from_translation(offset),
                super::booth::BoothBillboard::frame(info.kind),
                RenderLayers::layer(31),
            ));
    }
}

fn apply_equipment_fx_options(
    options: Res<crate::companion_renderer::CompanionFxOptions>,
    models: crate::entities::companion_effects::CompanionAttachedModels,
    mut meshes: Query<(
        &Mesh3d,
        &mut Visibility,
        Option<&benilla_world::doodad_anim::MatAnim>,
        Has<crate::companion_renderer::CompanionSpellFx>,
    )>,
    mut particles: Query<
        &mut benilla_world::particles::ParticleEmitter,
        With<crate::companion_renderer::CompanionEquipmentFx>,
    >,
    mut ribbon_hosts: Query<
        &mut benilla_world::model_fade::ModelFade,
        With<crate::companion_renderer::CompanionEquipmentFx>,
    >,
    mut last: Local<Option<bool>>,
) {
    let restoring = *last == Some(false) && options.equipment_effects;
    for (mesh, mut visible, alpha, spell_owned) in &mut meshes {
        if spell_owned {
            continue;
        }
        if models.equipment_glow_mesh(&mesh.0) {
            if !options.equipment_effects {
                *visible = Visibility::Hidden;
            } else if restoring {
                *visible = if alpha.is_none_or(|a| a.current > 0.0) {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
    for mut particle in &mut particles {
        particle.set_frozen(!options.equipment_effects);
    }
    for mut fade in &mut ribbon_hosts {
        fade.0 = if options.equipment_effects { 1.0 } else { 0.0 };
    }
    *last = Some(options.equipment_effects);
}

fn exclude_billboards_from_hit_mask(mut c:Commands,q:Query<Entity,(With<super::booth::BoothBillboard>,Without<crate::companion_renderer::CompanionHitExcluded>)>){for entity in &q{c.entity(entity).insert(crate::companion_renderer::CompanionHitExcluded);}}
