//! Resolve attached models for the companion by the existing bake's mesh identity, not item IDs.
use super::{equipment::ItemDisplays, item_glow::ItemGlows};
use bevy::{ecs::system::SystemParam, prelude::*};
#[derive(SystemParam)]
pub(crate) struct CompanionAttachedModels<'w> {
    items: Option<Res<'w, ItemDisplays>>,
    glows: Option<Res<'w, ItemGlows>>,
}
pub(crate) struct AttachedRibbons {
    pub bone: u16,
    pub offset: Vec3,
    pub ribbons: Vec<benilla_assets::ModelRibbon>,
}
impl CompanionAttachedModels<'_> {
    pub(crate) fn equipment_glow_mesh(&self, mesh: &Handle<Mesh>) -> bool {
        self.glows
            .iter()
            .flat_map(|g| g.models.values())
            .flat_map(|m| m.parts.iter().flatten())
            .any(|p| &p.mesh == mesh)
            || self
                .items
                .iter()
                .flat_map(|g| g.models.values())
                .flat_map(|m| m.parts.iter().flatten())
                .any(|p| &p.mesh == mesh && (p.billboard.is_some() || p.additive))
    }
    pub(crate) fn ribbons(&self, riders: &[crate::portrait::PreviewRider]) -> Vec<AttachedRibbons> {
        let mut result = Vec::new();
        let mut seen: Vec<(u16, Vec3, &super::DisplayModel)> = Vec::new();
        for rider in riders {
            let models = self
                .items
                .iter()
                .flat_map(|r| r.models.values())
                .chain(self.glows.iter().flat_map(|r| r.models.values()));
            if let Some(model) = models
                .filter(|m| !m.ribbons.is_empty())
                .find(|model| model.parts.iter().flatten().any(|p| p.mesh == rider.mesh))
            {
                if seen.iter().any(|(bone, offset, old)| {
                    *bone == rider.bone && *offset == rider.offset && std::ptr::eq(*old, model)
                }) {
                    continue;
                }
                seen.push((rider.bone, rider.offset, model));
                result.push(AttachedRibbons {
                    bone: rider.bone,
                    offset: rider.offset,
                    ribbons: model.ribbons.clone(),
                });
            }
        }
        result
    }
}

pub(crate) fn install_spell_visuals(app: &mut App) {
    super::spell_fx::companion::install(app);
}

#[derive(SystemParam)]
pub(crate) struct CompanionFxModels<'w> {
    fx: Option<Res<'w, super::spell_fx::SpellFx>>,
}
impl CompanionFxModels<'_> {
    pub(crate) fn billboard(&self, mesh: &Handle<Mesh>) -> Option<benilla_assets::BillboardInfo> {
        self.fx
            .as_ref()?
            .models
            .values()
            .flat_map(|m| m.parts.iter().flatten())
            .find(|p| &p.mesh == mesh)
            .and_then(|p| p.billboard.clone())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn distinct_equipment_assets_have_authored_emitters() {
        let path = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::Chain::open(std::path::Path::new(&path)).unwrap();
        for (path, count) in [
            (
                "Item/ObjectComponents/Shoulder/LShoulder_Robe_PVPHorde_C_01.m2",
                3,
            ),
            (
                "Item/ObjectComponents/Shoulder/RShoulder_Robe_PVPHorde_C_01.m2",
                3,
            ),
            ("Item/ObjectComponents/Weapon/Club_1H_Torch_A_01.m2", 1),
        ] {
            let summary = benilla_formats::load_m2_animation_summary(&mut chain, path).unwrap();
            assert_eq!(summary.particle_emitter_count, count, "{path}");
        }
    }
}
