//! Read role names without entering a role or blocking the UI.
use crate::{
    settings::{normalize_status_url, Settings},
    UiState,
};
use bevy::prelude::*;
use std::{
    collections::HashMap,
    io::Read,
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(crate) struct RoleProfile {
    pub race: Option<u8>,
    pub class_id: Option<u8>,
    pub level: Option<u32>,
    pub location: Option<String>,
    #[serde(skip)]
    pub connected: Option<bool>,
    #[serde(skip)]
    position: Option<ProfilePosition>,
}
#[derive(Clone, serde::Deserialize)]
struct ProfilePosition {
    map: u32,
    x: f32,
    y: f32,
}
impl RoleProfile {
    pub fn description(&self) -> String {
        let class = match self.class_id {
            Some(1) => "战士",
            Some(2) => "圣骑士",
            Some(3) => "猎人",
            Some(4) => "盗贼",
            Some(5) => "牧师",
            Some(7) => "萨满祭司",
            Some(8) => "法师",
            Some(9) => "术士",
            Some(11) => "德鲁伊",
            _ => "职业未知",
        };
        match self.level {
            Some(level) => format!("等级{level} {class}"),
            None => format!("等级未知 {class}"),
        }
    }
}
fn profile_path(path: &std::path::Path, id: &str, url: &str) -> std::path::PathBuf {
    path.parent()
        .unwrap_or(std::path::Path::new("."))
        .join("profiles")
        .join(format!("{}.json", crate::settings::cache_identity(id, url)))
}
struct AreaLookup {
    areas: benilla_formats::AreaTableCatalog,
    maps: benilla_formats::WorldMapAreaCatalog,
}
impl AreaLookup {
    fn load(path: &str) -> Option<Self> {
        let mut chain = benilla_formats::Chain::open(std::path::Path::new(path)).ok()?;
        Some(Self {
            areas: benilla_formats::load_area_table_catalog(&mut chain).ok()?,
            maps: benilla_formats::load_world_map_area_catalog(&mut chain).ok()?,
        })
    }
    fn resolve(&self, p: &ProfilePosition) -> Option<String> {
        self.maps
            .iter()
            .filter(|(_, a)| {
                a.map_id == p.map
                    && a.area_id != 0
                    && p.x >= a.loc_top.min(a.loc_bottom)
                    && p.x <= a.loc_top.max(a.loc_bottom)
                    && p.y >= a.loc_left.min(a.loc_right)
                    && p.y <= a.loc_left.max(a.loc_right)
            })
            .min_by(|(_, a), (_, b)| {
                ((a.loc_left - a.loc_right) * (a.loc_top - a.loc_bottom))
                    .abs()
                    .total_cmp(&((b.loc_left - b.loc_right) * (b.loc_top - b.loc_bottom)).abs())
            })
            .and_then(|(_, a)| self.areas.name(a.area_id))
            .map(str::to_owned)
    }
}
struct Request {
    id: String,
    url: String,
    data_dir: String,
}
struct Answer {
    id: String,
    url: String,
    name: Option<String>,
    profile: Option<RoleProfile>,
    probe: bool,
    error: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConnectionTest {
    Testing,
    Passed {
        character: String,
        online: Option<bool>,
    },
    Failed(String),
}
#[derive(Resource)]
pub(crate) struct NameResolver {
    send: mpsc::Sender<Request>,
    probe_answers: mpsc::Sender<Answer>,
    tests: HashMap<(String, String), ConnectionTest>,
    answers: Mutex<mpsc::Receiver<Answer>>,
    requested: HashMap<(String, String), Instant>,
    pub races: HashMap<(String, String), u8>,
    pub profiles: HashMap<(String, String), RoleProfile>,
    data_dir: String,
}
impl Default for NameResolver {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel::<Request>();
        let (out, answers) = mpsc::channel();
        let rx = Arc::new(Mutex::new(rx));
        for index in 0..2 {
            let rx = Arc::clone(&rx);
            let out = out.clone();
            std::thread::Builder::new()
                .name(format!("companion-names-{index}"))
                .spawn(move || {
                    let client = ureq::AgentBuilder::new()
                        .timeout(Duration::from_secs(4))
                        .build();
                    let mut catalog: Option<(String, Option<AreaLookup>)> = None;
                    loop {
                        let request = match rx.lock().unwrap().recv() {
                            Ok(r) => r,
                            Err(_) => break,
                        };
                        if catalog
                            .as_ref()
                            .is_none_or(|(path, _)| *path != request.data_dir)
                        {
                            catalog = Some((
                                request.data_dir.clone(),
                                AreaLookup::load(&request.data_dir),
                            ));
                        }
                        let profile =
                            fetch_profile(&client, &request.url)
                                .ok()
                                .map(|(name, mut profile)| {
                                    if profile.location.is_none() {
                                        profile.location = catalog
                                            .as_ref()
                                            .and_then(|(_, c)| c.as_ref())
                                            .and_then(|c| {
                                                profile.position.as_ref().and_then(|p| c.resolve(p))
                                            });
                                    }
                                    (name, profile)
                                });
                        let name = profile.as_ref().map(|p| p.0.clone());
                        let profile = profile.map(|p| p.1);
                        if out
                            .send(Answer {
                                id: request.id,
                                url: request.url,
                                name,
                                profile,
                                probe: false,
                                error: None,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                })
                .expect("name resolver");
        }
        Self {
            send: tx,
            probe_answers: out,
            tests: HashMap::new(),
            answers: Mutex::new(answers),
            requested: HashMap::new(),
            races: HashMap::new(),
            profiles: HashMap::new(),
            data_dir: String::new(),
        }
    }
}
impl NameResolver {
    fn test_key(id: &str, url: &str) -> (String, String) {
        (
            id.into(),
            normalize_status_url(url).unwrap_or_else(|_| url.into()),
        )
    }
    pub fn test_status(&self, id: &str, url: &str) -> Option<&ConnectionTest> {
        self.tests.get(&Self::test_key(id, url))
    }
    pub fn test_connection(&mut self, id: &str, url: &str) {
        let key = Self::test_key(id, url);
        if matches!(self.tests.get(&key), Some(ConnectionTest::Testing)) {
            return;
        }
        let url = match normalize_status_url(url) {
            Ok(url) => url,
            Err(e) => {
                self.tests.insert(key, ConnectionTest::Failed(e));
                return;
            }
        };
        self.tests.insert(key.clone(), ConnectionTest::Testing);
        let send = self.probe_answers.clone();
        let id = id.to_owned();
        // User tests bypass the background metadata queue and its30second cache.
        let started = std::thread::Builder::new()
            .name("companion-connection-test".into())
            .spawn(move || {
                let client = ureq::AgentBuilder::new()
                    .timeout(Duration::from_secs(4))
                    .build();
                let (name, profile, error) = match fetch_profile(&client, &url) {
                    Ok((name, profile)) => (Some(name), Some(profile), None),
                    Err(error) => (None, None, Some(error)),
                };
                let _ = send.send(Answer {
                    id,
                    url,
                    name,
                    profile,
                    probe: true,
                    error,
                });
            });
        if let Err(error) = started {
            self.tests
                .insert(key, ConnectionTest::Failed(error.to_string()));
        }
    }
    fn record_probe(&mut self, answer: &Answer) {
        if !answer.probe {
            return;
        }
        let status = if let Some(error) = &answer.error {
            ConnectionTest::Failed(error.clone())
        } else {
            ConnectionTest::Passed {
                character: answer.name.clone().unwrap_or_default(),
                online: answer.profile.as_ref().and_then(|p| p.connected),
            }
        };
        self.tests
            .insert((answer.id.clone(), answer.url.clone()), status);
    }
    pub fn set_data_dir(&mut self, path: &str) {
        if self.data_dir != path {
            self.data_dir = path.into();
            self.requested.clear();
        }
    }
    pub fn profile(&mut self, path: &std::path::Path, id: &str, url: &str) -> &RoleProfile {
        self.profiles
            .entry((id.into(), url.into()))
            .or_insert_with(|| {
                std::fs::read(profile_path(path, id, url))
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .unwrap_or_default()
            })
    }
    pub fn request(&mut self, id: &str, url: &str) {
        let Ok(url) = normalize_status_url(url) else {
            return;
        };
        let key = (id.to_owned(), url.clone());
        if self
            .requested
            .get(&key)
            .is_some_and(|at| at.elapsed() < Duration::from_secs(30))
        {
            return;
        }
        self.requested.insert(key, Instant::now());
        let _ = self.send.send(Request {
            id: id.into(),
            url,
            data_dir: self.data_dir.clone(),
        });
    }
}
#[cfg(test)]
fn fetch_name(client: &ureq::Agent, url: &str) -> Result<String, String> {
    fetch_profile(client, url).map(|p| p.0)
}
fn fetch_profile(client: &ureq::Agent, url: &str) -> Result<(String, RoleProfile), String> {
    let mut target = url::Url::parse(url).map_err(|e| e.to_string())?;
    let pairs: Vec<_> = target
        .query_pairs()
        .filter(|(k, v)| !(k == "view" && v == "companion"))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    target.set_query(None);
    if !pairs.is_empty() {
        target.query_pairs_mut().extend_pairs(pairs);
    }
    let response = client
        .get(target.as_str())
        .call()
        .map_err(|e| e.to_string())?;
    if response
        .header("Content-Type")
        .is_some_and(|s| s.starts_with("text/event-stream"))
    {
        return Err("请填写状态接口地址，当前地址返回的是事件流".into());
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("状态响应过大".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let name = value["character"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_owned())
        .ok_or_else(|| "接口未返回角色信息".to_owned())?;
    let race = value["appearance"]["race"]
        .as_u64()
        .or_else(|| value["race"].as_u64())
        .and_then(|r| u8::try_from(r).ok())
        .filter(|r| benilla_app::companion_renderer::scene_token(*r).is_some());
    let me = value["objects"].as_array().and_then(|objects| {
        objects.iter().find(|o| {
            o["name"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(&name))
        })
    });
    let level = value["level"]
        .as_u64()
        .or_else(|| me.and_then(|o| o["level"].as_u64()))
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0);
    let class_id = value["class"].as_u64().and_then(|n| u8::try_from(n).ok());
    let position = serde_json::from_value::<ProfilePosition>(value["position"].clone()).ok();
    let location = value["zone_name"]
        .as_str()
        .or_else(|| value["area_name"].as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    Ok((
        name,
        RoleProfile {
            race,
            class_id,
            level,
            location,
            connected: value["connected"].as_bool(),
            position,
        },
    ))
}
pub(crate) fn apply_name(
    ui: &mut UiState,
    id: &str,
    url: &str,
    name: &str,
) -> Result<bool, String> {
    if name.trim().is_empty() {
        return Ok(false);
    }
    let matches = |s: &Settings| {
        s.connections
            .iter()
            .any(|c| c.id == id && c.status_url == url)
    };
    if !matches(&ui.settings) && !matches(&ui.draft) {
        return Ok(false);
    }
    let already = ui
        .settings
        .connections
        .iter()
        .filter(|c| c.id == id && c.status_url == url)
        .all(|c| c.name == name)
        && ui
            .draft
            .connections
            .iter()
            .filter(|c| c.id == id && c.status_url == url)
            .all(|c| c.name == name)
        && ui.cached_names.get(id).is_some_and(|known| known == name);
    if already {
        return Ok(false);
    }
    for list in [&mut ui.settings.connections, &mut ui.draft.connections] {
        if let Some(c) = list.iter_mut().find(|c| c.id == id && c.status_url == url) {
            c.name = name.into();
        }
    }
    ui.cached_names.insert(id.into(), name.into());
    let before = Settings::load(&ui.path)?;
    let mut after = before.clone();
    if let Some(c) = after
        .connections
        .iter_mut()
        .find(|c| c.id == id && c.status_url == url)
    {
        if c.name != name {
            c.name = name.into();
            after.save_merged(&before, &ui.path)?;
        }
    }
    Ok(true)
}
pub(crate) fn updates(
    mut resolver: ResMut<NameResolver>,
    mut ui: ResMut<UiState>,
    mut refresh: Local<Option<Instant>>,
) {
    if ui.open && refresh.is_none_or(|at| at.elapsed() > Duration::from_secs(30)) {
        for c in ui.draft.connections.clone() {
            resolver.request(&c.id, &c.status_url);
        }
        *refresh = Some(Instant::now());
    }
    let answers: Vec<_> = resolver.answers.lock().unwrap().try_iter().collect();
    for answer in answers {
        resolver.record_probe(&answer);
        if let Some(profile) = answer.profile {
            let key = (answer.id.clone(), answer.url.clone());
            let mut merged = resolver.profile(&ui.path, &answer.id, &answer.url).clone();
            if profile.race.is_some() {
                merged.race = profile.race;
            }
            if profile.class_id.is_some() {
                merged.class_id = profile.class_id;
            }
            if profile.level.is_some() {
                merged.level = profile.level;
            }
            if profile.location.is_some() {
                merged.location = profile.location;
            }
            if let Some(race) = merged.race {
                resolver.races.insert(key.clone(), race);
            }
            let path = profile_path(&ui.path, &answer.id, &answer.url);
            if let Err(e) = crate::settings::write_locked(&path, &merged) {
                warn!("role metadata cache: {e}");
            }
            resolver.profiles.insert(key, merged);
        }
        if let Some(name) = answer.name {
            if let Err(e) = apply_name(&mut ui, &answer.id, &answer.url, &name) {
                warn!("name persistence: {e}");
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn url_only_profile_name_is_filled_and_stale_url_answer_is_ignored() {
        let dir = std::env::temp_dir().join(format!(
            "companion-name-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("settings.json");
        let mut settings = Settings::default();
        let id = settings.selected_connection_id.clone().unwrap();
        settings.connections[0].name = String::new();
        settings.save(&path).unwrap();
        let url = settings.status_url.clone();
        let mut ui = UiState {
            settings: settings.clone(),
            draft: settings,
            path: path.clone(),
            ..Default::default()
        };
        assert!(apply_name(&mut ui, &id, &url, "Ada").unwrap());
        assert_eq!(Settings::load(&path).unwrap().connections[0].name, "Ada");
        assert!(!apply_name(&mut ui, &id, "http://wrong.example/status", "Wrong").unwrap());
        assert_eq!(ui.draft.connections[0].name, "Ada");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn real_status_response_supplies_name_without_animation_or_login() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            use std::io::Write;
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            stream.read(&mut request).unwrap();
            let body = r#"{"character":"Iris","connected":false}"#;
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        assert_eq!(
            fetch_name(&ureq::Agent::new(), &format!("http://{address}/status")).unwrap(),
            "Iris"
        );
        worker.join().unwrap();
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    #[test]
    fn status_profile_extracts_self_level_and_localized_region() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            use std::io::Write;
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 2048];
            let count = stream.read(&mut request).unwrap();
            assert!(!String::from_utf8_lossy(&request[..count]).contains("view=companion"));
            let body = r#"{"character":"Iris","class":8,"appearance":{"race":8},"objects":[{"name":"Iris","level":60}],"position":{"map":1,"x":1244.38,"y":-4399.30}}"#;
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        let (name, profile) = fetch_profile(
            &ureq::Agent::new(),
            &format!("http://{address}/status?view=companion"),
        )
        .unwrap();
        assert_eq!(name, "Iris");
        assert_eq!(profile.level, Some(60));
        assert_eq!(profile.race, Some(8));
        assert_eq!(profile.description(), "等级60 法师");
        if let Ok(data) = std::env::var("WOW_DATA") {
            let catalog = AreaLookup::load(&data).unwrap();
            assert!(catalog
                .resolve(profile.position.as_ref().unwrap())
                .is_some());
        }
        worker.join().unwrap();
    }
}

#[cfg(test)]
mod connection_tests {
    use super::*;
    fn receive(resolver: &mut NameResolver) -> Answer {
        let answer = resolver
            .answers
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        resolver.record_probe(&answer);
        answer
    }
    #[test]
    fn explicit_test_is_fresh_and_reports_offline_character_then_failure() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let url = format!("http://{address}/status");
        let worker = std::thread::spawn(move || {
            use std::io::Write;
            for (code, body) in [
                (200, r#"{"character":"Ada","connected":false}"#),
                (503, "unavailable"),
            ] {
                let (mut stream, _) = listener.accept().unwrap();
                let mut input = [0; 2048];
                stream.read(&mut input).unwrap();
                write!(stream,"HTTP/1.1 {code} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
            }
        });
        let mut resolver = NameResolver::default();
        resolver
            .requested
            .insert(("ada".into(), url.clone()), Instant::now());
        resolver.test_connection("ada", &url);
        assert_eq!(
            resolver.test_status("ada", &url),
            Some(&ConnectionTest::Testing)
        );
        let answer = receive(&mut resolver);
        assert_eq!(answer.name.as_deref(), Some("Ada"));
        assert_eq!(
            resolver.test_status("ada", &url),
            Some(&ConnectionTest::Passed {
                character: "Ada".into(),
                online: Some(false)
            })
        );
        resolver.test_connection("ada", &url);
        receive(&mut resolver);
        assert!(
            matches!(resolver.test_status("ada",&url),Some(ConnectionTest::Failed(error))if error.contains("503"))
        );
        assert!(resolver
            .test_status("ada", "http://localhost:1/status")
            .is_none());
        worker.join().unwrap();
    }
    #[test]
    fn invalid_url_and_non_role_response_do_not_pass() {
        let mut resolver = NameResolver::default();
        resolver.test_connection("ada", "not-a-url");
        assert!(matches!(
            resolver.test_status("ada", "not-a-url"),
            Some(ConnectionTest::Failed(_))
        ));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            use std::io::Write;
            let (mut stream, _) = listener.accept().unwrap();
            let mut input = [0; 2048];
            stream.read(&mut input).unwrap();
            let body = r#"{"ok":true}"#;
            write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        let url = format!("http://{address}/status");
        resolver.test_connection("ada", &url);
        receive(&mut resolver);
        assert!(matches!(
            resolver.test_status("ada", &url),
            Some(ConnectionTest::Failed(_))
        ));
        worker.join().unwrap();
    }
}
