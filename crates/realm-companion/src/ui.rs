use crate::{presentation, CompanionFrame, Interaction, Poller, SkinState, UiPage, UiState};
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
fn sized_button(
    ui: &mut egui::Ui,
    skin: &Option<crate::skin::WowSkin>,
    label: &str,
    width: f32,
) -> egui::Response {
    if let Some(skin) = skin {
        skin.button_width(ui, label, width)
    } else {
        ui.add_sized([width, 28.], egui::Button::new(label))
    }
}
fn checkbox(
    ui: &mut egui::Ui,
    skin: &Option<crate::skin::WowSkin>,
    value: &mut bool,
    label: &str,
) -> egui::Response {
    if let Some(skin) = skin {
        skin.checkbox(ui, value, label)
    } else {
        ui.checkbox(value, label)
    }
}
fn fps_control(
    ui: &mut egui::Ui,
    skin: &Option<crate::skin::WowSkin>,
    label: &str,
    value: &mut u32,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let mut f = *value as f32;
        let r = if let Some(skin) = skin {
            skin.value_slider(ui, &mut f, 2., 120.)
        } else {
            ui.add(egui::Slider::new(&mut f, 2.0..=120.0).show_value(false))
        };
        if r.changed() {
            *value = f.round() as u32;
        }
        let id = ui.id().with(label);
        let focused = ui.ctx().memory(|m| m.has_focus(id));
        let mut text = if focused {
            ui.data_mut(|d| d.get_temp::<String>(id))
                .unwrap_or_else(|| value.to_string())
        } else {
            value.to_string()
        };
        let r = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(id)
                .desired_width(50.),
        );
        if r.changed() {
            if let Ok(n) = text.parse::<u32>() {
                *value = n.clamp(2, 120);
            }
        }
        ui.data_mut(|d| d.insert_temp(id, text));
        ui.label("FPS");
    });
}
fn section(ui: &mut egui::Ui, label: &str) {
    ui.add_space(12.);
    ui.separator();
    ui.label(
        egui::RichText::new(label)
            .color(crate::skin::GOLD)
            .size(19.),
    );
}
enum Action {
    Enter(String),
    Manage,
    Save,
    SelectRole,
    Navigate(crate::SelectorNavigation),
    SaveRoles,
    CancelRoles,
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
    mut selector: Option<ResMut<benilla_app::companion_renderer::CompanionSelectorBackground>>,
) -> Result {
    if let Some(selector) = selector.as_deref_mut() {
        selector.visible = state.open && state.page == UiPage::Selector;
    }
    if !state.open {
        return Ok(());
    }
    let scene = selector
        .as_ref()
        .and_then(|s| s.image.clone())
        .map(|image| contexts.add_image(bevy_egui::EguiTextureHandle::Strong(image)));
    if let Some(selector) = selector.as_deref_mut() {
        selector.visible = state.open && state.page == UiPage::Selector;
    }
    resolver.set_data_dir(&state.active_data_dir);
    let ctx = contexts.ctx_mut()?;
    let mut action = None;
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(24))
        .show(ctx, |ui| {
            if let Some(s) = &skin.value {
                s.paint_frame(ui, ui.max_rect().expand(24.0));
                if state.page==UiPage::Selector{
                    let key=ui.id().with(("selector-race",state.draft.selected_connection_id.clone(),state.draft.status_url.clone()));
                    let race=if let Some(cached)=ui.data_mut(|d|d.get_temp::<Option<u8>>(key)){cached}else{
                        let value=state.draft.active_connection().and_then(|c|crate::settings::Settings::load_cached_appearance_for(&state.path,&crate::settings::cache_identity(&c.id,&c.status_url)).ok().flatten()).and_then(|cache|cache.appearance["race"].as_u64().map(|v|v as u8));
                        ui.data_mut(|d|d.insert_temp(key,value));value
                    };
                    let race=state.draft.active_connection().and_then(|c|resolver.profile(&state.path,&c.id,&c.status_url).race).or(race).filter(|race|benilla_app::companion_renderer::scene_token(*race).is_some());
                    if let Some(selector)=selector.as_deref_mut(){selector.race=race;}
                    s.paint_selector(ui,ui.max_rect().expand(24.),race,if selector.as_ref().is_some_and(|s|s.ready){scene}else{None});
                }
            }
            if state.page==UiPage::Selector {
                if !ctx.wants_keyboard_input() {
                    let direction=ctx.input(|i|if i.key_pressed(egui::Key::ArrowDown)||i.key_pressed(egui::Key::ArrowRight){1}else if i.key_pressed(egui::Key::ArrowUp)||i.key_pressed(egui::Key::ArrowLeft){-1}else{0});
                    if direction!=0 {let ids:Vec<_>=state.draft.connections.iter().map(|c|c.id.clone()).collect();if !ids.is_empty(){let current=ids.iter().position(|id|Some(id)==state.draft.selected_connection_id.as_ref()).unwrap_or(0);let next=(current as isize+direction).rem_euclid(ids.len() as isize) as usize;let _=state.draft.select_connection(&ids[next]);}}
                    if ctx.input(|i|i.key_pressed(egui::Key::Enter)){if let Some(id)=state.draft.selected_connection_id.clone(){action=Some(Action::Enter(id));}}
                }
                let layout=crate::skin::selector_layout(ui.max_rect().expand(24.));
                if ui.scope_builder(egui::UiBuilder::new().max_rect(layout.manage),|ui|sized_button(ui,&skin.value,"管理角色",160.)).inner.clicked(){action=Some(Action::Manage);}
                for c in &state.draft.connections{resolver.request(&c.id,&c.status_url);}
                let selected=state.draft.selected_connection_id.clone();
                ui.scope_builder(egui::UiBuilder::new().max_rect(layout.list),|ui|{
                    let scroll=egui::ScrollArea::vertical().id_salt("portrait-role-list").min_scrolled_height(544.).max_height(544.).auto_shrink([false,false]).show(ui,|ui|{
                        ui.spacing_mut().item_spacing.y=4.;
                        for c in state.draft.connections.clone(){
                            let name=state.cached_names.get(&c.id).unwrap_or(&c.name);
                            let profile=resolver.profile(&state.path,&c.id,&c.status_url).clone();
                            let r=if let Some(skin)=&skin.value{skin.character_row(ui,name,&profile.description(),profile.location.as_deref().unwrap_or("地区未知"),selected.as_ref()==Some(&c.id))}else{ui.add_sized([340.,64.],egui::Button::new(name).selected(selected.as_ref()==Some(&c.id)))};
                            if r.clicked(){let _=state.draft.select_connection(&c.id);}
                            if r.double_clicked(){action=Some(Action::Enter(c.id.clone()));}
                            r.on_hover_text(&c.status_url);
                        }
                        if state.draft.connections.is_empty(){ui.label("尚未添加角色，请点击“管理角色”。");}
                    });
                    if let Some(skin)=&skin.value{skin.scroll_art(ui,scroll.inner_rect,scroll.content_size,scroll.state.offset);}
                });
                let enter=ui.scope_builder(egui::UiBuilder::new().max_rect(layout.enter),|ui|ui.add_enabled_ui(selected.is_some(),|ui|sized_button(ui,&skin.value,"进入角色",160.)).inner).inner;
                if enter.clicked(){if let Some(id)=selected{action=Some(Action::Enter(id));}}
                if ui.scope_builder(egui::UiBuilder::new().max_rect(layout.exit),|ui|sized_button(ui,&skin.value,"退出",160.)).inner.on_hover_text("关闭应用").clicked(){action=Some(Action::Exit);}
            } else if state.page==UiPage::Management {
                ui.heading(egui::RichText::new("管理角色").color(crate::skin::GOLD).size(25.));
                ui.label("添加或修改角色连接；保存不会切换当前桌宠。");
                let scroll=egui::ScrollArea::vertical().id_salt("role-management-scroll").max_height((ui.available_height()-65.).max(120.)).auto_shrink([false,false]).show(ui,|ui|{

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
                    if button(ui, &skin.value, "移除角色").on_hover_text("只移除连接，不会删除游戏角色").clicked() {
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
                });
                if let Some(skin)=&skin.value{skin.scroll_art(ui,scroll.inner_rect,scroll.content_size,scroll.state.offset);}
                ui.separator();ui.horizontal(|ui|{
                    if button(ui,&skin.value,"保存并返回").clicked(){action=Some(Action::SaveRoles);}
                    if button(ui,&skin.value,"取消").clicked(){action=Some(Action::CancelRoles);}
                });
            } else {
                ui.heading(egui::RichText::new("设置").color(crate::skin::GOLD).size(25.));
                ui.add_space(10.);
                ui.horizontal(|ui|{
                    ui.label(format!("当前角色：{}",frame.0["character"].as_str().unwrap_or("未连接")));
                    if button(ui,&skin.value,"选择角色").clicked(){action=Some(Action::SelectRole);}
                });
                let settings_scroll=egui::ScrollArea::vertical().id_salt("settings-management-scroll").max_height((ui.available_height()-65.).max(120.)).auto_shrink([false,false]).show(ui,|ui|{
                section(ui,"显示与性能");
                checkbox(ui,&skin.value,&mut state.draft.power_saving_idle,"自定义省电待机");
                ui.add_enabled_ui(state.draft.power_saving_idle,|ui|fps_control(ui,&skin.value,"待机帧率",&mut state.draft.idle_fps));
                fps_control(ui,&skin.value,"活动帧率",&mut state.draft.active_fps);
                ui.small("修改立即预览，保存后保留。建议待机10–15、活动30–60；越高越耗电。");
                ui.small("鼠标靠近、动作或特效使用活动档。关闭自定义待机恢复20帧，睡眠10帧。");
                ui.small("这是刷新目标，并非严格上限；受垂直同步、硬件性能和窗口事件影响。");
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
                #[cfg(target_os = "macos")]
                ui.horizontal(|ui| {
                    ui.label("显示范围");
                    ui.radio_value(&mut state.draft.all_desktops, true, "所有桌面");
                    ui.radio_value(&mut state.draft.all_desktops, false, "仅启动桌面");
                });
                section(ui,"外观与特效");
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
                section(ui,"互动与声音");
                checkbox(ui,&skin.value,&mut state.draft.show_speech_bubble,"显示表情气泡");
                checkbox(ui,&skin.value,&mut state.draft.play_emote_sounds,"播放表情声音（游戏原声）");
                ui.small("部分动作没有原声；对白是桌宠本地短句，不是语音字幕。");
                section(ui,"资源与连接状态");
                ui.add_space(8.0);
                ui.label("WoW Data 资源目录");
                ui.add(
                    egui::TextEdit::singleline(&mut state.draft.data_dir)
                        .desired_width(f32::INFINITY),
                );
                ui.small("更换资源目录后保存并重启。界面素材直接读取游戏资源。");
                ui.add_space(10.0);
                ui.label(&state.connection);
                });
                if let Some(skin)=&skin.value{skin.scroll_art(ui,settings_scroll.inner_rect,settings_scroll.content_size,settings_scroll.state.offset);}
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if button(ui, &skin.value, "保存").clicked() {
                        action = Some(Action::Save);
                    }
                    if state.settings.data_dir != state.active_data_dir
                        && button(ui, &skin.value, "重新启动").clicked()
                    {
                        action = Some(Action::Restart);
                    }
                    if button(ui, &skin.value, "取消").clicked() {
                        action = Some(Action::Close);
                    }
                });            }
            if !state.message.is_empty() {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(&state.message).color(crate::skin::GOLD));
            }
        });
    if state.pending_select {
        let response = egui::Modal::new(egui::Id::new("unsaved-settings-role-navigation"))
            .frame(egui::Frame::NONE.inner_margin(20.))
            .show(ctx, |ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(460., 140.),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        if let Some(skin) = &skin.value {
                            skin.paint_frame(ui, ui.max_rect().expand(20.));
                        }
                        ui.heading(egui::RichText::new("有未保存的修改").color(crate::skin::GOLD));
                        ui.label("选择角色前，是否保存当前设置？");
                        ui.add_space(10.);
                        ui.horizontal(|ui| {
                            if button(ui, &skin.value, "保存并继续").clicked() {
                                action = Some(Action::Navigate(crate::SelectorNavigation::Save));
                            }
                            if button(ui, &skin.value, "放弃修改").clicked() {
                                action = Some(Action::Navigate(crate::SelectorNavigation::Discard));
                            }
                            if button(ui, &skin.value, "继续设置").clicked() {
                                action = Some(Action::Navigate(crate::SelectorNavigation::Stay));
                            }
                        });
                    },
                );
            });
        if response.should_close() && action.is_none() {
            action = Some(Action::Navigate(crate::SelectorNavigation::Stay));
        }
    }
    match action {
        Some(Action::SelectRole) => state.request_selector(),
        Some(Action::Navigate(choice)) => {
            if let Err(e) = state.finish_selector_request(choice) {
                state.message = e;
            }
        }
        Some(Action::Enter(id)) => {
            if let Err(e) = state.activate(&id, &poller, &mut frame, &mut reveal, &mut interaction)
            {
                state.message = e;
            }
        }
        Some(Action::Manage) => {
            state.open_settings(true);
            state.page = UiPage::Management;
        }
        Some(Action::SaveRoles) => {
            if let Err(e) = state.save_roles() {
                state.message = e;
            } else {
                for c in &state.draft.connections {
                    resolver.request(&c.id, &c.status_url);
                }
            }
        }
        Some(Action::CancelRoles) => {
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
            if let Err(e) = state.save_preferences() {
                state.message = e;
            }
        }
        Some(Action::Close) => {
            if state.active_url.is_empty() {
                exit.write(AppExit::Success);
            } else {
                state.open = false;
                state.page = UiPage::Settings;
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
