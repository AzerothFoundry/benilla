//! Start a visual edge exactly once while keeping the locomotion clock untouched.
use bevy::animation::graph::AnimationNodeIndex;
use bevy::animation::{ActiveAnimation, AnimationPlayer, RepeatAnimation};
pub(super) fn upper(
    player: &mut AnimationPlayer,
    node: AnimationNodeIndex,
    new_edge: bool,
    looping: bool,
) -> &mut ActiveAnimation {
    let active = if new_edge {
        player.start(node)
    } else {
        player.play(node)
    };
    active.set_repeat(if looping {
        RepeatAnimation::Forever
    } else {
        RepeatAnimation::Count(1)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_edge_rewinds_finished_upper_without_rewinding_run() {
        let (run, wave) = (AnimationNodeIndex::new(0), AnimationNodeIndex::new(1));
        let mut p = AnimationPlayer::default();
        p.play(run).repeat().set_seek_time(0.73);
        p.play(wave)
            .set_seek_time(1.9)
            .set_repeat(RepeatAnimation::Count(0));
        assert!(p.animation(wave).unwrap().is_finished());
        upper(&mut p, wave, true, false);
        assert_eq!(p.animation(wave).unwrap().seek_time(), 0.);
        assert!(!p.animation(wave).unwrap().is_finished());
        assert_eq!(p.animation(run).unwrap().seek_time(), 0.73);
        p.animation_mut(wave).unwrap().set_seek_time(0.4);
        upper(&mut p, wave, false, false);
        assert_eq!(
            p.animation(wave).unwrap().seek_time(),
            0.4,
            "same sequence must not replay"
        );
        upper(&mut p, wave, true, false);
        assert_eq!(p.animation(wave).unwrap().seek_time(), 0.);
        assert_eq!(p.animation(run).unwrap().seek_time(), 0.73);
    }
}
