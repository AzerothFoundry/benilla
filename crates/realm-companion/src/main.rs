mod hit_test;
mod icon;
mod interaction;
#[cfg(target_os = "macos")]
mod menu;
mod names;
mod poller;
mod presentation;
mod skin;
mod speech;
mod performance;
mod native_wake;
mod emote_audio;
mod ui;
use interaction::{dance_allowed, Interaction};
mod settings;
use poller::Poller;

use benilla_app::companion_renderer::{
    add_companion_renderer, register_companion_assets, CompanionDisplayAspect, CompanionFrame,
};
use bevy::{
    prelude::*,
    window::{Monitor, PrimaryMonitor, PrimaryWindow, WindowLevel, WindowMoved, WindowPosition},
};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use serde_json::Value;
use settings::Settings;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Resource, Default)]
struct UiState {
    settings: Settings,
    draft: Settings,
    path: PathBuf,
    open: bool,
    message: String,
    connection: String,
    base_size: Vec2,
    monitor_ready: bool,
    active_data_dir: String,
    recenter: bool,
    restart_requested: Arc<AtomicBool>,
    chooser: bool,
    active_connection_id: Option<String>,
    active_url: String,
    cached_names: std::collections::HashMap<String, String>,
    last_saved_position: Option<[i32; 2]>,
    position_dirty: bool,
    last_move_at: f64,
}
#[derive(Component)]
struct SettingsWindow;
#[derive(Component)]
struct SettingsCamera;

