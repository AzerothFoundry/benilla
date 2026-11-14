//! Asynchronous, geometry-only silhouette. Spell planes and particles never become mouse targets.
use crate::companion_renderer::{
    CompanionActor, CompanionEquipmentFx, CompanionHitExcluded, CompanionSpellFx,
};
use bevy::{
    camera::{visibility::RenderLayers, RenderTarget},
    prelude::*,
    render::{
        gpu_readback::{Readback, ReadbackComplete},
        render_resource::{TextureFormat, TextureUsages},
    },
};
const LAYER: usize = 30;
#[derive(Resource, Default)]
pub struct CompanionHitMask {
    pub pose_stamp:u64,
    pub current_stamp:u64,
    pub stable_idle:bool,
    pub top_fraction: f32,
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
    pub valid: bool,
    pub captured_at: f64,
}
impl CompanionHitMask {
    pub fn usable(&self,now:f64,power_saving:bool)->bool{
        self.valid&&(now-self.captured_at<0.5||(power_saving&&self.stable_idle&&self.pose_stamp==self.current_stamp))
    }

    pub fn contains(&self, point: Vec2, size: Vec2) -> bool {
        if !self.valid
            || size.x <= 0.
            || size.y <= 0.
            || point.x < 0.
            || point.y < 0.
            || point.x >= size.x
            || point.y >= size.y
        {
            return false;
        }
        let top=size.y*self.top_fraction;
        if point.y<top{return false;}
        let x = (point.x / size.x * self.width as f32) as i32;
        let y = ((point.y-top) / (size.y-top).max(1.) * self.height as f32) as i32;
        (-2..=2).any(|dy| {
            (-2..=2).any(|dx| {
                let (a, b) = (x + dx, y + dy);
                a >= 0
                    && b >= 0
                    && (a as u32) < self.width
                    && (b as u32) < self.height
                    && self
                        .alpha
                        .get(b as usize * self.width as usize + a as usize)
                        .is_some_and(|v| *v >= 48)
            })
        })
    }
}
#[derive(Component)]
struct MaskCamera;
#[derive(Resource, Default)]
struct Capture {
    image: Handle<Image>,
    size: UVec2,
    pending: bool,
    next: f64,
    dumped: bool,
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<CompanionHitMask>()
        .init_resource::<Capture>()
        .add_systems(PostUpdate, update.before(bevy::camera::CameraUpdateSystems));
}
fn update(
    mut c: Commands,
    time: Res<Time>,
    mut images: ResMut<Assets<Image>>,
    mut cap: ResMut<Capture>,
    mut counts: ResMut<crate::companion_renderer::CompanionRenderCounters>,
    rate: Res<crate::companion_renderer::CompanionMaskRate>,
    mut mask: ResMut<CompanionHitMask>,
    state:(Res<crate::companion_renderer::CompanionVisualState>,Res<crate::companion_renderer::CompanionFrame>,Res<crate::companion_renderer::CompanionLocalGesture>,Res<crate::companion_renderer::CompanionAppearanceOptions>,Res<crate::companion_renderer::CompanionDerivedActivity>),
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    source: Query<(&Transform, &Projection, &Camera), With<crate::portrait::companion_booth::PetCamera>>,
    mut camera: Query<
        (
            &mut Camera,
            &mut Transform,
            &mut Projection,
            &mut RenderTarget,
        ),
        (
            With<MaskCamera>,
            Without<crate::portrait::companion_booth::PetCamera>,
        ),
    >,
    meshes: Query<(Entity, Option<&RenderLayers>), With<Mesh3d>>,
    parents: Query<&ChildOf>,
    roots: Query<(), With<CompanionActor>>,
    excluded: Query<
        (),
        Or<(
            With<CompanionSpellFx>,
            With<CompanionEquipmentFx>,
            With<CompanionHitExcluded>,
        )>,
    >,
) {
    use std::hash::{Hash,Hasher};
    let (visual,frame,gesture,options,derived)=state;
    let mut hash=std::collections::hash_map::DefaultHasher::new();
    frame.0["connected"].hash(&mut hash);frame.0["_local_preview"].hash(&mut hash);frame.0["appearance"].hash(&mut hash);meshes.iter().len().hash(&mut hash);options.show_helm.hash(&mut hash);options.equipment_visibility.hash(&mut hash);
    for key in ["stand_state","sleeping","dancing","dead","mounted","moving","locomotion","cast_spell","auto_repeat_spell","stealthed","stunned","emote_state_id"]{frame.0["animation"][key].hash(&mut hash);}
    derived.0.hash(&mut hash);gesture.active.hash(&mut hash);if gesture.active{gesture.request.hash(&mut hash);}
    let stamp=hash.finish();
    if mask.current_stamp!=stamp{mask.current_stamp=stamp;mask.valid=false;}
    let a=&frame.0["animation"];
    mask.stable_idle=!derived.0&&a["dancing"]!=true&&!gesture.active&&a["moving"]!=true&&a["dead"]!=true&&a["stealthed"]!=true&&a["stunned"]!=true&&matches!(a["stand_state"].as_u64().unwrap_or(0),0|1|3)&&a["emote_state_id"].as_u64().unwrap_or(0)==0&&a["cast_spell"].as_u64().unwrap_or(0)==0&&a["auto_repeat_spell"].as_u64().unwrap_or(0)==0&&matches!(a["locomotion"].as_str(),None|Some("stand"));
    if let Ok((mut cam, _, _, _)) = camera.single_mut() {
        cam.is_active = false;
    }
    if !visual.ready || window.single().is_ok_and(|w|!w.visible) {
        mask.valid = false;
        return;
    }
    let (Ok(w), Ok((pose, projection, source_camera))) = (window.single(), source.single()) else {
        return;
    };
    let top_fraction=source_camera.viewport.as_ref().map_or(0.,|v|v.physical_position.y as f32/w.physical_height().max(1) as f32);
    mask.top_fraction=top_fraction;
    let size = UVec2::new(
        256,
        (256. * w.height() * (1.-top_fraction) / w.width().max(1.))
            .round()
            .clamp(1., 2048.) as u32,
    );
    if cap.size != size {
        mask.valid = false;
        if cap.pending {
            return;
        }
        let mut image =
            Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        cap.image = images.add(image);
        cap.size = size;
    }
    if rate.hz<=0.&&mask.valid&&std::env::var_os("REALM_COMPANION_HIT_MASK_DUMP").is_none(){return;}
    if cap.pending || time.elapsed_secs_f64() < cap.next {
        return;
    }
    for (e, layers) in &meshes {
        let mut node = e;
        let mut ours = false;
        let mut skip = false;
        for _ in 0..64 {
            if excluded.contains(node) {
                skip = true;
                break;
            }
            if roots.contains(node) {
                ours = true;
                break;
            }
            let Ok(p) = parents.get(node) else { break };
            node = p.parent();
        }
        if skip {
            if let Some(l) = layers.filter(|l| l.intersects(&RenderLayers::layer(LAYER))) {
                c.entity(e).insert(l.clone().without(LAYER));
            }
        }
        if ours && !skip {
            let mut l = layers.cloned().unwrap_or_default();
            if !l.intersects(&RenderLayers::layer(LAYER)) {
                l = l.with(LAYER);
                c.entity(e).insert(l);
            }
        }
    }
    if let Ok((mut cam, mut t, mut p, mut target)) = camera.single_mut() {
        cam.is_active = true;
        cam.viewport = None;
        *t = *pose;
        *p = projection.clone();
        *target = RenderTarget::Image(cap.image.clone().into());
    } else {
        c.spawn((
            MaskCamera,
            Camera3d::default(),
            Camera {
                is_active: true,
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(cap.image.clone().into()),
            RenderLayers::layer(LAYER),
            *pose,
            projection.clone(),
            bevy::core_pipeline::tonemapping::Tonemapping::None,
        ));
    }
    // Target changes only on scale/aspect changes; update existing target too.
    if counts.enabled{counts.mask_captures+=1;}
    cap.pending = true;
    cap.next = time.elapsed_secs_f64() + 1.0 / rate.hz.clamp(5.,30.);
    let dimensions = size;
    let image = cap.image.clone();
    c.spawn(Readback::texture(image)).observe(
        move |event: On<ReadbackComplete>,
              mut c: Commands,
              mut out: ResMut<CompanionHitMask>,
              mut capture: ResMut<Capture>,
              mut counts: ResMut<crate::companion_renderer::CompanionRenderCounters>,
              time: Res<Time>| {
            if counts.enabled{counts.mask_readbacks+=1;}
            let bytes = &event.data;
            if dimensions == capture.size && out.current_stamp==stamp
                && bytes.len() >= dimensions.x as usize * dimensions.y as usize * 4
            {
                out.width = dimensions.x;
                out.height = dimensions.y;
                out.alpha = bytes
                    .chunks_exact(4)
                    .take((dimensions.x * dimensions.y) as usize)
                    .map(|v| v[3])
                    .collect();
                out.pose_stamp=stamp;
                out.valid = out.alpha.iter().any(|alpha|*alpha>=48);
                out.captured_at = time.elapsed_secs_f64();
                if !capture.dumped && time.elapsed_secs_f64()>3.0 && out.alpha.iter().any(|a| *a >= 48) {
                    if let Some(path) = std::env::var_os("REALM_COMPANION_HIT_MASK_DUMP") {
                        let mut pgm =
                            format!("P5\n{} {}\n255\n", out.width, out.height).into_bytes();
                        pgm.extend_from_slice(&out.alpha);
                        if std::fs::write(path, pgm).is_ok() {
                            capture.dumped = true;
                        }
                    }
                }
            }
            capture.pending = false;
            c.entity(event.entity).despawn();
        },
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transparent_area_and_bounds_are_not_targets() {
        let mut m = CompanionHitMask {
            width: 16,
            height: 16,
            alpha: vec![0; 256],
            valid: true,
            captured_at: 0.,
            ..Default::default()
        };
        m.alpha[8 * 16 + 8] = 255;
        assert!(m.contains(Vec2::splat(8.), Vec2::splat(16.)));
        assert!(!m.contains(Vec2::ZERO, Vec2::splat(16.)));
        assert!(!m.contains(Vec2::new(-1., 8.), Vec2::splat(16.)));
        m.valid = false;
        assert!(!m.contains(Vec2::splat(8.), Vec2::splat(16.)));
    }
}
