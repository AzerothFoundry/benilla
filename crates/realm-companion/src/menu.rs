//! Native macOS menu-bar entry, independent of the avatar's screen position.
use crate::UiState;
use bevy::prelude::*;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};
struct DesktopMenu {
    _tray: TrayIcon,
    settings: MenuId,
    quit: MenuId,
    choose: MenuId,
    shown_class: Option<u8>,
    tooltip: String,
}
pub fn install(app: &mut App) {
    app.add_systems(Startup, setup)
        .add_systems(Update, (events, spaces, refresh_icon));
}
fn setup(world: &mut World) {
    let menu = Menu::new();
    let settings = MenuItem::new("设置…", true, None);
    let quit = MenuItem::new("退出桌宠", true, None);
    let choose = MenuItem::new("选择角色…", true, None);
    if let Err(e) =
        menu.append_items(&[&settings, &choose, &PredefinedMenuItem::separator(), &quit])
    {
        error!("Menu: {e}");
        return;
    }
    let mut builder = TrayIconBuilder::new();
    if let Some(icon) = world.get_resource::<crate::icon::AppIcon>() {
        use objc2::ClassType;
        use objc2_app_kit::{NSApplication, NSImage};
        use objc2_foundation::{MainThreadMarker, NSString};
        if let Some(mtm) = MainThreadMarker::new() {
            // SAFETY: native icon initialization runs on the main thread with our local PNG.
            unsafe {
                if let Some(image) = NSImage::initWithContentsOfFile(
                    NSImage::alloc(),
                    &NSString::from_str(&icon.path.to_string_lossy()),
                ) {
                    NSApplication::sharedApplication(mtm).setApplicationIconImage(Some(&image));
                }
            }
        }
        if let Ok(icon) = tray_icon::Icon::from_rgba(icon.menu_wow.clone(), 18, 18) {
            builder = builder.with_icon(icon);
        }
    } else {
        builder = builder.with_title("⚔");
    }
    match builder
        .with_tooltip("Realm Companion")
        .with_menu(Box::new(menu))
        .build()
    {
        Ok(tray) => world.insert_non_send_resource(DesktopMenu {
            _tray: tray,
            settings: settings.id().clone(),
            quit: quit.id().clone(),
            choose: choose.id().clone(),
            shown_class: None,
            tooltip: "Realm Companion".into(),
        }),
        Err(e) => error!("Menu bar: {e}"),
    }
}
fn events(
    menu: Option<NonSend<DesktopMenu>>,
    mut ui: ResMut<UiState>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some(menu) = menu else { return };
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        if event.id == menu.settings {
            ui.open_settings(false);
        } else if event.id == menu.choose {
            ui.request_selector();
        } else if event.id == menu.quit {
            exit.write(AppExit::Success);
        }
    }
}

// AppKit owns Spaces membership. Keep it separate from the window's z-order.
fn spaces(_main_thread: bevy::ecs::system::NonSendMarker, ui: Res<UiState>) {
    use objc2_app_kit::{NSApplication, NSWindowCollectionBehavior as Behavior};
    use objc2_foundation::MainThreadMarker;
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    // SAFETY: this system runs only on the main thread; all windows belong to this app.
    unsafe {
        for window in app.windows().iter() {
            let title = window.title().to_string();
            let old = window.collectionBehavior();
            let mut next = old;
            if title == "Realm Companion" {
                next.remove(
                    Behavior::CanJoinAllSpaces
                        | Behavior::FullScreenAuxiliary
                        | Behavior::FullScreenPrimary
                        | Behavior::MoveToActiveSpace,
                );
                if ui.settings.all_desktops {
                    next.insert(Behavior::CanJoinAllSpaces | Behavior::FullScreenAuxiliary);
                }
            } else if matches!(title.as_str(), "设置" | "桌宠设置" | "选择角色" | "管理角色") {
                next.remove(Behavior::CanJoinAllSpaces);
                next.insert(Behavior::MoveToActiveSpace);
            }
            if old != next {
                window.setCollectionBehavior(next);
                if title == "Realm Companion" {
                    info!(
                        "desktop Spaces: all_desktops={}, collection_behavior={}",
                        ui.settings.all_desktops,
                        next.bits()
                    );
                }
            }
        }
    }
}

fn refresh_icon(
    frame: Res<crate::CompanionFrame>,
    icons: Option<Res<crate::icon::AppIcon>>,
    menu: Option<NonSendMut<DesktopMenu>>,
) {
    let (Some(icons), Some(mut menu)) = (icons, menu) else {
        return;
    };
    let class = if frame.0["connected"] == true {
        frame.0["class"]
            .as_u64()
            .and_then(|n| u8::try_from(n).ok())
            .filter(|c| icons.classes.contains_key(c))
    } else {
        None
    };
    let tooltip = if frame.0["connected"] == true {
        format!(
            "Realm Companion · {}",
            frame.0["character"].as_str().unwrap_or("角色")
        )
    } else {
        "Realm Companion · 未连接".into()
    };
    if menu.tooltip != tooltip {
        if menu._tray.set_tooltip(Some(tooltip.as_str())).is_ok() {
            menu.tooltip = tooltip;
        }
    }
    if menu.shown_class == class {
        return;
    }
    let rgba = class
        .and_then(|c| icons.classes.get(&c))
        .unwrap_or(&icons.menu_wow)
        .clone();
    if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, 18, 18) {
        if menu._tray.set_icon(Some(icon)).is_ok() {
            menu.shown_class = class;
            info!("companion menu icon: class={class:?}");
        }
    }
}