fn main() -> AppExit {
    let path = settings::config_path();
    let read_only_cli=std::env::args().any(|arg|matches!(arg.as_str(),"--export-app-icon"|"--help"));
    let loaded=if read_only_cli{Settings::load(&path)}else{Settings::upgrade_file(&path).and_then(|_|Settings::load(&path))};
    let (mut settings, mut setup_message) = match loaded {
        Ok(s) => (s, String::new()),
        Err(e) => (
            Settings::default(),
            format!("原配置无法读取：{e}，请重新设置"),
        ),
    };
    let mut open = !setup_message.is_empty();
    let mut explicit_url = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--export-app-icon" => {
                let Some(path)=args.next()else{return AppExit::error()};
                if let Err(e)=icon::export(std::path::Path::new(&settings.data_dir),std::path::Path::new(&path)){eprintln!("{e}");return AppExit::error();}
                return AppExit::Success;
            }
            "--settings" => open = true,
            "--url" => {
                if let Some(v) = args.next() {
                    match settings::normalize_status_url(&v) {
                        Ok(value) => {
                            settings.status_url = value.clone();
                            if let Some(id) = settings.selected_connection_id.clone() {
                                if let Some(c) =
                                    settings.connections.iter_mut().find(|c| c.id == id)
                                {
                                    c.status_url = value;
                                }
                            }
                            explicit_url = true;
                        }
                        Err(e) => {
                            eprintln!("{e}");
                            return AppExit::error();
                        }
                    }
                }
            }
            "--data" => {
                if let Some(v) = args.next() {
                    settings.data_dir = v;
                }
            }
            "--help" => {
                println!("realm-companion [--settings] [--url FULL_STATUS_URL] [--data WOW_DATA_DIRECTORY]\nRight click / F2: settings; left drag: move. Settings persist in {}",path.display());
                return AppExit::Success;
            }
            _ => {
                eprintln!("Unknown argument: {arg}");
                return AppExit::error();
            }
        }
    }
    if let Err(e) = settings.validate() {
        setup_message = e;
        open = true;
    }
    let chooser =
        (!explicit_url && settings.connections.len() > 1) || settings.connections.is_empty();
    open |= chooser;
    let active_connection_id = if chooser {
        None
    } else {
        settings.selected_connection_id.clone()
    };
    let active_url = if chooser {
        String::new()
    } else {
        settings.status_url.clone()
    };
    let cached = settings.load_cached_appearance(&path).ok().flatten();
    let cached_names = connection_names(&settings, &path);
    let data = PathBuf::from(&settings.data_dir);
    let mut app = App::new();
    app.init_resource::<benilla_app::companion_renderer::CompanionLocalGesture>();
    app.insert_resource(ClearColor(Color::NONE));
    app.insert_resource(presentation::Reveal::default());
    app.insert_resource(SkinState::default());
    if let Ok(icon) = icon::load(&data, &path) {
        app.insert_resource(icon);
    }
    let render_enabled = if data.is_dir() {
        match register_companion_assets(&mut app, &data) {
            Ok(()) => true,
            Err(e) => {
                setup_message = format!("无法读取游戏资源：{e:#}");
                open = true;
                false
            }
        }
    } else {
        setup_message = "请选择本机 WoW Data 资源目录，保存并重启".into();
        open = true;
        false
    };
    let base_size = Vec2::new(233.0, 268.0);
    let mut window = Window {
        title: "Realm Companion".into(),
        resolution: (base_size.x as u32, base_size.y as u32).into(),
        transparent: true,
        visible: false,
        decorations: false,
        resizable: false,
        has_shadow: false,
        composite_alpha_mode: bevy::window::CompositeAlphaMode::PostMultiplied,
        window_level: if settings.always_on_top {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        },
        ..default()
    };
    if let Some([x, y]) = settings.position {
        window.position = WindowPosition::At(IVec2::new(x, y));
    }
    let defaults=benilla_world::boot::tuned_default_plugins(window.clone());
    let defaults=if std::env::var("REALM_COMPANION_POOLS").as_deref()==Ok("game"){defaults}else{
        defaults.set(bevy::app::TaskPoolPlugin{task_pool_options:bevy::app::TaskPoolOptions{
            min_total_threads:4,max_total_threads:4,
            io:bevy::app::TaskPoolThreadAssignmentPolicy{min_threads:2,max_threads:2,percent:0.5,on_thread_spawn:None,on_thread_destroy:None},
            async_compute:bevy::app::TaskPoolThreadAssignmentPolicy{min_threads:1,max_threads:1,percent:0.25,on_thread_spawn:None,on_thread_destroy:None},
            compute:bevy::app::TaskPoolThreadAssignmentPolicy{min_threads:1,max_threads:1,percent:0.25,on_thread_spawn:None,on_thread_destroy:None},
        }})
    };
    app.add_plugins(
        defaults
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>()
            .set(WindowPlugin {
                primary_window: Some(window.clone()),
                close_when_requested: false,
                ..default()
            }),
    );
    if render_enabled {
        add_companion_renderer(&mut app, data);
    } else {
        app.add_systems(Startup, |mut c: Commands| {
            c.spawn((
                Camera2d,
                Camera {
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
            ));
        });
    }
    app.init_resource::<CompanionFrame>()
        .init_resource::<CompanionDisplayAspect>()
        .init_resource::<benilla_app::companion_renderer::CompanionVisualState>()
        .init_resource::<benilla_app::companion_renderer::CompanionHitMask>();
    app.insert_resource(CompanionFrame(presentation::preview(cached)));
    app.insert_resource(bevy_egui::EguiGlobalSettings {
        auto_create_primary_context: false,
        ..default()
    });
    app.add_plugins(EguiPlugin::default());
    app.add_systems(Startup, setup_settings_window);
    #[cfg(target_os = "macos")]
    menu::install(&mut app);
    app.insert_resource(Poller::new(active_url.clone()));
    app.init_resource::<Interaction>();
    hit_test::install(&mut app);
    app.insert_resource(benilla_app::companion_renderer::CompanionFxOptions {spell_effects:settings.show_spell_effects,enchant_effects:settings.show_enchant_effects,equipment_effects:settings.show_equipment_effects});
    app.insert_resource(benilla_app::companion_renderer::CompanionAppearanceOptions{show_helm:settings.show_helm,equipment_visibility:settings.equipment_visibility.clone()});
    let mut resolver = names::NameResolver::default();
    resolver.set_data_dir(&settings.data_dir);
    for c in &settings.connections {
        resolver.request(&c.id, &c.status_url);
    }
    app.insert_resource(resolver);
    let restart_requested = Arc::new(AtomicBool::new(false));
    app.insert_resource(UiState {
        restart_requested: Arc::clone(&restart_requested),
        draft: settings.clone(),
        chooser,
        active_connection_id,
        active_url,
        cached_names,
        last_saved_position: settings.position,
        position_dirty: false,
        last_move_at: 0.0,
        recenter: open,
        path,
        open,
        message: setup_message,
        connection: "正在连接…".into(),
        base_size,
        monitor_ready: false,
        active_data_dir: if render_enabled {
            settings.data_dir.clone()
        } else {
            String::new()
        },
        settings,
    });
    app.add_systems(
        Update,
        (
            poll_status,
            names::updates,
            window_controls,
            apply_local_actions,
            sync_visual_preferences,
            settings_window_controls,
            visual_probe,
            presentation::reveal_avatar,
        )
            .chain(),
    );
    app.add_systems(
        EguiPrimaryContextPass,
        (fonts, load_skin, ui::settings_ui).chain(),
    );
    speech::install(&mut app);
    emote_audio::install(&mut app);
    performance::install(&mut app);
    performance::install_refresh(&mut app);
    performance::install_scene_executor(&mut app);
    let result = app.run();
    if restart_requested.load(Ordering::Relaxed) {
        if let Err(e) =
            std::env::current_exe().and_then(|exe| std::process::Command::new(exe).spawn())
        {
            eprintln!("Restart: {e}");
        }
    }
    result
}
fn fonts(mut contexts: EguiContexts, mut installed: Local<bool>) -> Result {
    if *installed {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    *installed = true;
    let candidates = [
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "C:/Windows/Fonts/msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut fonts = egui::FontDefinitions::default();
            fonts
                .font_data
                .insert("cjk".into(), Arc::new(egui::FontData::from_owned(bytes)));
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "cjk".into());
            ctx.set_fonts(fonts);
            break;
        }
    }
    Ok(())
}
fn poll_status(
    poller: Res<Poller>,
    mut frame: ResMut<CompanionFrame>,
    mut ui: ResMut<UiState>,
    mut interaction: ResMut<Interaction>,
) {
    let latest = poller.latest.lock().unwrap().take();
    if let Some((generation, result)) = latest {
        if generation != poller.url.lock().unwrap().0 {
            return;
        }
        match result {
            Ok(mut data) => {
                let remote_appearance = data["appearance"].is_object();
                if data["appearance"].is_null() {
                    data["appearance"] = frame.0["appearance"].clone();
                } else if data["appearance"] != frame.0["appearance"] {
                    if let Some(id) = &ui.active_connection_id {
                        let _ = Settings::save_cached_appearance_for(
                            &ui.path,
                            &settings::cache_identity(id, &ui.active_url),
                            &data,
                        );
                    }
                }
                if let Some(id) = &ui.active_connection_id {
                    if remote_appearance && data["character"] != frame.0["character"] {
                        let _ = Settings::save_cached_appearance_for(
                            &ui.path,
                            &settings::cache_identity(id, &ui.active_url),
                            &data,
                        );
                    }
                    if let Some(name) = data["character"].as_str().filter(|s| !s.is_empty()) {
                        let id = id.clone();
                        let url = ui.active_url.clone();
                        if let Err(e) = names::apply_name(&mut ui, &id, &url, name) {
                            warn!("name persistence: {e}");
                        }
                    }
                }
                if std::env::var_os("REALM_COMPANION_PERF").is_some(){
                    if let Some(sent)=data["_fixture_sent_ms"].as_u64(){
                        let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0,|d|d.as_millis() as u64);
                        info!("companion fixture delivery: delay_ms={}",now.saturating_sub(sent));
                    }
                }
                interaction.server_update(&frame.0, &data);
                ui.connection = if data["connected"] == true {
                    format!(
                        "{} · 已连接{}",
                        data["character"].as_str().unwrap_or("角色"),
                        if data["_transport"] == "sse" {
                            "（SSE）"
                        } else {
                            "（轮询）"
                        }
                    )
                } else {
                    "角色离线".into()
                };
                frame.0 = data;
            }
            Err(e) => {
                ui.connection = e;
                interaction.cancel();
                frame.0["connected"] = Value::Bool(false);
            }
        }
    }
}
fn window_controls(
    mut ui: ResMut<UiState>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    monitors: Query<&Monitor, With<PrimaryMonitor>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gate: Res<hit_test::PointerGate>,
    keys: Res<ButtonInput<KeyCode>>,
    mut moved: MessageReader<WindowMoved>,
    mut clicks:MessageReader<bevy::input::mouse::MouseButtonInput>,
    mut display_aspect: ResMut<CompanionDisplayAspect>,
    pet_entity: Single<Entity, With<PrimaryWindow>>,
    mut interaction: ResMut<Interaction>,
    frame: Res<CompanionFrame>,
    time: Res<Time>,
) {
    if !ui.monitor_ready {
        if let Some(m) = monitors.iter().next() {
            let logical_height = m.physical_height as f32 / m.scale_factor as f32;
            ui.base_size = Vec2::new(233.0, 268.0) * (logical_height / 768.0);
            display_aspect.0 = m.physical_width as f32 / m.physical_height as f32;
            ui.monitor_ready = true;
        }
    }
    let mut left_pressed=false;let mut left_released=false;let mut right_pressed=false;
    for event in clicks.read(){
        if event.window!=*pet_entity{continue;}
        match (event.button,event.state){
            (MouseButton::Left,bevy::input::ButtonState::Pressed)=>left_pressed=true,
            (MouseButton::Left,bevy::input::ButtonState::Released)=>left_released=true,
            (MouseButton::Right,bevy::input::ButtonState::Pressed)=>right_pressed=true,
            _=>{}
        }
    }
    if right_pressed || keys.just_pressed(KeyCode::F2)
    {
        ui.open_settings(false);
    }
    if !ui.chooser && left_pressed && (gate.hit||gate.captured) {
        if let Some(point) = gate.point.or(window.cursor_position()) {
            interaction.press(point);
        }
    }
    if mouse.pressed(MouseButton::Left) {
        if let Some(point) = gate.point.or(window.cursor_position()) {
            if interaction.move_pointer(point) {
                window.start_drag_move();
            }
        }
    }
    if left_released||mouse.just_released(MouseButton::Left) {
        interaction.release(
            (time.elapsed_secs_f64() * 1000.0) as u64,
            gate.point.or(window.cursor_position()),
            dance_allowed(&frame.0),
        );
    }
    for event in moved.read() {
        if event.window != *pet_entity {
            continue;
        }
        ui.settings.position = Some([event.position.x, event.position.y]);
        ui.position_dirty = true;
        ui.last_move_at = time.elapsed_secs_f64();
    }
    if ui.position_dirty && time.elapsed_secs_f64() - ui.last_move_at > 0.5 {
        let mut before = ui.settings.clone();
        before.position = ui.last_saved_position;
        if ui.settings.save_merged(&before, &ui.path).is_ok() {
            ui.last_saved_position = ui.settings.position;
            ui.position_dirty = false;
        }
    }
    let size = ui.base_size * (ui.preview_settings().scale_percent / 100.0);
    if (window.width() - size.x).abs() > 1.0 || (window.height() - size.y).abs() > 1.0 {
        window.resolution.set(size.x, size.y);
    }
    let level=if ui.settings.always_on_top{WindowLevel::AlwaysOnTop}else{WindowLevel::Normal};
    if window.window_level!=level{window.window_level=level;}
}
// Optional local visual acceptance probe; never writes into the game install.
fn visual_probe(
    mut c: Commands,
    time: Res<Time>,
    mut captured: Local<bool>,
    mut gesture_probed: Local<bool>,
    mut probe_gesture: ResMut<benilla_app::companion_renderer::CompanionLocalGesture>,
    mut interaction: ResMut<Interaction>,
    frame: Res<CompanionFrame>,
    mut exit: MessageWriter<AppExit>,
    settings_window: Query<Entity, With<SettingsWindow>>,
    ui: Res<UiState>,
) {
    // Explicit opt-in visual fixture: drive the same double-click interaction, never game state.
    if !*gesture_probed && time.elapsed_secs() > 4.0 && std::env::var_os("REALM_COMPANION_GESTURE_PROBE").is_some() && dance_allowed(&frame.0) {
        let ms=(time.elapsed_secs_f64()*1000.) as u64;
        interaction.press(Vec2::ZERO); interaction.release(ms,Some(Vec2::ZERO),true);
        interaction.press(Vec2::ZERO); interaction.release(ms+100,Some(Vec2::ZERO),true);
        if let Ok(id)=std::env::var("REALM_COMPANION_GESTURE_PROBE").unwrap_or_default().parse::<u16>() {
            probe_gesture.selected=Some(id); probe_gesture.selected_request=interaction.request;
        }
        *gesture_probed=true;
    }
    let capture_after = std::env::var("REALM_COMPANION_CAPTURE_AFTER")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(12.0);
    if !*captured && time.elapsed_secs() > capture_after {
        *captured = true;
        if let Some(path) = std::env::var_os("REALM_COMPANION_CAPTURE") {
            let path = PathBuf::from(path);
            let screenshot =
                if std::env::var_os("REALM_COMPANION_CAPTURE_SETTINGS").is_some() && ui.open {
                    settings_window
                        .single()
                        .map(bevy::render::view::screenshot::Screenshot::window)
                        .unwrap_or_else(|_| {
                            bevy::render::view::screenshot::Screenshot::primary_window()
                        })
                } else {
                    bevy::render::view::screenshot::Screenshot::primary_window()
                };
            c.spawn(screenshot).observe(
                move |shot: On<bevy::render::view::screenshot::ScreenshotCaptured>| match shot
                    .image
                    .clone()
                    .try_into_dynamic()
                {
                    Ok(image) => {
                        if let Err(e) = image.save(&path) {
                            error!("capture: {e}");
                        }
                    }
                    Err(e) => error!("capture conversion: {e}"),
                },
            );
        }
    }
    if let Ok(s) = std::env::var("REALM_COMPANION_EXIT_AFTER") {
        if s.parse::<f32>()
            .is_ok_and(|secs| time.elapsed_secs() > secs)
        {
            exit.write(AppExit::Success);
        }
    }
}

