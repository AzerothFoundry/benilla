//! Character-pane-only mounted visual. No world object or network state is fabricated.
use super::{
    super::booth::{spawn_booth_model, BoothInstance, BoothMotion, BoothPart, BoothTwins},
    PetRoot,
};
use crate::companion_renderer::CompanionFrame;
use bevy::{camera::visibility::RenderLayers, prelude::*};
#[derive(Resource, Default)]
pub(super) struct MountPresentation {
    pub ready: bool,
    display: u32,
    request: Option<Entity>,
    body: Option<Entity>,
    seat: Option<Entity>,
    revision: u64,
}
#[derive(Component)]
struct MountPreview;
#[derive(Component)]
struct MountAnimation(u16);
pub(super) fn install(app: &mut App) {
    app.init_resource::<MountPresentation>().add_systems(
        Update,
        (
            sync.after(super::sync).before(super::animate),
            animate.after(sync),
            fit_camera.after(sync),
        ),
    );
}
fn sync(
    mut c: Commands,
    frame: Res<CompanionFrame>,
    presentation: Res<super::CompanionPresentation>,
    mut state: ResMut<MountPresentation>,
    creatures: Option<Res<crate::entities::Creatures>>,
    mut palettes: ResMut<benilla_world::rig_palette::RigPalettes>,
    roots: Query<Entity, With<PetRoot>>,
    mut poses: Query<&mut benilla_world::rig_anim::RigPose>,
    mounts: Query<Entity, With<MountPreview>>,
    render_state: (
        Res<benilla_world::doodad_anim::UvAnimMaterials>,
        Res<benilla_world::doodad_anim::TintAnimMaterials>,
        Res<benilla_world::lighting::SharedLightBuffer>,
    ),
) {
    let Ok(root) = roots.single() else { return };
    let id = mount_display(&frame.0);
    if state.display != id
        || state.revision != presentation.model_revision
        || state.body.is_some_and(|e| mounts.get(e).is_err())
    {
        // Rider anchors must leave the seat before the mount subtree is removed.
        if let Ok(mut rig) = poses.get_mut(root) {
            if rig.joints_root != root {
                let anchors: Vec<_> = rig.anchors.iter().map(|(_, a)| *a).collect();
                c.entity(root).add_children(&anchors);
                rig.joints_root = root;
                rig.pose_dirty = true;
            }
        }
        if let Some(e) = state.body.take() {
            c.entity(e).try_despawn();
        }
        if let Some(e) = state.request.take() {
            c.entity(e).try_despawn();
        }
        state.display = id;
        state.revision = presentation.model_revision;
        state.ready = false;
        state.seat = None;
    }
    if id == 0 {
        return;
    }
    let Some(creatures) = creatures else { return };
    if state.request.is_none() {
        state.request = Some(
            c.spawn((
                crate::net::NetEntity {
                    kind: benilla_protocol::EntityKind::Unit,
                    display_id: Some(id),
                    scale: 1.,
                },
                crate::entities::VisualAttached,
                Transform::default(),
                Visibility::Hidden,
            ))
            .id(),
        );
        return;
    }
    if state.ready {
        if let (Some(seat), Ok(mut rider)) = (state.seat, poses.get_mut(root)) {
            if rider.joints_root != seat {
                let anchors: Vec<_> = rider.anchors.iter().map(|(_, a)| *a).collect();
                c.entity(seat).add_children(&anchors);
                rider.joints_root = seat;
                rider.pose_dirty = true;
            }
        }
        return;
    }
    let Some((parts, cards)) = creatures.display_mirror(id) else {
        return;
    };
    let Some(dm) = creatures.display_rig(id) else {
        return;
    };
    let Some((bone, offset, scale)) = creatures.companion_mount_seat(id) else {
        return;
    };
    let body = c
        .spawn((
            MountPreview,
            MountAnimation(u16::MAX),
            Transform::from_scale(Vec3::splat(scale)),
            Visibility::default(),
            bevy::animation::transition::AnimationTransitions::default(),
        ))
        .id();
    c.entity(root).add_child(body);
    let (uv, tint, light) = render_state;
    let animated = |m: &Handle<benilla_assets::materials::WowModelMaterial>| {
        uv.0.contains_key(&m.id()) || tint.0.contains_key(&m.id())
    };
    let cards: Vec<_> = cards
        .iter()
        .map(|p| super::super::booth::BoothBillboardSpec {
            mesh: p.mesh.clone(),
            material: p.material.clone(),
            bone: p.bone,
            offset: Vec3::ZERO,
            kind: p.kind,
            twins: BoothTwins::default(),
            mat_anim: animated(&p.material),
        })
        .collect();
    let parts: Vec<_> = parts
        .iter()
        .map(|p| BoothPart {
            skinned: p.skinned_mesh.clone(),
            static_mesh: p.static_mesh.clone(),
            material: p.material.clone(),
            alpha_anim: None,
            twins: BoothTwins::default(),
            mat_anim: animated(&p.material),
        })
        .collect();
    let mut model = spawn_booth_model(
        &mut c,
        &mut palettes,
        body,
        RenderLayers::layer(31),
        &parts,
        &[],
        dm.inverse_bindposes
            .as_ref()
            .map(|b| (dm.skeleton, b, dm.animations)),
        None,
        BoothMotion::Loop,
        [false, false],
        &cards,
        BoothInstance::default(),
    );
    let Some(joint) = model.anchor(&mut c, bone) else {
        c.entity(body).try_despawn();
        return;
    };
    let seat = c
        .spawn((
            seat_transform(offset, scale),
            Visibility::default(),
            benilla_world::rig_anim::RigFrame(root),
        ))
        .id();
    c.entity(joint).add_child(seat);
    if let Some(emitters) = creatures.companion_mount_emitters(id) {
        super::super::booth::spawn_booth_own_emitters(
            &mut c,
            &mut model,
            body,
            &RenderLayers::layer(31),
            Some(&light.0),
            emitters,
            benilla_world::particles::EmitClock::Host(body),
        );
    }
    model.finish(&mut c);
    if let Ok(mut rider) = poses.get_mut(root) {
        let anchors: Vec<_> = rider.anchors.iter().map(|(_, a)| *a).collect();
        c.entity(seat).add_children(&anchors);
        rider.joints_root = seat;
        rider.pose_dirty = true;
    }
    state.body = Some(body);
    state.seat = Some(seat);
    state.ready = true;
    info!(
        "companion mount ready: display={} parts={} seat_bone={} scale={}",
        id,
        parts.len(),
        bone,
        scale
    );
}

