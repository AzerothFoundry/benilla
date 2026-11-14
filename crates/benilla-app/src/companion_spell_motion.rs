//! Unit body animation from the same SpellVisualKit rows as the game's visual engine.
use benilla_formats::{SpellCatalog, SpellVisualCatalog};
fn stage(
    spells: &SpellCatalog,
    visuals: &SpellVisualCatalog,
    spell: u32,
) -> Option<benilla_formats::VisualStages> {
    visuals.stages(spells.get(spell)?.visual).copied()
}
pub(super) fn kit(visuals: &SpellVisualCatalog, id: u32) -> Option<u16> {
    visuals.kit(id)?.positive_anim().filter(|a| *a < 208)
}
pub(super) fn hold(
    spells: &SpellCatalog,
    visuals: &SpellVisualCatalog,
    spell: u32,
    channeling: bool,
) -> Option<u16> {
    let stages = stage(spells, visuals, spell)?;
    if channeling {
        kit(visuals, stages.channel).or_else(|| kit(visuals, stages.precast))
    } else {
        kit(visuals, stages.precast)
    }
}
pub(super) fn release(
    spells: &SpellCatalog,
    visuals: &SpellVisualCatalog,
    spell: u32,
) -> Option<u16> {
    kit(visuals, stage(spells, visuals, spell)?.cast)
}
/// `0x60d450`: ranged-slot spells merge the worn ranged display's visual into empty slots.
pub(super) fn ranged_release(
    spells: &SpellCatalog,
    visuals: &SpellVisualCatalog,
    displays: &benilla_formats::ItemDisplayCatalog,
    spell: u32,
    display: u32,
) -> Option<u16> {
    let def = spells.get(spell)?;
    if !def.ranged_slot() {
        return None;
    }
    let weapon = visuals.stages(displays.get(display)?.spell_visual)?;
    let stages = visuals
        .stages(def.visual)
        .copied()
        .unwrap_or_default()
        .merged_over_weapon(weapon);
    kit(visuals, stages.cast)
}
pub(super) fn ranged_ready(
    spells: &SpellCatalog,
    spell: u32,
    item_class: Option<u32>,
    subclass: Option<u32>,
) -> Option<u16> {
    if !spells.get(spell)?.auto_repeat() || item_class != Some(2) {
        return None;
    }
    match subclass? {
        2 => Some(109),
        3 | 18 => Some(110),
        19 => Some(111),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_kit_is_not_a_guessed_animation() {
        let v = SpellVisualCatalog::from_tables(Default::default(), Default::default());
        assert!(kit(&v, 99999).is_none());
    }
    #[test]
    fn actual_spell_rows_supply_body_actions_when_data_is_available() {
        let path = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::Chain::open(std::path::Path::new(&path)).unwrap();
        let s = benilla_formats::load_spell_catalog(&mut chain).unwrap();
        let v = benilla_formats::load_spell_visual_catalog(&mut chain).unwrap();
        for id in [116, 133, 7620, 2366, 2575, 2538, 5143] {
            println!(
                "spell {id} hold={:?} channel={:?} release={:?}",
                hold(&s, &v, id, false),
                hold(&s, &v, id, true),
                release(&s, &v, id)
            );
        }
        assert!(hold(&s, &v, 116, false).is_some());
        assert_eq!(release(&s, &v, 1137), Some(61));
        assert!(hold(&s, &v, 7620, true).is_some());
    }
    #[test]
    fn actual_auto_repeat_uses_weapon_visual_and_distinct_hold_when_data_available() {
        let path = benilla_formats::wow_data_or_skip!();
        let mut chain = benilla_formats::Chain::open(std::path::Path::new(&path)).unwrap();
        let s = benilla_formats::load_spell_catalog(&mut chain).unwrap();
        let v = benilla_formats::load_spell_visual_catalog(&mut chain).unwrap();
        let d = benilla_formats::load_item_display_catalog(&mut chain).unwrap();
        assert!(s.get(75).unwrap().auto_repeat());
        assert!(s.get(5019).unwrap().auto_repeat());
        assert_eq!(ranged_ready(&s, 75, Some(2), Some(2)), Some(109));
        assert_eq!(ranged_ready(&s, 75, Some(2), Some(3)), Some(110));
        assert_eq!(ranged_ready(&s, 5019, Some(2), Some(19)), Some(111));
        assert!(ranged_ready(&s, 116, Some(2), Some(19)).is_none());
        assert!(ranged_ready(&s, 5019, None, None).is_none());
        for expected in [46, 49] {
            let display = (1..70_000)
                .find(|id| {
                    d.get(*id)
                        .and_then(|row| v.stages(row.spell_visual))
                        .and_then(|st| kit(&v, st.cast))
                        == Some(expected)
                })
                .expect("actual weapon display visual exists");
            assert_eq!(ranged_release(&s, &v, &d, 75, display), Some(expected));
            println!("actual auto shot display{display} release={expected}");
        }
        let anim = ranged_release(&s, &v, &d, 5019, 35263);
        println!(
            "actual Ada wand display35263 visual={} release={anim:?}",
            d.get(35263).unwrap().spell_visual
        );
        assert!(anim.is_some());
        assert_ne!(anim, Some(49), "wand must not be guessed as a rifle");
        assert!(ranged_release(&s, &v, &d, 5019, 0).is_none());
    }
}