fn setup_settings_window(mut c: Commands, ui: Res<UiState>) {
    let window = c
        .spawn((
            SettingsWindow,
            benilla_app::companion_renderer::CompanionSelectorWindow,
            Window {
                title: "桌宠设置".into(),
                resolution: (1120, 630).into(),
                visible: ui.open,
                position: WindowPosition::Centered(bevy::window::MonitorSelection::Primary),
                window_level: WindowLevel::AlwaysOnTop,
                resizable: false,
                ..default()
            },
        ))
        .id();
    c.spawn((
        SettingsCamera,
        Camera2d,
        Camera {
            is_active: ui.open,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.11, 0.10, 0.09)),
            ..default()
        },
        bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(window)),
        bevy_egui::PrimaryEguiContext,
    ));
}
fn settings_window_controls(
    mut ui: ResMut<UiState>,
    mut windows: Query<(Entity, &mut Window), With<SettingsWindow>>,
    mut cameras: Query<&mut Camera, With<SettingsCamera>>,
    mut close: MessageReader<bevy::window::WindowCloseRequested>,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok((id, mut window)) = windows.single_mut() else {
        return;
    };
    let size=if ui.chooser{Vec2::new(1120.,630.)}else{Vec2::new(760.,680.)};
    if (window.width()-size.x).abs()>1.||(window.height()-size.y).abs()>1.{window.resolution.set(size.x,size.y);window.position=WindowPosition::Centered(bevy::window::MonitorSelection::Primary);}
    for event in close.read() {
        if event.window == id {
            if ui.chooser && ui.active_url.is_empty() {
                exit.write(AppExit::Success);
            }
            ui.chooser = false;
            ui.open = false;
        } else {
            exit.write(AppExit::Success);
        }
    }
    let title=if ui.chooser{"选择角色"}else{"桌宠设置"};
    if window.title!=title{window.title=title.into();}
    if window.visible!=ui.open{window.visible=ui.open;}
    if ui.recenter && ui.open {
        window.position = WindowPosition::Centered(bevy::window::MonitorSelection::Primary);
        window.focused = true;
        ui.recenter = false;
    }
    for mut camera in &mut cameras {
        if camera.is_active!=ui.open{camera.is_active=ui.open;}
    }
}

