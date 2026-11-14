//! Native IME text joins the existing character channel in window-event order.
//! The original IME writer is documented at 0x77ac60; platform composition itself
//! belongs to winit. Preedit is never stored as committed EditBox text.
use benilla_ui::script::UiScript;
use benilla_ui::widget::FrameHandle;
use bevy::input::{keyboard::KeyboardInput, ButtonState};
use bevy::prelude::*;
use bevy::window::{Ime, PrimaryWindow};

#[derive(Default)]
pub(super) struct Composition {
    owner: Option<(u64, FrameHandle)>,
    composing: bool,
    confirmed: bool,
}
fn focus(script: &UiScript) -> Option<(u64, FrameHandle)> {
    script
        .focused_editbox_bounds()
        .map(|(handle, _)| (script.session(), handle))
}
impl Composition {
    pub(super) fn begin_frame(&mut self) {
        self.confirmed = false;
    }
    pub(super) fn dispatch(&mut self, script: &mut UiScript, event: &Ime) {
        match event {
            Ime::Preedit { value, .. } => {
                if !self.composing {
                    self.owner = focus(script);
                }
                if self.composing && value.is_empty() {
                    self.confirmed = true;
                }
                self.composing = !value.is_empty();
            }
            Ime::Commit { value, .. } => {
                let current = focus(script);
                if current.is_some() && (self.owner.is_none() || self.owner == current) {
                    for ch in value.chars() {
                        if focus(script) != current {
                            break;
                        }
                        let mut bytes = [0; 4];
                        script.char_input(ch.encode_utf8(&mut bytes));
                    }
                }
                self.owner = None;
                self.composing = false;
                self.confirmed = true;
            }
            Ime::Disabled { .. } => {
                self.owner = None;
                self.composing = false;
            }
            Ime::Enabled { .. } => {}
        }
    }
    pub(super) fn consumes(&mut self, script: &UiScript, key: &KeyboardInput) -> bool {
        if self.owner.is_some() && self.owner != focus(script) {
            self.composing = false;
        }
        key.state == ButtonState::Pressed
            && script.has_keyboard_focus()
            && (self.composing
                || (self.confirmed
                    && matches!(key.key_code, KeyCode::Enter | KeyCode::NumpadEnter)))
    }
}

