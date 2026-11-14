use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const DEFAULT_URL: &str = "http://192.168.3.16:18112/status?view=companion";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub status_url: String,
}
pub const VISIBLE_EQUIPMENT_SLOTS: &[(u8, &str)] = &[
    (0, "头盔"),
    (2, "肩膀"),
    (14, "披风"),
    (4, "胸甲"),
    (3, "衬衣"),
    (18, "战袍"),
    (5, "腰带"),
    (8, "护腕"),
    (9, "手套"),
    (6, "裤子"),
    (7, "鞋子"),
    (15, "主手"),
    (16, "副手"),
    (17, "远程"),
];
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Compatibility mirror of the active connection; not a second connection registry.
    pub status_url: String,
    pub connections: Vec<Connection>,
    pub selected_connection_id: Option<String>,
    pub data_dir: String,
    pub scale_percent: f32,
    pub always_on_top: bool,
    pub all_desktops: bool,
    pub power_saving_idle: bool,
    pub idle_fps: u32,
    pub active_fps: u32,
    pub show_speech_bubble: bool,
    pub play_emote_sounds: bool,
    pub show_spell_effects: bool,
    pub show_enchant_effects: bool,
    pub show_equipment_effects: bool,
    /// None follows the game's hide-helm flag; Some overrides this desktop only.
    pub show_helm: Option<bool>,
    #[serde(default)]
    pub equipment_visibility: BTreeMap<u8, bool>,
    pub position: Option<[i32; 2]>,
}
fn stable_id(url: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for b in url.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("legacy-{hash:016x}")
}
/// Bind appearance storage to both the profile and its exact normalized source URL.
/// Separate live instances keep writing their own source cache after another edits the profile.
pub fn cache_identity(id: &str, url: &str) -> String {
    format!("{id}-{}", stable_id(url))
}
fn unique() -> String {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!(
        "{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}
fn legacy_connection(url: &str) -> Connection {
    Connection {
        id: stable_id(url),
        name: if url.contains(":18112") {
            "Ada".into()
        } else {
            url::Url::parse(url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .unwrap_or_else(|| "角色".into())
        },
        status_url: url.into(),
    }
}
impl Default for Settings {
    fn default() -> Self {
        let c = legacy_connection(DEFAULT_URL);
        Self {
            status_url: DEFAULT_URL.into(),
            selected_connection_id: Some(c.id.clone()),
            connections: vec![c],
            data_dir: std::env::var("WOW_DATA").unwrap_or_default(),
            scale_percent: 100.,
            always_on_top: true,
            all_desktops: true,
            power_saving_idle: true,
            idle_fps: 4,
            active_fps: 30,
            show_speech_bubble: true,
            play_emote_sounds: false,
            show_spell_effects: true,
            show_enchant_effects: true,
            show_equipment_effects: true,
            show_helm: None,
            equipment_visibility: BTreeMap::new(),
            position: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppearanceCache {
    pub character: Option<String>,
    pub appearance: serde_json::Value,
}
impl Settings {
    pub fn set_equipment_visibility(&mut self,value:Option<bool>){
        self.show_helm=value;
        self.equipment_visibility.clear();
        if let Some(value)=value{for (slot,_) in VISIBLE_EQUIPMENT_SLOTS{if *slot!=0{self.equipment_visibility.insert(*slot,value);}}}
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let legacy = value.get("connections").is_none();
        let mut settings: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if legacy {
            settings.connections = if settings.status_url.is_empty() {
                vec![]
            } else {
                vec![legacy_connection(&settings.status_url)]
            };
            settings.selected_connection_id = settings.connections.first().map(|c| c.id.clone());
        }
        for connection in &mut settings.connections {
            connection.status_url = normalize_status_url(&connection.status_url)?;
        }
        if settings.connections.len() == 1 && settings.selected_connection_id.is_none() {
            settings.selected_connection_id = Some(settings.connections[0].id.clone());
        }
        settings.sync_active_url();
        settings.validate()?;
        if legacy {
            if let Some(connection) = settings.active_connection() {
                let target = cache_path(
                    path,
                    &cache_identity(&connection.id, &connection.status_url),
                )?;
                let old = path.with_file_name("last-avatar.json");
                if !target.exists() && old.exists() {
                    if let Some(frame) = std::fs::read(old)
                        .ok()
                        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    {
                        if let Some(cache) = AppearanceCache::from_frame(&frame)
                            .or_else(|| legacy_raw_appearance(&frame))
                        {
                            write_locked(&target, &cache)?;
                        }
                    }
                }
            }
        }
        Ok(settings)
    }
    pub fn active_connection(&self) -> Option<&Connection> {
        self.selected_connection_id
            .as_ref()
            .and_then(|id| self.connections.iter().find(|c| &c.id == id))
    }
    fn sync_active_url(&mut self) {
        self.status_url = self
            .active_connection()
            .map(|c| c.status_url.clone())
            .unwrap_or_default();
    }
    pub fn select_connection(&mut self, id: &str) -> Result<(), String> {
        if !self.connections.iter().any(|c| c.id == id) {
            return Err("连接不存在".into());
        }
        self.selected_connection_id = Some(id.into());
        self.sync_active_url();
        Ok(())
    }
    pub fn add_connection(&mut self, name: String, status_url: String) -> Result<String, String> {
        let status_url = normalize_status_url(&status_url)?;
        let id = format!("connection-{}", unique());
        self.connections.push(Connection {
            id: id.clone(),
            name: if name.trim().is_empty() {
                provisional_name(&status_url)
            } else {
                name
            },
            status_url,
        });
        self.select_connection(&id)?;
        Ok(id)
    }
    pub fn remove_connection(&mut self, id: &str) -> bool {
        let old = self.connections.len();
        self.connections.retain(|c| c.id != id);
        if self.selected_connection_id.as_deref() == Some(id) {
            self.selected_connection_id = self.connections.first().map(|c| c.id.clone());
        }
        self.sync_active_url();
        old != self.connections.len()
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(2..=120).contains(&self.idle_fps) || !(2..=120).contains(&self.active_fps) {
            return Err("待机和活动帧率必须在2–120之间".into());
        }
        if !self.status_url.is_empty() {
            validate_url(&self.status_url)?;
        }
        if self
            .equipment_visibility
            .keys()
            .any(|slot| *slot == 0 || !VISIBLE_EQUIPMENT_SLOTS.iter().any(|(id, _)| id == slot))
        {
            return Err("仅支持有外观的装备槽位，头盔使用 show_helm".into());
        }
        let mut ids = HashSet::new();
        for c in &self.connections {
            if c.id.is_empty()
                || !c
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            {
                return Err("连接 ID 无效".into());
            }
            if !ids.insert(&c.id) {
                return Err("连接 ID 重复".into());
            }
            validate_url(&c.status_url)?;
        }
        if self.selected_connection_id.is_some() && self.active_connection().is_none() {
            return Err("选中的连接不存在".into());
        }
        if !self.scale_percent.is_finite() || !(25.0..=400.0).contains(&self.scale_percent) {
            return Err("缩放范围为 25%–400%".into());
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        write_locked(path, self)
    }
    /// Three-way merge against the latest file. Unchanged stale fields never overwrite other instances.
    pub fn save_merged(&self, before: &Self, path: &Path) -> Result<Self, String> {
        self.validate()?;
        let _lock = transaction_lock(path)?;
        let mut merged = if path.exists() {
            Self::load(path)?
        } else {
            before.clone()
        };
        macro_rules! field {
            ($f:ident) => {
                if self.$f != before.$f {
                    merged.$f = self.$f.clone();
                }
            };
        }
        field!(data_dir);
        field!(scale_percent);
        field!(always_on_top);
        field!(all_desktops);
        field!(power_saving_idle);
        field!(idle_fps);
        field!(active_fps);
        field!(show_speech_bubble);
        field!(play_emote_sounds);
        field!(show_spell_effects);
        field!(show_enchant_effects);
        field!(show_equipment_effects);
        field!(show_helm);
        for slot in self
            .equipment_visibility
            .keys()
            .chain(before.equipment_visibility.keys())
        {
            if self.equipment_visibility.get(slot) != before.equipment_visibility.get(slot) {
                match self.equipment_visibility.get(slot) {
                    Some(value) => {
                        merged.equipment_visibility.insert(*slot, *value);
                    }
                    None => {
                        merged.equipment_visibility.remove(slot);
                    }
                }
            }
        }
        field!(position);
        for old in &before.connections {
            if !self.connections.iter().any(|c| c.id == old.id) {
                merged.connections.retain(|c| c.id != old.id);
            }
        }
        for next in &self.connections {
            match before.connections.iter().find(|c| c.id == next.id) {
                None => {
                    if !merged.connections.iter().any(|c| c.id == next.id) {
                        let mut normalized = next.clone();
                        normalized.status_url = normalize_status_url(&normalized.status_url)?;
                        merged.connections.push(normalized);
                    }
                }
                Some(old) => {
                    if let Some(latest) = merged.connections.iter_mut().find(|c| c.id == next.id) {
                        if next.name != old.name {
                            latest.name = next.name.clone();
                        }
                        if next.status_url != old.status_url {
                            latest.status_url = normalize_status_url(&next.status_url)?;
                        }
                    } else if next != old {
                        return Err("连接已在另一窗口删除，请刷新后重新添加".into());
                    }
                }
            }
        }
        // Retain compatibility for callers that edit only the historical URL field.
        if self.status_url != before.status_url
            && self.connections == before.connections
            && self.selected_connection_id == before.selected_connection_id
        {
            if let Some(c) = merged
                .connections
                .iter_mut()
                .find(|c| Some(&c.id) == self.selected_connection_id.as_ref())
            {
                c.status_url = normalize_status_url(&self.status_url)?;
            }
        }
        if self.selected_connection_id != before.selected_connection_id {
            merged.selected_connection_id = self.selected_connection_id.clone();
        }
        if merged.active_connection().is_none() {
            merged.selected_connection_id = merged.connections.first().map(|c| c.id.clone());
        }
        merged.sync_active_url();
        merged.validate()?;
        write_atomic(path, &merged)?;
        Ok(merged)
    }
    pub fn load_cached_appearance(&self, path: &Path) -> Result<Option<AppearanceCache>, String> {
        let Some(c) = self.active_connection() else {
            return Ok(None);
        };
        Self::load_cached_appearance_for(path, &cache_identity(&c.id, &c.status_url))
    }
    pub fn load_cached_appearance_for(
        path: &Path,
        id: &str,
    ) -> Result<Option<AppearanceCache>, String> {
        let p = cache_path(path, id)?;
        if !p.exists() {
            return Ok(None);
        }
        let cache: AppearanceCache =
            serde_json::from_slice(&std::fs::read(p).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if !cache.appearance.is_object() {
            return Err("外观缓存格式无效".into());
        }
        Ok(Some(cache))
    }
    pub fn save_cached_appearance(
        &self,
        path: &Path,
        frame: &serde_json::Value,
    ) -> Result<(), String> {
        let Some(c) = self.active_connection() else {
            return Ok(());
        };
        Self::save_cached_appearance_for(path, &cache_identity(&c.id, &c.status_url), frame)
    }
    pub fn save_cached_appearance_for(
        path: &Path,
        id: &str,
        frame: &serde_json::Value,
    ) -> Result<(), String> {
        if let Some(cache) = AppearanceCache::from_frame(frame) {
            write_locked(&cache_path(path, id)?, &cache)?;
        }
        Ok(())
    }
}
// The first desktop preview persisted the bare appearance, before envelopes existed.
// Only this migration accepts that shape; live-frame cache writes remain strict.
fn legacy_raw_appearance(value: &serde_json::Value) -> Option<AppearanceCache> {
    let race = value.get("race")?.as_u64()?;
    let sex = value.get("sex")?.as_u64()?;
    let display = value
        .get("display_id")
        .and_then(|v| v.as_u64())
        .is_some_and(|v| v > 0);
    let customization = ["skin", "face", "hair_style"]
        .iter()
        .all(|key| value.get(key).and_then(|v| v.as_u64()).is_some());
    ((1..=8).contains(&race) && sex <= 1 && (display || customization)).then(|| AppearanceCache {
        character: None,
        appearance: value.clone(),
    })
}
impl AppearanceCache {
    pub fn from_frame(frame: &serde_json::Value) -> Option<Self> {
        frame
            .get("appearance")
            .filter(|a| a.is_object())
            .map(|a| Self {
                character: frame
                    .get("character")
                    .and_then(|c| c.as_str())
                    .map(str::to_owned),
                appearance: a.clone(),
            })
    }
    /// Build an offline presentation; animation is generated at startup, never cached.
    pub fn offline_frame(&self, local_hour: u32) -> serde_json::Value {
        serde_json::json!({"connected":false,"character":self.character,"appearance":self.appearance,"animation":{"sleeping":offline_sleeping(local_hour)}})
    }
}
pub fn offline_sleeping(local_hour: u32) -> bool {
    !(6..22).contains(&(local_hour % 24))
}
/// Correct only the known event-stream endpoint pasted into the status field.
/// All other full URLs are returned byte-for-byte unchanged after validation.
pub fn normalize_status_url(raw: &str) -> Result<String, String> {
    validate_url(raw)?;
    let mut url =
        url::Url::parse(raw).map_err(|_| "请输入完整的 HTTP/HTTPS status URL".to_string())?;
    let Some(prefix) = url.path().strip_suffix("/companion/events") else {
        return Ok(raw.to_owned());
    };
    let path = format!("{prefix}/status");
    let mut query: Vec<String> = url
        .query()
        .into_iter()
        .flat_map(|q| q.split('&'))
        .filter(|pair| {
            url::form_urlencoded::parse(pair.as_bytes())
                .next()
                .is_none_or(|(key, _)| key != "view")
        })
        .map(str::to_owned)
        .collect();
    query.push("view=companion".into());
    url.set_path(&path);
    url.set_query(Some(&query.join("&")));
    Ok(url.into())
}
pub fn provisional_name(raw: &str) -> String {
    url::Url::parse(raw)
        .ok()
        .and_then(|u| {
            u.host_str().map(|h| match u.port() {
                Some(p) => format!("{h}:{p}"),
                None => h.to_owned(),
            })
        })
        .unwrap_or_else(|| "等待连接".into())
}
fn validate_url(s: &str) -> Result<(), String> {
    let u = url::Url::parse(s).map_err(|_| "请输入完整的 HTTP/HTTPS status URL".to_string())?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
        return Err("URL 必须使用 http:// 或 https://".into());
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err("URL 不应包含账号密码".into());
    }
    Ok(())
}
fn cache_path(config: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err("缓存连接 ID 无效".into());
    }
    Ok(config
        .parent()
        .ok_or("配置路径无效")?
        .join("appearances")
        .join(format!("{id}.json")))
}
fn transaction_lock(path: &Path) -> Result<File, String> {
    let parent = path.parent().ok_or("配置路径无效")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("json.lock"))
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(lock),
            Err(std::fs::TryLockError::WouldBlock) if start.elapsed() < Duration::from_secs(2) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(e) => return Err(format!("配置文件暂时忙碌：{e}")),
        }
    }
}
fn write_locked(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let _lock = transaction_lock(path)?;
    write_atomic(path, value)
}
fn write_atomic(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let tmp = path.with_extension(format!("json.{}.tmp", unique()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result
}
pub fn config_path() -> PathBuf {
    if let Some(p) = std::env::var_os("REALM_COMPANION_CONFIG") {
        return p.into();
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    #[cfg(target_os = "macos")]
    return home.join("Library/Application Support/AzerothFoundry/companion/settings.json");
    #[cfg(not(target_os = "macos"))]
    return std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"))
        .join("azeroth-foundry/companion/settings.json");
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!("companion-settings-test-{}", unique()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn old_settings_migrate_stable_connection_and_only_appearance() {
        let d = dir();
        let p = d.join("settings.json");
        std::fs::write(
            &p,
            r#"{"status_url":"http://localhost:18112/custom?x=1","scale_percent":150}"#,
        )
        .unwrap();
        std::fs::write(d.join("last-avatar.json"),r#"{"character":"Ada","appearance":{"race":8},"animation":{"moving":true},"health_pct":40}"#).unwrap();
        let a = Settings::load(&p).unwrap();
        let b = Settings::load(&p).unwrap();
        assert_eq!(a.connections, b.connections);
        assert_eq!(a.status_url, "http://localhost:18112/custom?x=1");
        let c = a.load_cached_appearance(&p).unwrap().unwrap();
        assert_eq!(c.character.as_deref(), Some("Ada"));
        let cached = std::fs::read_to_string(
            cache_path(
                &p,
                &cache_identity(&a.connections[0].id, &a.connections[0].status_url),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(!cached.contains("animation"));
        assert!(!cached.contains("health"));
        let mut s = a.clone();
        let id = s
            .add_connection("Bran".into(), "http://localhost:20002/status".into())
            .unwrap();
        assert!(Settings::load_cached_appearance_for(&p, &id)
            .unwrap()
            .is_none());
        s.save(&p).unwrap();
        assert!(Settings::load(&p)
            .unwrap()
            .load_cached_appearance(&p)
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn equipment_bulk_actions_cover_helm_and_every_visible_slot_and_restore_inheritance(){
        let mut s=Settings::default();
        for value in [false,true]{s.set_equipment_visibility(Some(value));assert_eq!(s.show_helm,Some(value));for(slot,_)in VISIBLE_EQUIPMENT_SLOTS{if *slot!=0{assert_eq!(s.equipment_visibility.get(slot),Some(&value));}}}
        s.set_equipment_visibility(None);assert!(s.show_helm.is_none());assert!(s.equipment_visibility.is_empty());
    }
    #[test]
    fn speech_preferences_default_safely_and_merge_independently() {
        let old:Settings=serde_json::from_str("{}").unwrap();
        assert!(old.show_speech_bubble);assert!(!old.play_emote_sounds);
        let d=dir();let p=d.join("settings.json");old.save(&p).unwrap();
        let mut audio=old.clone();audio.play_emote_sounds=true;audio.save_merged(&old,&p).unwrap();
        let mut bubble=old.clone();bubble.show_speech_bubble=false;
        let saved=bubble.save_merged(&old,&p).unwrap();assert!(saved.play_emote_sounds);assert!(!saved.show_speech_bubble);
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn frame_rates_upgrade_validate_and_merge_independently() {
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!((old.idle_fps, old.active_fps), (4, 30));
        let d = dir(); let p = d.join("settings.json"); old.save(&p).unwrap();
        let mut idle = old.clone(); idle.idle_fps = 12; idle.save_merged(&old, &p).unwrap();
        let mut active = old.clone(); active.active_fps = 60;
        let saved = active.save_merged(&old, &p).unwrap();
        assert_eq!((saved.idle_fps, saved.active_fps), (12, 60));
        assert_eq!(Settings::load(&p).unwrap(), saved);
        for value in [0, 1, 121, u32::MAX] {
            let mut invalid = old.clone(); invalid.idle_fps = value; assert!(invalid.validate().is_err());
            invalid.idle_fps = 4; invalid.active_fps = value; assert!(invalid.validate().is_err());
        }
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn stale_save_preserves_other_addition_selection_and_window_edits() {
        let d = dir();
        let p = d.join("settings.json");
        let base = Settings::default();
        base.save(&p).unwrap();
        let mut first = base.clone();
        let id = first
            .add_connection("Iris".into(), "http://localhost:20001/status".into())
            .unwrap();
        first.scale_percent = 200.;
        first.save_merged(&base, &p).unwrap();
        let mut stale = base.clone();
        stale.position = Some([90, 100]);
        let merged = stale.save_merged(&base, &p).unwrap();
        assert_eq!(merged.connections.len(), 2);
        assert_eq!(merged.selected_connection_id.as_deref(), Some(id.as_str()));
        assert_eq!(merged.scale_percent, 200.);
        assert_eq!(merged.position, Some([90, 100]));
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn deletion_wins_stale_unchanged_and_conflicts_with_edit() {
        let d = dir();
        let p = d.join("settings.json");
        let base = Settings::default();
        base.save(&p).unwrap();
        let id = base.connections[0].id.clone();
        let mut deletion = base.clone();
        deletion.remove_connection(&id);
        deletion.save_merged(&base, &p).unwrap();
        let mut stale = base.clone();
        stale.position = Some([1, 2]);
        let result = stale.save_merged(&base, &p).unwrap();
        assert!(result.connections.is_empty());
        assert_eq!(result.selected_connection_id, None);
        assert_eq!(result.status_url, "");
        let mut edit = base.clone();
        edit.connections[0].name = "Renamed".into();
        assert!(edit.save_merged(&base, &p).is_err());
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn concurrent_threeway_adds_use_locked_read_modify_write() {
        let d = dir();
        let p = d.join("settings.json");
        let base = Settings::default();
        base.save(&p).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|i| {
                let p = p.clone();
                let base = base.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut s = base.clone();
                    s.add_connection(
                        format!("Bot{i}"),
                        format!("http://localhost:{}/status", 21000 + i),
                    )
                    .unwrap();
                    barrier.wait();
                    s.save_merged(&base, &p).unwrap();
                })
            })
            .collect();
        for t in handles {
            t.join().unwrap();
        }
        let s = Settings::load(&p).unwrap();
        assert_eq!(s.connections.len(), 3);
        assert_eq!(
            std::fs::read_dir(&d)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|p| p.file_name().to_string_lossy().ends_with(".tmp"))
                .count(),
            0
        );
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn caches_are_per_connection_and_never_persist_live_animation() {
        let d = dir();
        let p = d.join("settings.json");
        Settings::save_cached_appearance_for(&p,"ada",&serde_json::json!({"character":"Ada","appearance":{"race":8},"animation":{"moving":true},"connected":true})).unwrap();
        Settings::save_cached_appearance_for(&p,"iris",&serde_json::json!({"character":"Iris","appearance":{"race":5},"animation":{"dead":true}})).unwrap();
        let ada = Settings::load_cached_appearance_for(&p, "ada")
            .unwrap()
            .unwrap();
        let iris = Settings::load_cached_appearance_for(&p, "iris")
            .unwrap()
            .unwrap();
        assert_eq!(ada.appearance["race"], 8);
        assert_eq!(iris.appearance["race"], 5);
        assert!(ada.offline_frame(23)["animation"]["sleeping"]
            .as_bool()
            .unwrap());
        assert!(!ada.offline_frame(12)["animation"]["sleeping"]
            .as_bool()
            .unwrap());
        assert!(ada.offline_frame(12)["animation"].get("moving").is_none());
        assert!(Settings::save_cached_appearance_for(
            &p,
            "../escape",
            &serde_json::json!({"appearance":{}})
        )
        .is_err());
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn empty_list_stays_empty_and_night_boundary_exact() {
        let d = dir();
        let p = d.join("settings.json");
        std::fs::write(
            &p,
            r#"{"connections":[],"selected_connection_id":null,"status_url":""}"#,
        )
        .unwrap();
        let s = Settings::load(&p).unwrap();
        assert!(s.active_connection().is_none());
        assert!(s.connections.is_empty());
        assert!(offline_sleeping(5));
        assert!(!offline_sleeping(6));
        assert!(!offline_sleeping(21));
        assert!(offline_sleeping(22));
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn validation_rejects_bad_url_zoom_and_identity() {
        let mut s = Settings::default();
        s.status_url = "file:///etc/passwd".into();
        assert!(s.validate().is_err());
        s.status_url = DEFAULT_URL.into();
        s.scale_percent = f32::NAN;
        assert!(s.validate().is_err());
        s.scale_percent = 100.;
        s.connections[0].id = "../escape".into();
        assert!(s.validate().is_err());
    }
    #[test]
    fn normalizes_only_known_stream_suffix_and_preserves_proxy_queries() {
        let raw="https://example.test/realm/ada/companion/events?token=a%2Fb%2Bc&view=other&x=1&view=old#part";
        assert_eq!(
            normalize_status_url(raw).unwrap(),
            "https://example.test/realm/ada/status?token=a%2Fb%2Bc&x=1&view=companion#part"
        );
        let custom = "https://example.test/custom/status?token=a%2Fb&view=custom";
        assert_eq!(normalize_status_url(custom).unwrap(), custom);
        assert_eq!(
            normalize_status_url("http://localhost/companion/events").unwrap(),
            "http://localhost/status?view=companion"
        );
        assert_eq!(
            normalize_status_url("http://localhost/companion/events/extra").unwrap(),
            "http://localhost/companion/events/extra"
        );
    }
    #[test]
    fn normalization_applies_to_load_add_and_merge_edits() {
        let d = dir();
        let p = d.join("settings.json");
        std::fs::write(
            &p,
            r#"{"status_url":"http://localhost/proxy/companion/events?token=abc"}"#,
        )
        .unwrap();
        let base = Settings::load(&p).unwrap();
        assert_eq!(
            base.active_connection().unwrap().status_url,
            "http://localhost/proxy/status?token=abc&view=companion"
        );
        base.save(&p).unwrap();
        let mut next = base.clone();
        let id = next
            .add_connection(
                "Iris".into(),
                "http://localhost:20001/companion/events".into(),
            )
            .unwrap();
        assert_eq!(
            next.active_connection().unwrap().status_url,
            "http://localhost:20001/status?view=companion"
        );
        let merged = next.save_merged(&base, &p).unwrap();
        let mut edit = merged.clone();
        edit.connections
            .iter_mut()
            .find(|c| c.id == id)
            .unwrap()
            .status_url = "http://localhost/proxy2/companion/events?view=old".into();
        let saved = edit.save_merged(&merged, &p).unwrap();
        assert_eq!(
            saved.active_connection().unwrap().status_url,
            "http://localhost/proxy2/status?view=companion"
        );
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn migrates_bare_legacy_appearance_without_relaxing_live_cache() {
        let d = dir();
        let p = d.join("settings.json");
        std::fs::write(&p, r#"{"status_url":"http://localhost:18112/status"}"#).unwrap();
        let raw = serde_json::json!({"race":8,"sex":1,"display_id":1479,"skin":2,"face":3,"hair_style":4,"equipment":[]});
        assert!(AppearanceCache::from_frame(&raw).is_none());
        std::fs::write(
            d.join("last-avatar.json"),
            serde_json::to_vec(&raw).unwrap(),
        )
        .unwrap();
        let settings = Settings::load(&p).unwrap();
        let cache = settings.load_cached_appearance(&p).unwrap().unwrap();
        assert_eq!(cache.appearance, raw);
        assert_eq!(cache.character, None);
        assert!(
            legacy_raw_appearance(&serde_json::json!({"race":8,"sex":5,"display_id":1479}))
                .is_none()
        );
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn sole_unselected_connection_is_selected_on_load() {
        let d = dir();
        let p = d.join("settings.json");
        std::fs::write(&p,r#"{"connections":[{"id":"ada","name":"Ada","status_url":"http://localhost/status"}],"selected_connection_id":null,"status_url":""}"#).unwrap();
        let settings = Settings::load(&p).unwrap();
        assert_eq!(settings.selected_connection_id.as_deref(), Some("ada"));
        assert_eq!(settings.status_url, "http://localhost/status");
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn same_profile_different_sources_never_share_appearance_cache() {
        let d = dir();
        let p = d.join("settings.json");
        let a = cache_identity("actor", "http://localhost:20001/status");
        let b = cache_identity("actor", "http://localhost:20002/status");
        assert_ne!(a, b);
        Settings::save_cached_appearance_for(
            &p,
            &a,
            &serde_json::json!({"character":"Ada","appearance":{"race":8}}),
        )
        .unwrap();
        Settings::save_cached_appearance_for(
            &p,
            &b,
            &serde_json::json!({"character":"Bran","appearance":{"race":2}}),
        )
        .unwrap();
        assert_eq!(
            Settings::load_cached_appearance_for(&p, &a)
                .unwrap()
                .unwrap()
                .character
                .as_deref(),
            Some("Ada")
        );
        assert_eq!(
            Settings::load_cached_appearance_for(&p, &b)
                .unwrap()
                .unwrap()
                .character
                .as_deref(),
            Some("Bran")
        );
        let mut settings = Settings::default();
        settings.connections = vec![Connection {
            id: "actor".into(),
            name: "Actor".into(),
            status_url: "http://localhost:20002/status".into(),
        }];
        settings.selected_connection_id = Some("actor".into());
        assert_eq!(
            settings
                .load_cached_appearance(&p)
                .unwrap()
                .unwrap()
                .character
                .as_deref(),
            Some("Bran")
        );
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn corrupt_optional_legacy_cache_does_not_discard_valid_settings() {
        let d = dir();
        let p = d.join("settings.json");
        std::fs::write(
            &p,
            r#"{"status_url":"http://localhost:123/status","data_dir":"/known/data"}"#,
        )
        .unwrap();
        std::fs::write(d.join("last-avatar.json"), "{").unwrap();
        let loaded = Settings::load(&p).unwrap();
        assert_eq!(loaded.data_dir, "/known/data");
        assert_eq!(loaded.connections.len(), 1);
        assert!(loaded.load_cached_appearance(&p).unwrap().is_none());
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn legacy_settings_without_effect_preferences_default_to_enabled() {
        let d = dir();
        let p = d.join("settings.json");
        let mut legacy = serde_json::to_value(Settings::default()).unwrap();
        for key in [
            "show_spell_effects",
            "show_enchant_effects",
            "show_equipment_effects",
        ] {
            legacy.as_object_mut().unwrap().remove(key);
        }
        std::fs::write(&p, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let loaded = Settings::load(&p).unwrap();
        assert!(
            loaded.show_spell_effects
                && loaded.show_enchant_effects
                && loaded.show_equipment_effects
        );
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn effect_preferences_survive_stale_window_saves_without_erasing_other_addresses() {
        let d = dir();
        let p = d.join("settings.json");
        let base = Settings::default();
        base.save(&p).unwrap();
        let mut other = base.clone();
        let second = other
            .add_connection(
                String::new(),
                "http://localhost:20002/status?view=companion".into(),
            )
            .unwrap();
        other.save_merged(&base, &p).unwrap();
        let mut own = base.clone();
        own.show_spell_effects = false;
        own.show_enchant_effects = false;
        own.show_equipment_effects = false;
        own.save_merged(&base, &p).unwrap();
        let saved = Settings::load(&p).unwrap();
        assert_eq!(saved.connections.len(), 2);
        assert_eq!(
            saved.selected_connection_id.as_deref(),
            Some(second.as_str())
        );
        assert!(
            !saved.show_spell_effects
                && !saved.show_enchant_effects
                && !saved.show_equipment_effects
        );
        let mut stale = other.clone();
        stale.scale_percent = 120.;
        stale.save_merged(&other, &p).unwrap();
        let saved = Settings::load(&p).unwrap();
        assert_eq!(saved.connections.len(), 2);
        assert_eq!(saved.scale_percent, 120.);
        assert!(
            !saved.show_spell_effects
                && !saved.show_enchant_effects
                && !saved.show_equipment_effects
        );
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn local_helm_override_defaults_to_server_and_merges_without_resetting_other_preferences() {
        let d = dir();
        let p = d.join("settings.json");
        std::fs::write(
            &p,
            r#"{"status_url":"http://localhost:18112/status","show_spell_effects":false}"#,
        )
        .unwrap();
        let base = Settings::load(&p).unwrap();
        assert_eq!(base.show_helm, None);
        let mut other = base.clone();
        other
            .add_connection("Iris".into(), "http://localhost:20001/status".into())
            .unwrap();
        other.scale_percent = 175.;
        other.save_merged(&base, &p).unwrap();
        let mut local = base.clone();
        local.show_helm = Some(false);
        local.save_merged(&base, &p).unwrap();
        let saved = Settings::load(&p).unwrap();
        assert_eq!(saved.show_helm, Some(false));
        assert!(!saved.show_spell_effects);
        assert_eq!(saved.scale_percent, 175.);
        assert_eq!(saved.connections.len(), 2);
        let mut follow = saved.clone();
        follow.show_helm = None;
        follow.save_merged(&saved, &p).unwrap();
        assert_eq!(Settings::load(&p).unwrap().show_helm, None);
        std::fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn slot_overrides_merge_independently_and_restore_default_without_hiding_jewelry() {
        let d = dir();
        let p = d.join("settings.json");
        let base = Settings::default();
        base.save(&p).unwrap();
        let mut first = base.clone();
        first.equipment_visibility.insert(2, false);
        first.save_merged(&base, &p).unwrap();
        let mut other = base.clone();
        other.equipment_visibility.insert(14, true);
        other.save_merged(&base, &p).unwrap();
        let saved = Settings::load(&p).unwrap();
        assert_eq!(saved.equipment_visibility.get(&2), Some(&false));
        assert_eq!(saved.equipment_visibility.get(&14), Some(&true));
        assert_eq!(saved.show_helm, None);
        let mut follow = saved.clone();
        follow.equipment_visibility.clear();
        follow.save_merged(&saved, &p).unwrap();
        assert!(Settings::load(&p).unwrap().equipment_visibility.is_empty());
        let mut invalid = base;
        invalid.equipment_visibility.insert(10, false);
        assert!(invalid.validate().is_err());
        assert!(!VISIBLE_EQUIPMENT_SLOTS
            .iter()
            .any(|(s, _)| [1, 10, 11, 12, 13].contains(s)));
        std::fs::remove_dir_all(d).unwrap();
    }
}