fn apply_local_actions(mut frame: ResMut<CompanionFrame>, mut interaction: ResMut<Interaction>, mut gesture: ResMut<benilla_app::companion_renderer::CompanionLocalGesture>) {
    if frame.0["_local_preview"] == true {
        frame.0["animation"]["sleeping"] = settings::offline_sleeping(presentation::local_hour()).into();
    }
    if !dance_allowed(&frame.0) || (interaction.dancing && gesture.finished == interaction.request) {
        interaction.cancel();
    }
    gesture.request = interaction.request;
    gesture.active = interaction.dancing;
}

#[derive(Resource, Default)]
struct SkinState {
    value: Option<skin::WowSkin>,
    attempted: bool,
}
fn load_skin(mut contexts: EguiContexts, ui: Res<UiState>, mut skin: ResMut<SkinState>) -> Result {
    if !skin.attempted {
        let ctx = contexts.ctx_mut()?;
        skin.attempted = true;
        match skin::WowSkin::load(std::path::Path::new(&ui.active_data_dir), ctx) {
            Ok(value) => {
                value.apply(ctx);
                skin.value = Some(value);
            }
            Err(e) => warn!("WoW UI resources: {e}"),
        }
    }
    Ok(())
}
fn connection_names(
    settings: &Settings,
    path: &std::path::Path,
) -> std::collections::HashMap<String, String> {
    settings
        .connections
        .iter()
        .filter_map(|c| {
            Settings::load_cached_appearance_for(
                path,
                &settings::cache_identity(&c.id, &c.status_url),
            )
            .ok()
            .flatten()
            .and_then(|a| a.character)
            .map(|name| (c.id.clone(), name))
        })
        .collect()
}
impl UiState {
    fn preview_settings(&self)->&Settings{if self.open&&!self.chooser{&self.draft}else{&self.settings}}
    fn open_settings(&mut self, chooser: bool) {
        if let Ok(latest) = Settings::load(&self.path) {
            self.settings.connections = latest.connections;
            self.settings.selected_connection_id = match self.active_connection_id.as_ref() {
                Some(id) if self.settings.connections.iter().any(|c| &c.id == id) => {
                    Some(id.clone())
                }
                Some(_) => {
                    self.message = "当前连接已从列表删除，仍在运行；可选择其它连接。".into();
                    None
                }
                None => latest.selected_connection_id,
            };
            self.settings.status_url = self
                .settings
                .active_connection()
                .map(|c| c.status_url.clone())
                .unwrap_or_default();
        }
        self.cached_names = connection_names(&self.settings, &self.path);
        self.draft = self.settings.clone();
        self.open = true;
        self.recenter = true;
        self.chooser = chooser;
    }
    fn activate(
        &mut self,
        id: &str,
        poller: &Poller,
        frame: &mut CompanionFrame,
        reveal: &mut presentation::Reveal,
        interaction: &mut Interaction,
    ) -> Result<(), String> {
        let before = Settings::load(&self.path)?;
        let mut selected = before.clone();
        selected.select_connection(id)?;
        let merged = selected.save_merged(&before, &self.path)?;
        let connection = merged
            .connections
            .iter()
            .find(|c| c.id == id)
            .ok_or("连接已删除，请刷新列表")?
            .clone();
        self.settings.connections = merged.connections;
        self.settings.select_connection(id)?;
        if self.active_connection_id.as_deref() == Some(id)
            && self.active_url == connection.status_url
        {
            self.open = false;
            self.chooser = false;
            self.draft = self.settings.clone();
            return Ok(());
        }
        self.active_connection_id = Some(id.into());
        self.active_url = connection.status_url.clone();
        frame.0 = presentation::preview(Settings::load_cached_appearance_for(
            &self.path,
            &settings::cache_identity(id, &connection.status_url),
        )?);
        reveal.reset();
        interaction.cancel();
        poller.set_url(connection.status_url);
        self.open = false;
        self.chooser = false;
        self.connection = "正在连接…".into();
        self.draft = self.settings.clone();
        Ok(())
    }
}

