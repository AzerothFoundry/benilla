//! Local, bounded emote voice playback. Read the original race/sex voice mapping, not TTS.
use benilla_app::companion_renderer::CompanionLocalGesture;
use benilla_formats::{Chain, EmoteSoundCatalog, SoundKitCatalog};
use bevy::prelude::*;
use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
};
#[derive(Resource, Default)]
struct Audio {
    data: Option<(Chain, EmoteSoundCatalog, SoundKitCatalog)>,
    seen: u64,
    child: Option<Child>,
}
impl Audio {
    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        self.stop();
    }
}
pub fn install(app: &mut App) {
    app.init_resource::<Audio>()
        .add_systems(Startup, load)
        .add_systems(Update, update);
}
fn load(mut audio: ResMut<Audio>, ui: Res<crate::UiState>) {
    if let Ok(mut chain) = Chain::open(std::path::Path::new(&ui.active_data_dir)) {
        if let (Ok(emotes), Ok(kits)) = (
            benilla_formats::load_emote_sound_catalog(&mut chain),
            benilla_formats::load_sound_kit_catalog(&mut chain),
        ) {
            audio.data = Some((chain, emotes, kits));
        }
    }
}
// This first lane covers EmotesTextSound voices; M2 $CSD key-timed sounds remain separate.
fn command_name(animation: u16) -> Option<&'static str> {
    Some(match animation {
        60 => "WELCOME",
        64 => "CONGRATULATE",
        65 => "QUESTION",
        66 => "BOW",
        67 => "HELLO",
        68 => "CHEER",
        69 => "DANCE",
        70 => "LAUGH",
        73 => "RASP",
        74 => "ROAR",
        75 => "KNEEL",
        76 => "KISS",
        77 => "CRY",
        78 => "CHICKEN",
        79 => "BEG",
        80 => "APPLAUD",
        81 => "SHOUT",
        82 => "FLEX",
        83 => "FLIRT",
        84 => "POINT",
        113 => "SALUTE",
        _ => return None,
    })
}
fn update(
    mut audio: ResMut<Audio>,
    gesture: Res<CompanionLocalGesture>,
    interaction: Res<crate::Interaction>,
    ui: Res<crate::UiState>,
    frame: Res<crate::CompanionFrame>,
) {
    if !ui.settings.play_emote_sounds || !interaction.dancing || !gesture.active {
        audio.stop();
        return;
    }
    let Some(animation) = gesture.selected else {
        return;
    };
    if audio.seen == gesture.request {
        if audio
            .child
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_some())
        {
            audio.child = None;
        }
        return;
    }
    audio.seen = gesture.request;
    audio.stop();
    let Some((chain, emotes, kits)) = &audio.data else {
        return;
    };
    let race = frame.0["appearance"]["race"].as_u64().unwrap_or(0) as u32;
    let sex = frame.0["appearance"]["sex"].as_u64().unwrap_or(0) as u32;
    let Some(kit) = command_name(animation)
        .and_then(|name| emotes.text_id(name))
        .and_then(|id| emotes.voice(id, race, sex))
        .and_then(|id| kits.get(id))
    else {
        return;
    };
    let available: Vec<_> = kit
        .files
        .iter()
        .filter(|(name, _)| chain.contains(name))
        .collect();
    if available.is_empty() {
        return;
    }
    let index =
        benilla_app::companion_renderer::local_gesture_index(gesture.request, available.len());
    let path = &available[index].0;
    let Ok(bytes) = chain.read(path) else { return };
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("wav")
        .to_ascii_lowercase();
    if !matches!(ext.as_str(), "wav" | "mp3" | "ogg") {
        return;
    }
    let cache = ui
        .path
        .parent()
        .unwrap_or(std::path::Path::new("/tmp"))
        .join("emote-audio");
    if std::fs::create_dir_all(&cache).is_err() {
        return;
    }
    let output: PathBuf = cache.join(format!("{}-{}.{}", std::process::id(), kit.id, ext));
    if std::fs::write(&output, bytes).is_err() {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        audio.child = Command::new("/usr/bin/afplay")
            .arg("-v")
            .arg("0.6")
            .arg(&output)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok();
        if audio.child.is_some() {
            info!(
                "companion emote voice: animation={animation} race={race} sex={sex} file={}",
                output.display()
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_gestures_have_semantic_voice_commands() {
        assert_eq!(command_name(67), Some("HELLO"));
        assert_eq!(command_name(70), Some("LAUGH"));
        assert_eq!(command_name(17), None);
    }
}

#[cfg(test)]
mod data_tests {
    use super::*;
    #[test]
    fn live_troll_female_voice_uses_existing_game_resources() {
        let path = benilla_formats::wow_data_or_skip!();
        let mut chain = Chain::open(std::path::Path::new(&path)).unwrap();
        let emotes = benilla_formats::load_emote_sound_catalog(&mut chain).unwrap();
        let kits = benilla_formats::load_sound_kit_catalog(&mut chain).unwrap();
        let kit = emotes
            .voice(emotes.text_id(command_name(67).unwrap()).unwrap(), 8, 1)
            .and_then(|k| kits.get(k))
            .unwrap();
        assert!(kit.files.iter().any(|(p, _)| chain.contains(p)));
        for (path, _) in &kit.files {
            if chain.contains(path) {
                let bytes = chain.read(path).unwrap();
                assert!(!bytes.is_empty());
            }
        }
    }
}
