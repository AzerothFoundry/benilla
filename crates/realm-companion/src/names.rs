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
struct Request {
    id: String,
    url: String,
}
struct Answer {
    id: String,
    url: String,
    name: Option<String>,
}
#[derive(Resource)]
pub(crate) struct NameResolver {
    send: mpsc::Sender<Request>,
    answers: Mutex<mpsc::Receiver<Answer>>,
    requested: HashMap<(String, String), Instant>,
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
                    loop {
                        let request = match rx.lock().unwrap().recv() {
                            Ok(r) => r,
                            Err(_) => break,
                        };
                        let name = fetch_name(&client, &request.url).ok();
                        if out
                            .send(Answer {
                                id: request.id,
                                url: request.url,
                                name,
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
            answers: Mutex::new(answers),
            requested: HashMap::new(),
        }
    }
}
impl NameResolver {
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
        let _ = self.send.send(Request { id: id.into(), url });
    }
}
fn fetch_name(client: &ureq::Agent, url: &str) -> Result<String, String> {
    let response = client.get(url).call().map_err(|e| e.to_string())?;
    if response
        .header("Content-Type")
        .is_some_and(|s| s.starts_with("text/event-stream"))
    {
        return Err("expected status JSON".into());
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("status too large".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    value["character"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.trim().to_owned())
        .ok_or("character not yet known".into())
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