fn sync_visual_preferences(ui:Res<UiState>,mut options:ResMut<benilla_app::companion_renderer::CompanionFxOptions>,mut appearance:ResMut<benilla_app::companion_renderer::CompanionAppearanceOptions>){
    let preview=ui.preview_settings();
    let next=benilla_app::companion_renderer::CompanionFxOptions {spell_effects:preview.show_spell_effects,enchant_effects:preview.show_enchant_effects,equipment_effects:preview.show_equipment_effects};
    if *options!=next{*options=next;}
    if appearance.show_helm!=preview.show_helm{appearance.show_helm=preview.show_helm;}
    if appearance.equipment_visibility!=preview.equipment_visibility{appearance.equipment_visibility=preview.equipment_visibility.clone();}
}

#[cfg(test)]
mod local_gesture_tests {
    use super::*;
    #[test]
    fn preview_changes_are_live_only_while_editing_and_do_not_mutate_saved_values(){
        let mut ui=UiState::default();ui.settings.scale_percent=100.;ui.draft=ui.settings.clone();ui.open=true;ui.chooser=false;
        ui.draft.scale_percent=175.;ui.draft.idle_fps=12;ui.draft.active_fps=60;ui.draft.set_equipment_visibility(Some(false));
        assert_eq!(ui.preview_settings().active_fps,60);assert_eq!(ui.settings.active_fps,30);
        assert_eq!(ui.preview_settings().scale_percent,175.);assert_eq!(ui.preview_settings().show_helm,Some(false));assert_eq!(ui.settings.scale_percent,100.);
        ui.open=false;assert_eq!(ui.preview_settings().active_fps,30);assert_eq!(ui.preview_settings().scale_percent,100.);assert_ne!(ui.preview_settings().show_helm,Some(false));
        ui.open=true;ui.chooser=true;assert_eq!(ui.preview_settings().scale_percent,100.);
    }
    #[test]
    fn idle_hit_cache_never_reuses_a_stale_changed_or_dynamic_pose(){
        let mut mask=benilla_app::companion_renderer::CompanionHitMask::default();mask.valid=true;mask.stable_idle=true;mask.current_stamp=7;mask.pose_stamp=7;mask.captured_at=1.;
        assert!(mask.usable(10.,true));assert!(!mask.usable(10.,false));
        mask.current_stamp=8;assert!(!mask.usable(10.,true));
        mask.current_stamp=7;mask.stable_idle=false;assert!(!mask.usable(10.,true));assert!(mask.usable(1.1,true));
        mask.valid=false;assert!(!mask.usable(1.1,true));
    }
    #[test]
    fn speech_headroom_is_passthrough_and_body_mask_maps_below_it(){
        let mut mask=benilla_app::companion_renderer::CompanionHitMask::default();
        mask.valid=true;mask.width=100;mask.height=100;mask.top_fraction=44./268.;mask.alpha=vec![0;10000];
        mask.alpha[50*100+50]=255;
        assert!(!mask.contains(Vec2::new(50.,20.),Vec2::new(100.,268.)));
        assert!(mask.contains(Vec2::new(50.,44.+224.*0.5),Vec2::new(100.,268.)));
        assert!(!mask.contains(Vec2::new(5.,44.+224.*0.5),Vec2::new(100.,268.)));
    }
    #[test]
    fn coarse_clock_entropy_does_not_alternate_only_first_two_gestures(){
        let selected:std::collections::HashSet<_>=(0..200).map(|i|benilla_app::companion_renderer::local_gesture_index(i*1000,20)).collect();
        assert!(selected.len()>15);
    }

