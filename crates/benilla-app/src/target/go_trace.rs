//! Opt-in diagnostics contain only clicked GO fields and our own spell state.
use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

#[cfg(any(not(target_arch = "wasm32"), test))]
fn requested(value: Option<&str>) -> bool {
    value == Some("1")
}
fn relevant_spell(spell: u32) -> bool {
    matches!(spell, 21651 | 23932)
}
static CLICKED: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
pub(crate) fn watch_object(guid: u64) {
    if enabled() {
        *CLICKED.lock().unwrap_or_else(|e| e.into_inner()) = guid;
    }
}
fn caster_matches(caster: u64, own: Option<u64>, clicked: u64) -> bool {
    caster != 0 && (Some(caster) == own || (clicked != 0 && caster == clicked))
}
pub(crate) fn capture_event(spell: u32, caster: u64, own: Option<u64>) -> bool {
    enabled()
        && relevant_spell(spell)
        && caster_matches(
            caster,
            own,
            *CLICKED.lock().unwrap_or_else(|e| e.into_inner()),
        )
}
pub(crate) fn enabled() -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static ON: std::sync::LazyLock<bool> =
            std::sync::LazyLock::new(|| requested(std::env::var("WOW_GO_TRACE").ok().as_deref()));
        *ON
    }
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
}
pub(crate) fn line(args: fmt::Arguments<'_>) {
    if enabled() {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        bevy::log::info!(
            "GO_TRACE seq={} {}",
            NEXT.fetch_add(1, Ordering::Relaxed),
            args
        );
    }
}
pub(crate) fn spell_line(spell: u32, args: fmt::Arguments<'_>) {
    if relevant_spell(spell) {
        line(args);
    }
}

pub(super) fn snapshot(
    inputs: &super::lock::GoLockInputs,
    openers: &[u32],
    known: bool,
    go: Option<(&crate::net::ObjectStore, u32)>,
    me: Option<&crate::net::ObjectStore>,
    guid: u64,
) {
    if !enabled() {
        return;
    }
    if let Some(me) = me {
        line(format_args!(
            "actor ownUnitFlags={:#x} mounted={} form={} deadOrGhost={}",
            me.0.unit_flags(),
            me.0.unit_mount_display_id(),
            me.0.unit_shapeshift_form(),
            me.0.is_dead_or_ghost()
        ));
    }
    watch_object(guid);
    let template = go.and_then(|(store, _)| inputs.templates.of(&store.0));
    let facts = super::lock::go_facts(go);
    line(format_args!("resolve guid={guid:#x} entry={:?} type={} wireState={:?} resolvedState={} flags={:#x} level={} lock={:?} known21651={} opener21651={} catalog21651={}",
        go.and_then(|(s,_)|s.0.object_entry()),go.map_or(-1,|(s,_)|s.0.gameobject_type_id()),go.and_then(|(s,_)|s.0.gameobject_state()),facts.state,
        go.map_or(0,|(s,_)|s.0.gameobject_flags()),facts.level,template.map(|t|t.lock_id),known,openers.contains(&21651),inputs.spells.as_deref().is_some_and(|s|s.catalog.get(21651).is_some())));
    if let Some(slots) =
        template.and_then(|t| inputs.locks.as_deref().and_then(|l| l.0.slots(t.lock_id)))
    {
        for (i, slot) in slots.iter().enumerate().filter(|(_, s)| s.key_type != 0) {
            line(format_args!("lockSlot guid={guid:#x} slot={i} keyType={} index={} skill={} action={} available={}",slot.key_type,slot.index,slot.skill,slot.action,slot.available(facts.state,facts.flag_locked)));
        }
        let mut matched = None;
        let outcome = super::lock::resolve_lock(
            slots,
            openers,
            inputs.spells.as_deref(),
            inputs.skill_lines.as_ref().map(|s| &s.catalog),
            me,
            &inputs.objects,
            facts,
            &mut matched,
        );
        line(format_args!(
            "lockMatch guid={guid:#x} outcome={outcome:?} matched={matched:?}"
        ));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_explicit_trace_value_and_capture_spell_are_enabled() {
        assert!(super::requested(Some("1")));
        for value in [None, Some("0"), Some("true"), Some("")] {
            assert!(!super::requested(value));
        }
        assert!(super::relevant_spell(21651));
        assert!(super::relevant_spell(23932));
        assert!(super::caster_matches(7, Some(7), 99));
        assert!(super::caster_matches(99, Some(7), 99));
        assert!(!super::caster_matches(88, Some(7), 99));
        assert!(!super::caster_matches(0, None, 0));
        for spell in [0, 133, 6247] {
            assert!(!super::relevant_spell(spell));
        }
    }
}