fn animate(
    frame: Res<CompanionFrame>,
    data: Option<Res<super::CompanionAnimationData>>,
    mut roots: Query<
        (
            &benilla_assets::ModelAnimations,
            &mut AnimationPlayer,
            &mut MountAnimation,
        ),
        With<MountPreview>,
    >,
) {
    let id = if frame.0["animation"]["moving"].as_bool() == Some(true) {
        if frame.0["animation"]["walking"].as_bool() == Some(true) {
            4
        } else {
            5
        }
    } else {
        0
    };
    for (anims, mut player, mut held) in &mut roots {
        if held.0 == id {
            continue;
        }
        let clip = if let Some(data) = data.as_ref() {
            let resolved = anims.resolve(id, &data.animations);
            anims.find(resolved.id)
        } else {
            anims.find(id)
        }
        .or_else(|| anims.find(0));
        if let Some(clip) = clip {
            player.stop_all();
            player.play(clip.node).repeat();
            held.0 = id;
        }
    }
}

fn mount_display(frame: &serde_json::Value) -> u32 {
    if frame["connected"].as_bool() != Some(true)
        || frame["animation"]["dead"].as_bool() == Some(true)
        || frame["animation"]["sleeping"].as_bool() == Some(true)
        || frame["animation"]["dancing"].as_bool() == Some(true)
        || matches!(
            frame["animation"]["stand_state"].as_u64(),
            Some(1 | 3 | 7 | 8)
        )
    {
        return 0;
    }
    frame["appearance"]["mount_display_id"]
        .as_u64()
        .unwrap_or(0) as u32
}
fn seat_transform(offset: Vec3, scale: f32) -> Transform {
    Transform::from_translation(offset).with_scale(Vec3::splat(1. / scale.max(0.001)))
}
fn fit_camera(
    state: Res<MountPresentation>,
    mut cameras: Query<&mut Transform, With<super::PetCamera>>,
    frame: Res<CompanionFrame>,
    creatures: Option<Res<crate::entities::Creatures>>,
    roots: Query<Entity, With<PetRoot>>,
    poses: Query<&benilla_world::rig_anim::RigPose>,
    globals: Query<&GlobalTransform>,
    mut logged: Local<u32>,
) {
    if !state.ready {
        *logged = 0;
        return;
    }
    // Front-elevated presentation: the mount's forward crest must not hide the rider's face.
    // The actual authored attachment-0 seat and both skeletons remain untouched.
    for mut eye in &mut cameras {
        eye.translation *= 1.6;
        eye.translation.y += 2.4;
        eye.look_at(Vec3::Y * 1.8, Vec3::Y);
    }
    if std::env::var_os("REALM_COMPANION_MOUNT_POSE_AUDIT").is_some() && *logged < 11 {
        *logged += 1;
        if *logged == 10 {
            if let (Ok(root), Some(body), Some(seat), Some(creatures)) =
                (roots.single(), state.body, state.seat, creatures.as_deref())
            {
                let sample = |entity: Entity, model: u32, model_frame: Entity| -> Option<Vec3> {
                    let bone = creatures.display_rig(model)?.skeleton.head_bone?;
                    let p = poses
                        .get(entity)
                        .ok()?
                        .model
                        .get(bone as usize)?
                        .translation;
                    Some(
                        globals
                            .get(model_frame)
                            .ok()?
                            .transform_point(Vec3::from(p)),
                    )
                };
                let rider_display =
                    frame.0["appearance"]["display_id"].as_u64().unwrap_or(0) as u32;
                info!(
                    "companion mount pose audit: seat={:?} rider_head={:?} mount_head={:?}",
                    globals.get(seat).ok().map(|g| g.translation()),
                    sample(root, rider_display, seat),
                    sample(body, state.display, body)
                );
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seat_preserves_rider_size_under_differently_scaled_mounts() {
        for scale in [0.5, 1., 2.5] {
            let seat = seat_transform(Vec3::Y * 2., scale);
            let composed =
                Transform::from_scale(Vec3::splat(scale)).compute_affine() * seat.compute_affine();
            assert!((composed.transform_vector3(Vec3::X).length() - 1.).abs() < 1e-5);
            assert!((composed.translation.y - 2. * scale).abs() < 1e-5);
        }
    }
    #[test]
    fn offline_or_dead_character_never_remains_seated_on_a_cached_mount() {
        let mut f = serde_json::json!({"connected":true,"appearance":{"mount_display_id":14339},"animation":{"stand_state":0}});
        assert_eq!(mount_display(&f), 14339);
        f["connected"] = false.into();
        assert_eq!(mount_display(&f), 0);
        f["connected"] = true.into();
        f["animation"]["dead"] = true.into();
        assert_eq!(mount_display(&f), 0);
    }
}