    #[test]
    fn completed_gesture_returns_to_untouched_authoritative_state() {
        let original=serde_json::json!({"connected":true,"animation":{"moving":true,"locomotion":"run"}});
        let mut app=App::new();
        app.insert_resource(CompanionFrame(original.clone()));
        let mut interaction=Interaction::default();interaction.dancing=true;interaction.request=(1u64<<63)|42;
        let mut gesture=benilla_app::companion_renderer::CompanionLocalGesture::default();gesture.finished=interaction.request;
        app.insert_resource(interaction).insert_resource(gesture).add_systems(Update,apply_local_actions);
        app.update();
        assert!(!app.world().resource::<Interaction>().dancing);
        assert!(!app.world().resource::<benilla_app::companion_renderer::CompanionLocalGesture>().active);
        assert_eq!(app.world().resource::<CompanionFrame>().0,original);
    }
}

#[cfg(test)]mod window_input_tests{
 use super::*;
 #[test]fn body_mouse_events_work_without_focus_and_settings_clicks_do_not_leak(){
  for own in [true,false]{
   let mut app=App::new();app.add_plugins(MinimalPlugins).init_resource::<UiState>().init_resource::<Interaction>().init_resource::<hit_test::PointerGate>().init_resource::<ButtonInput<MouseButton>>().init_resource::<ButtonInput<KeyCode>>().init_resource::<CompanionDisplayAspect>().add_message::<WindowMoved>().add_message::<bevy::input::mouse::MouseButtonInput>();
   app.insert_resource(CompanionFrame(serde_json::json!({"connected":true,"animation":{"dead":false}})));
   let pet=app.world_mut().spawn((Window{focused:false,..default()},PrimaryWindow)).id();let other=app.world_mut().spawn(Window::default()).id();
   {let mut ui=app.world_mut().resource_mut::<UiState>();ui.chooser=false;ui.monitor_ready=true;ui.base_size=Vec2::new(233.,268.);}
   {let mut gate=app.world_mut().resource_mut::<hit_test::PointerGate>();gate.hit=true;gate.point=Some(Vec2::ZERO);}
   app.add_systems(Update,window_controls);
   for _ in 0..2{
    for state in [bevy::input::ButtonState::Pressed,bevy::input::ButtonState::Released]{app.world_mut().resource_mut::<Messages<bevy::input::mouse::MouseButtonInput>>().write(bevy::input::mouse::MouseButtonInput{button:MouseButton::Left,state,window:if own{pet}else{other}});}
    app.update();
   }
   assert_eq!(app.world().resource::<Interaction>().dancing,own);
  }
 }
}
