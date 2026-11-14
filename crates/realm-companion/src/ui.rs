use crate::{
    presentation, settings::normalize_status_url, CompanionFrame, Interaction, Poller, SkinState,
    UiState,
};
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use std::sync::atomic::Ordering;
fn button(ui: &mut egui::Ui, skin: &Option<crate::skin::WowSkin>, label: &str) -> egui::Response {
    if let Some(skin) = skin {
        skin.button(ui, label)
    } else {
        ui.button(label)
    }
}
enum Action {
    Enter(String),
    Manage,
    Selector,
    Save,
    Add,
    Remove,
    Close,
    Exit,
    Restart,
}
pub fn settings_ui(
    mut contexts: EguiContexts,
    mut state: ResMut<UiState>,
    skin: Res<SkinState>,
    poller: Res<Poller>,
    mut frame: ResMut<CompanionFrame>,
    mut reveal: ResMut<presentation::Reveal>,
    mut interaction: ResMut<Interaction>,
    mut exit: MessageWriter<AppExit>,
    mut resolver: ResMut<crate::names::NameResolver>,
) -> Result {
    if !state.open {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let mut action = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(24))
        .show(ctx, |ui| {
            if let Some(s) = &skin.value {
                s.paint_frame(ui, ui.max_rect().expand(24.0));
                if state.chooser{
                    let key=ui.id().with(("selector-race",state.draft.selected_connection_id.clone()));
                    let race=if let Some(cached)=ui.data_mut(|d|d.get_temp::<Option<u8>>(key)){cached}else{
                        let value=state.draft.active_connection().and_then(|c|crate::settings::Settings::load_cached_appearance_for(&state.path,&crate::settings::cache_identity(&c.id,&c.status_url)).ok().flatten()).and_then(|cache|cache.appearance["race"].as_u64().map(|v|v as u8));
                        ui.data_mut(|d|d.insert_temp(key,value));value
                    };
                    s.paint_selector(ui,ui.max_rect().expand(24.),race);
                }
            }
            if state.chooser {
                ui.horizontal(|ui| {
                ui.add_space((ui.available_width()-360.).max(0.));
                ui.allocate_ui_with_layout(egui::vec2(360.,ui.available_height()),egui::Layout::top_down(egui::Align::Min),|ui| {
                ui.set_max_width(360.);
                ui.heading(
                    egui::RichText::new("选择你的角色")
                        .color(crate::skin::GOLD)
                        .size(26.0),
                );
                ui.label("双击角色，或选择后点击“进入角色”");
                ui.add_space(16.0);
                for c in &state.draft.connections {
                    resolver.request(&c.id, &c.status_url);
                }
                let selected = state.draft.selected_connection_id.clone();
                let selection_scroll=egui::ScrollArea::vertical()
                    .max_width(360.0)
                    .min_scrolled_height(416.0)
                    .max_height(416.0)
                    .auto_shrink([false,false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y=4.;
                        let rows = state.draft.connections.clone();
                        for c in rows {
                            let name = state.cached_names.get(&c.id).unwrap_or(&c.name);
                            let response = if let Some(skin) = &skin.value {
                                skin.character_row(
                                    ui,
                                    name,
                                    &c.name,
                                    selected.as_ref() == Some(&c.id),
                                )
                            } else {
                                ui.add_sized(
                                    [ui.available_width(), 48.0],
                                    egui::Button::new(name)
                                        .selected(selected.as_ref() == Some(&c.id)),
                                )
                            };
                            if response.clicked() {
                                let _ = state.draft.select_connection(&c.id);
                            }
                            if response.double_clicked() {
                                action = Some(Action::Enter(c.id.clone()));
                            }
                            response.on_hover_text(&c.status_url);
                        }
                        if state.draft.connections.is_empty() {
                            ui.label("尚未添加角色连接，请先管理连接。");
                        }
                    });
                if let Some(skin)=&skin.value{skin.scroll_art(ui,selection_scroll.inner_rect,selection_scroll.content_size,selection_scroll.state.offset);}
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if let Some(id) = state.draft.selected_connection_id.clone() {
                        if button(ui, &skin.value, "进入角色").clicked() {
                            action = Some(Action::Enter(id));
                        }
                    }
                    if button(ui, &skin.value, "管理连接").clicked() {
                        action = Some(Action::Manage);
                    }
                    if button(ui, &skin.value, "退出").clicked() {
                        action = Some(Action::Exit);
                    }
                });
                });
                });
            } else {
                let settings_scroll=egui::ScrollArea::vertical().id_salt("settings-management-scroll").show(ui,|ui|{
                ui.heading(
                    egui::RichText::new("桌宠设置")
                        .color(crate::skin::GOLD)
                        .size(25.0),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let label = state
                        .draft
                        .active_connection()
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| "请选择连接".into());
                    egui::ComboBox::from_id_salt("connections")
                        .width(260.0)
                        .selected_text(label)
                        .show_ui(ui, |ui| {
                            for c in state.draft.connections.clone() {
                                if ui
                                    .selectable_label(
                                        state.draft.selected_connection_id.as_ref() == Some(&c.id),
                                        &c.name,
                                    )
                                    .clicked()
                                {
                                    let _ = state.draft.select_connection(&c.id);
                                }
                            }
                        });
                    if button(ui, &skin.value, "添加").clicked() {
                        action = Some(Action::Add);
                    }
                    if button(ui, &skin.value, "删除").clicked() {
                        action = Some(Action::Remove);
                    }
                });
                if let Some(id) = state.draft.selected_connection_id.clone() {
                    if let Some(c) = state.draft.connections.iter_mut().find(|c| c.id == id) {
                        ui.label(format!(
                            "角色：{}",
                            if c.name.is_empty() {
                                "获取中…"
                            } else {
                                &c.name
                            }
                        ));
                        ui.label("完整 status URL");
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut c.status_url)
                                .desired_width(f32::INFINITY),
                        );
                        if response.changed() {
                            c.name = crate::settings::provisional_name(&c.status_url);
                        }
                        if response.lost_focus() {
                            resolver.request(&c.id, &c.status_url);
                        }
                    }
                }
                ui.small("事件接口自动发现；不可用时回退到 status 轮询。");
                ui.add_space(6.0);
                ui.checkbox(&mut state.draft.power_saving_idle,"自定义省电待机");
                ui.horizontal(|ui| {
                    ui.label("待机帧率");
                    ui.add_enabled(state.draft.power_saving_idle, egui::Slider::new(&mut state.draft.idle_fps, 2..=120).suffix(" FPS"));
                });
                ui.horizontal(|ui| {
                    ui.label("活动帧率");
                    ui.add(egui::Slider::new(&mut state.draft.active_fps, 2..=120).suffix(" FPS"));
                });
                ui.small("修改立即预览，保存后保留。建议待机10–15、活动30–60；越高越耗电。");
                ui.small("鼠标靠近、动作或特效使用活动档。关闭自定义待机恢复20帧，睡眠10帧。");
                ui.small("这是刷新目标，并非严格上限；受垂直同步、硬件性能和窗口事件影响。");
                ui.checkbox(&mut state.draft.show_speech_bubble,"显示表情气泡");
                ui.checkbox(&mut state.draft.play_emote_sounds,"播放表情声音（游戏原声）");
                ui.small("部分动作没有原声；对白是桌宠本地短句，不是语音字幕。");
                ui.label(egui::RichText::new("外观显示").color(crate::skin::GOLD));
                egui::Grid::new("visible-equipment-slots").num_columns(4).spacing([12.,3.]).show(ui,|ui|{
                    for(index,(slot,label))in crate::settings::VISIBLE_EQUIPMENT_SLOTS.iter().enumerate(){
                        let source=match *slot{0=>frame.0["appearance"]["hide_helm"].as_bool()!=Some(true),14=>frame.0["appearance"]["hide_cloak"].as_bool()!=Some(true),_=>true};
                        let override_value=if *slot==0{state.draft.show_helm}else{state.draft.equipment_visibility.get(slot).copied()};let mut visible=override_value.unwrap_or(source);
                        let r=if let Some(skin)=&skin.value{skin.checkbox(ui,&mut visible,label)}else{ui.checkbox(&mut visible,*label)};
                        if r.changed(){if *slot==0{state.draft.show_helm=Some(visible);}else{state.draft.equipment_visibility.insert(*slot,visible);}}
                        r.on_hover_text("只改变桌宠外观，不修改游戏装备。没有装备时不会凭空生成模型。");
                        if(index+1)%4==0{ui.end_row();}
                    }
                });
                ui.horizontal(|ui|{
                    if button(ui,&skin.value,"全部显示").clicked(){state.draft.set_equipment_visibility(Some(true));}
                    if button(ui,&skin.value,"全部隐藏").clicked(){state.draft.set_equipment_visibility(Some(false));}
                    if button(ui,&skin.value,"遵循游戏").clicked(){state.draft.set_equipment_visibility(None);}
                });

                ui.add_space(8.0);
                ui.label("WoW Data 资源目录");
                ui.add(
                    egui::TextEdit::singleline(&mut state.draft.data_dir)
                        .desired_width(f32::INFINITY),
                );
                ui.small("更换资源目录后保存并重启。界面素材直接读取游戏资源。");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label("显示比例");
                    if let Some(skin)=&skin.value{skin.scale_slider(ui,&mut state.draft.scale_percent);}else{ui.add(egui::Slider::new(&mut state.draft.scale_percent,25.0..=400.0).show_value(false));}
                    let id=ui.id().with("scale-percent-input");
                    let focused=ui.ctx().memory(|m|m.has_focus(id));
                    let mut value=if focused{ui.data_mut(|d|d.get_temp::<String>(id)).unwrap_or_else(||format!("{:.0}",state.draft.scale_percent))}else{format!("{:.0}",state.draft.scale_percent)};
                    let response=ui.add(egui::TextEdit::singleline(&mut value).id(id).desired_width(50.));
                    if response.changed(){if let Ok(v)=value.parse::<f32>(){if v.is_finite(){state.draft.scale_percent=v.clamp(25.,400.);}}}
                    ui.data_mut(|d|d.insert_temp(id,value));
                    ui.label("%");
                });
                if button(ui, &skin.value, "恢复 100%（角色面板基准）").clicked() {
                    state.draft.scale_percent = 100.0;
                }
                ui.horizontal(|ui| {
                    if let Some(skin) = &skin.value {
                        skin.checkbox(ui, &mut state.draft.always_on_top, "始终置顶");
                    } else {
                        ui.checkbox(&mut state.draft.always_on_top, "始终置顶");
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("视觉效果");
                    let spell = if let Some(skin) = &skin.value {
                        skin.checkbox(ui, &mut state.draft.show_spell_effects, "法术")
                    } else {
                        ui.checkbox(&mut state.draft.show_spell_effects, "法术")
                    };
                    spell.on_hover_text(
                        "法术视觉效果；不影响角色动作、坐下、吃喝以及杯子和食物模型。",
                    );
                    let enchant = if let Some(skin) = &skin.value {
                        skin.checkbox(ui, &mut state.draft.show_enchant_effects, "附魔")
                    } else {
                        ui.checkbox(&mut state.draft.show_enchant_effects, "附魔")
                    };
                    enchant.on_hover_text("目前仅保存偏好，待附魔特效适配后生效。");
                    let equipment = if let Some(skin) = &skin.value {
                        skin.checkbox(ui, &mut state.draft.show_equipment_effects, "装备")
                    } else {
                        ui.checkbox(&mut state.draft.show_equipment_effects, "装备")
                    };
                    equipment.on_hover_text("装备自带的发光、粒子等效果，不隐藏装备外观。");
                });
                #[cfg(target_os = "macos")]
                ui.horizontal(|ui| {
                    ui.label("显示范围");
                    ui.radio_value(&mut state.draft.all_desktops, true, "所有桌面");
                    ui.radio_value(&mut state.draft.all_desktops, false, "仅启动桌面");
                });
                ui.add_space(10.0);
                ui.label(&state.connection);
                ui.horizontal_wrapped(|ui| {
                    if button(ui, &skin.value, "保存并应用").clicked() {
                        action = Some(Action::Save);
                    }
                    if button(ui, &skin.value, "角色选择").clicked() {
                        action = Some(Action::Selector);
                    }
                    if state.settings.data_dir != state.active_data_dir
                        && button(ui, &skin.value, "重新启动").clicked()
                    {
                        action = Some(Action::Restart);
                    }
                    if button(ui, &skin.value, "关闭设置").clicked() {
                        action = Some(Action::Close);
                    }
                    if button(ui, &skin.value, "退出桌宠").clicked() {
                        action = Some(Action::Exit);
                    }
                });
                });
                if let Some(skin)=&skin.value{skin.scroll_art(ui,settings_scroll.inner_rect,settings_scroll.content_size,settings_scroll.state.offset);}
            }
            if !state.message.is_empty() {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(&state.message).color(crate::skin::GOLD));
            }
        });
    match action {
        Some(Action::Enter(id)) => {
            if let Err(e) = state.activate(&id, &poller, &mut frame, &mut reveal, &mut interaction)
            {
                state.message = e;
            }
        }
        Some(Action::Manage) => {
            state.chooser = false;
        }
        Some(Action::Selector) => {
            state.message.clear();
            state.open_settings(true);
        }
        Some(Action::Add) => {
            match state.draft.add_connection(
                String::new(),
                "http://127.0.0.1:20001/status?view=companion".into(),
            ) {
                Ok(_) => state.message = "只需填写地址，角色名字会自动读取。".into(),
                Err(e) => state.message = e,
            }
        }
        Some(Action::Remove) => {
            if let Some(id) = state.draft.selected_connection_id.clone() {
                state.draft.remove_connection(&id);
            }
        }
        Some(Action::Save) => {
            let result = (|| -> Result<(), String> {
                for c in &mut state.draft.connections {
                    c.status_url = normalize_status_url(&c.status_url)?;
                    if c.name.trim().is_empty() {
                        c.name = crate::settings::provisional_name(&c.status_url);
                    }
                }
                for c in &state.draft.connections {
                    resolver.request(&c.id, &c.status_url);
                }
                let selected = state.draft.selected_connection_id.clone();
                state.draft.status_url = state
                    .draft
                    .active_connection()
                    .map(|c| c.status_url.clone())
                    .unwrap_or_default();
                state.draft.position = state.settings.position;
                let merged = state.draft.save_merged(&state.settings, &state.path)?;
                let changed_resource = state.draft.data_dir != state.active_data_dir;
                state.settings = state.draft.clone();
                state.settings.connections = merged.connections;
                if let Some(id) =
                    selected.filter(|id| state.settings.connections.iter().any(|c| &c.id == id))
                {
                    if !changed_resource {
                        let next_url = state
                            .settings
                            .connections
                            .iter()
                            .find(|c| c.id == id)
                            .map(|c| c.status_url.clone())
                            .unwrap_or_default();
                        if state.active_connection_id.as_deref() != Some(id.as_str())
                            || state.active_url != next_url
                        {
                            state.activate(
                                &id,
                                &poller,
                                &mut frame,
                                &mut reveal,
                                &mut interaction,
                            )?;
                        } else {
                            state.open = false;
                            state.chooser = false;
                            state.draft = state.settings.clone();
                        }
                    }
                } else {
                    state.chooser = state.active_url.is_empty();
                }
                state.message = if changed_resource {
                    "已保存，请重新启动以加载资源目录。".into()
                } else {
                    String::new()
                };
                Ok(())
            })();
            if let Err(e) = result {
                state.message = e;
            }
        }
        Some(Action::Close) => {
            if state.active_url.is_empty() {
                exit.write(AppExit::Success);
            } else {
                state.open = false;
                state.chooser = false;
            }
        }
        Some(Action::Exit) => {
            exit.write(AppExit::Success);
        }
        Some(Action::Restart) => {
            state.restart_requested.store(true, Ordering::Relaxed);
            exit.write(AppExit::Success);
        }
        None => {}
    }
    Ok(())
}
