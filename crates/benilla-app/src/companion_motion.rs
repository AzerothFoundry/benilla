//! Render-only animation replay. Session/sequence deduplication is independent of HTTP/SSE.
use serde_json::Value;
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Motion {
    pub id: u16,
    pub looping: bool,
    pub sequence: u64,
    pub until: f64,
    pub spell_motion: bool,
    pub spell_id: Option<u64>,
}
#[derive(Default)]
pub(crate) struct Replay {
    session: String,
    sequence: u64,
    pub active: Option<Motion>,
    jump_until: f64,
}
impl Replay {
    pub fn jumping(&self, now: f64) -> bool {
        now < self.jump_until
    }
    pub fn select(&mut self, frame: &Value, local_now: f64, finished: bool) -> Option<Motion> {
        let session = frame["visual_session"].as_str().unwrap_or("");
        if self.session != session {
            self.session = session.into();
            self.sequence = 0;
            self.active = None;
            self.jump_until = 0.;
        }
        if self
            .active
            .is_some_and(|m| local_now >= m.until || (!m.looping && finished))
        {
            self.active = None;
        }
        let now = frame["now_ms"].as_u64().unwrap_or(0);
        for e in frame["visual_events"].as_array().into_iter().flatten() {
            let seq = e["sequence"].as_u64().unwrap_or(0);
            if seq <= self.sequence {
                continue;
            }
            self.sequence = seq;
            if matches!(e["kind"].as_str(), Some("spell_start" | "spell_cancel"))
                && self.active.is_some_and(|m| m.spell_motion)
            {
                let channel_launch = e["kind"] == "spell_start"
                    && frame["animation"]["channeling"] == true
                    && self.active.is_some_and(|m| {
                        m.spell_id.is_some() && m.spell_id == e["spell_id"].as_u64()
                    });
                if !channel_launch {
                    self.active = None;
                }
            }
            if e["kind"] != "animation" {
                continue;
            }
            let Some(id) = e["animation_id"].as_u64().filter(|id| *id < 208) else {
                continue;
            };
            let age = now.saturating_sub(e["at_ms"].as_u64().unwrap_or(0));
            let duration = e["duration_ms"].as_u64().unwrap_or(2500).clamp(100, 15_000);
            if age >= duration {
                continue;
            }
            if id == 37 {
                self.jump_until = local_now + 5.;
            } else if id == 39 {
                self.jump_until = 0.;
            }
            self.active = Some(Motion {
                id: id as u16,
                looping: e["looping"].as_bool().unwrap_or(false),
                sequence: seq,
                until: local_now + (duration - age) as f64 / 1000.,
                spell_motion: e["spell_motion"] == true,
                spell_id: e["spell_id"].as_u64(),
            });
        }
        if frame["connected"] == false
            || frame["animation"]["dead"] == true
            || frame["animation"]["sleeping"] == true
        {
            self.active = None;
            self.jump_until = 0.;
        }
        self.active
    }
}
pub(crate) fn new_edge(
    previous_session: &str,
    previous_sequence: u64,
    session: &str,
    sequence: u64,
) -> bool {
    sequence != 0 && (session != previous_session || sequence != previous_sequence)
}
pub(crate) fn upper_body_event(id: u16) -> bool {
    matches!(id,8..=10|16..=24|46|49|53..=59|60|64..=68|70|73..=74|76..=88|95|107|113|118|185..=186|195)
}
/// Base state names stay authored animation IDs. Rotation remains the paper-doll camera's.
pub(crate) fn base(a: &Value) -> Option<u16> {
    if a["dead"] == true || a["stand_state"] == 7 {
        return Some(1);
    }
    if a["stealthed"] == true {
        return Some(if a["moving"] == true { 119 } else { 120 });
    }
    if a["looting"] == true && a["moving"] != true {
        return Some(50);
    }
    match a["locomotion"].as_str() {
        Some("fly") => Some(144),
        Some("jump_start") => Some(37),
        Some("jump") => Some(38),
        Some("jump_end") => Some(39),
        Some("fall") => Some(40),
        Some("swim_idle") => Some(41),
        Some("swim") => Some(42),
        Some("swim_left") => Some(43),
        Some("swim_right") => Some(44),
        Some("swim_backward") => Some(45),
        Some("backward") => Some(13),
        Some("strafe_left") => Some(11),
        Some("strafe_right") => Some(12),
        Some("stealth_idle") => Some(120),
        Some("stealth_walk") => Some(119),
        _ => match a["stand_state"].as_u64() {
            Some(2) => Some(103),
            Some(4) => Some(102),
            Some(5) => Some(103),
            Some(6) => Some(104),
            Some(8) => Some(115),
            _ => match a["posture"].as_str() {
                Some("kneel") => Some(115),
                Some("sit_chair_low") => Some(102),
                Some("sit_chair_medium") => Some(103),
                Some("sit_chair_high") => Some(104),
                _ => None,
            },
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn deduplicates_reconnect_and_expires_one_shot() {
        let f = json!({"connected":true,"visual_session":"a","now_ms":100,"animation":{},"visual_events":[{"kind":"animation","animation_id":67,"sequence":1,"at_ms":100,"duration_ms":1000}]});
        let mut r = Replay::default();
        assert_eq!(r.select(&f, 0., false).unwrap().id, 67);
        assert!(r.select(&f, 0.2, true).is_none());
        assert!(r.select(&f, 0.3, false).is_none());
        let mut fresh = f.clone();
        fresh["visual_session"] = "b".into();
        assert!(r.select(&fresh, 0.4, false).is_some());
    }
    #[test]
    fn death_preempts_and_stale_events_do_not_replay() {
        let f = json!({"connected":true,"visual_session":"a","now_ms":5000,"animation":{},"visual_events":[{"kind":"animation","animation_id":67,"sequence":1,"at_ms":100,"duration_ms":1000}]});
        assert!(Replay::default().select(&f, 0., false).is_none());
        assert_eq!(base(&json!({"dead":true,"locomotion":"jump"})), Some(1));
        assert_eq!(base(&json!({"locomotion":"swim"})), Some(42));
    }
    #[test]
    fn state_postures_and_locomotion_map_to_real_ids() {
        for (state, id) in [(2, 103), (4, 102), (5, 103), (6, 104), (7, 1), (8, 115)] {
            assert_eq!(base(&json!({"stand_state":state})), Some(id));
        }
        assert_eq!(base(&json!({"stealthed":true,"moving":false})), Some(120));
        assert_eq!(base(&json!({"stealthed":true,"moving":true})), Some(119));
        assert_eq!(base(&json!({"looting":true})), Some(50));
        assert_eq!(
            base(&json!({"looting":true,"moving":true,"locomotion":"backward"})),
            Some(13)
        );
    }
    #[test]
    fn jump_land_lifecycle_is_bounded_and_repeated_id_gets_new_sequence() {
        let mut r = Replay::default();
        let mut f = json!({"connected":true,"visual_session":"a","now_ms":100,"animation":{},"visual_events":[{"kind":"animation","animation_id":37,"sequence":1,"at_ms":100}]});
        assert_eq!(r.select(&f, 0., false).unwrap().id, 37);
        assert!(r.jumping(1.));
        assert!(!r.jumping(6.));
        f["visual_events"][0]["animation_id"] = 39.into();
        f["visual_events"][0]["sequence"] = 2.into();
        assert_eq!(r.select(&f, 1., false).unwrap().id, 39);
        assert!(!r.jumping(1.));
        f["visual_events"][0]["sequence"] = 3.into();
        assert_eq!(r.select(&f, 1.1, false).unwrap().sequence, 3);
    }
    #[test]
    fn cancel_or_new_cast_cannot_reveal_a_previous_release_gesture() {
        let mut f = json!({"connected":true,"visual_session":"a","now_ms":100,"animation":{},"visual_events":[{"kind":"animation","animation_id":53,"sequence":1,"at_ms":100,"spell_motion":true}]});
        let mut r = Replay::default();
        assert!(r.select(&f, 0., false).is_some());
        f["visual_events"]
            .as_array_mut()
            .unwrap()
            .push(json!({"kind":"spell_cancel","sequence":2,"at_ms":101}));
        assert!(r.select(&f, 0.1, false).is_none());
        assert!(r.select(&f, 0.2, false).is_none());
    }

    #[test]
    fn channel_start_keeps_its_matching_cast_gesture() {
        let mut r = Replay::default();
        let f = json!({"connected":true,"visual_session":"a","now_ms":101,"animation":{"channeling":true},"visual_events":[{"kind":"animation","animation_id":133,"spell_id":7620,"spell_motion":true,"sequence":1,"at_ms":100},{"kind":"spell_start","spell_id":7620,"sequence":2,"at_ms":101}]});
        assert_eq!(r.select(&f, 0., false).unwrap().id, 133);
    }

    #[test]
    fn different_spell_start_clears_release_but_not_social() {
        for (spell_motion, expected) in [(true, false), (false, true)] {
            let mut r = Replay::default();
            let f = json!({"connected":true,"visual_session":"a","now_ms":101,"animation":{"channeling":true},"visual_events":[{"kind":"animation","animation_id":53,"spell_id":116,"spell_motion":spell_motion,"sequence":1,"at_ms":100},{"kind":"spell_start","spell_id":133,"sequence":2,"at_ms":101}]});
            assert_eq!(r.select(&f, 0., false).is_some(), expected);
        }
    }
    #[test]
    fn body_edge_key_includes_the_agent_session() {
        assert!(!new_edge("a", 1, "a", 1));
        assert!(new_edge("a", 1, "b", 1));
        assert!(new_edge("a", 1, "a", 2));
        assert!(!new_edge("a", 1, "b", 0));
    }
}