pub(super) fn sync_window(
    script: Option<NonSend<UiScript>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    egui: Option<Res<super::EguiKeyboard>>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    if egui.is_some_and(|value| value.0) {
        return;
    }
    window.ime_enabled = window.focused
        && script
            .as_ref()
            .is_some_and(|script| script.has_keyboard_focus());
    if let Some(script) = script {
        if let Some((_, rect)) = script.focused_editbox_bounds() {
            let wide = script.cvar("widescreen").is_none_or(|value| value != "0");
            let screen = super::UiScreen::of(Vec2::new(window.width(), window.height()), wide);
            let at = super::ui_to_window(screen, Vec2::new(rect.left, rect.bottom));
            window.ime_position = at.clamp(Vec2::ZERO, Vec2::new(window.width(), window.height()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::input::keyboard::Key;

    fn editor() -> UiScript {
        let mut script = UiScript::new().unwrap();
        script.set_screen_size(1024.0, 768.0);
        script
            .run(
                r#"
            for _, name in ipairs({"ImeA", "ImeB"}) do
                local e=CreateFrame("EditBox",name)
                e:SetWidth(200);e:SetHeight(30);e:SetPoint("BOTTOMLEFT",nil,"BOTTOMLEFT",20,30)
                e:SetAutoFocus(false);e:Show()
                e:SetScript("OnEnterPressed",function() ImeSends=(ImeSends or 0)+1 end)
            end
            ImeA:SetFocus()
        "#,
            )
            .unwrap();
        script.resolve();
        script
    }
    fn preedit(value: &str) -> Ime {
        Ime::Preedit {
            window: Entity::PLACEHOLDER,
            value: value.into(),
            cursor: Some((0, value.len())),
        }
    }
    fn commit(value: &str) -> Ime {
        Ime::Commit {
            window: Entity::PLACEHOLDER,
            value: value.into(),
        }
    }
    fn enter() -> KeyboardInput {
        KeyboardInput {
            key_code: KeyCode::Enter,
            logical_key: Key::Enter,
            state: ButtonState::Pressed,
            text: Some("\r".into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }
    #[test]
    fn commits_chinese_without_inserting_preedit_or_sending_chat() {
        let mut script = editor();
        let mut ime = Composition::default();
        ime.dispatch(&mut script, &preedit("zhongwen"));
        assert_eq!(script.eval::<String>("return ImeA:GetText()").unwrap(), "");
        assert!(ime.consumes(&script, &enter()));
        ime.dispatch(&mut script, &preedit(""));
        ime.dispatch(&mut script, &commit("中文"));
        assert_eq!(
            script.eval::<String>("return ImeA:GetText()").unwrap(),
            "中文"
        );
        assert!(ime.consumes(&script, &enter()));
        assert_eq!(script.eval::<Option<u32>>("return ImeSends").unwrap(), None);
        ime.begin_frame();
        assert!(!ime.consumes(&script, &enter()));
        script.key_down("ENTER");
        assert_eq!(script.eval::<u32>("return ImeSends").unwrap(), 1);
    }
    #[test]
    fn focus_change_drops_a_pending_commit_instead_of_typing_into_another_box() {
        let mut script = editor();
        let mut ime = Composition::default();
        ime.dispatch(&mut script, &preedit("ni"));
        script.run("ImeB:SetFocus()").unwrap();
        ime.dispatch(&mut script, &commit("你"));
        assert_eq!(
            script
                .eval::<String>("return ImeA:GetText() .. ImeB:GetText()")
                .unwrap(),
            ""
        );
        ime.dispatch(&mut script, &preedit("hao"));
        ime.dispatch(&mut script, &commit("好"));
        assert_eq!(
            script.eval::<String>("return ImeB:GetText()").unwrap(),
            "好"
        );
    }
    #[test]
    fn commit_replaces_selection_and_stops_at_the_editbox_character_limit() {
        let mut script = editor();
        let mut ime = Composition::default();
        script
            .run("ImeA:SetText('old'); ImeA:HighlightText(0,-1); ImeA:SetMaxLetters(2)")
            .unwrap();
        ime.dispatch(&mut script, &preedit("renwu"));
        ime.dispatch(&mut script, &commit("任务点"));
        assert_eq!(
            script.eval::<String>("return ImeA:GetText()").unwrap(),
            "任务"
        );
        script.run("ImeA:ClearFocus()").unwrap();
        ime.dispatch(&mut script, &commit("不应输入"));
        assert_eq!(
            script.eval::<String>("return ImeA:GetText()").unwrap(),
            "任务"
        );
    }
    #[test]
    fn native_ime_is_enabled_only_for_a_focused_editbox_in_an_active_window() {
        let mut world = World::new();
        world.insert_non_send_resource(editor());
        let window = world.spawn((Window::default(), PrimaryWindow)).id();
        world.run_system_once(sync_window).unwrap();
        let native = world.get::<Window>(window).unwrap();
        assert!(native.ime_enabled);
        assert!(native.ime_position.is_finite());
        world.get_mut::<Window>(window).unwrap().focused = false;
        world.run_system_once(sync_window).unwrap();
        assert!(!world.get::<Window>(window).unwrap().ime_enabled);
        world.get_mut::<Window>(window).unwrap().focused = true;
        world
            .non_send_resource::<UiScript>()
            .run("ImeA:ClearFocus()")
            .unwrap();
        world.run_system_once(sync_window).unwrap();
        assert!(!world.get::<Window>(window).unwrap().ime_enabled);
    }
}
