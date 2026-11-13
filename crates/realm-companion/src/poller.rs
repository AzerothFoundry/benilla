use bevy::prelude::*;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Resource)]
pub(crate) struct Poller {
    pub(crate) url: Arc<Mutex<(u64, String)>>,
    pub(crate) latest: Arc<Mutex<Option<(u64, Result<Value, String>)>>>,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
#[derive(Clone, Copy)]
struct Timing {
    poll: Duration,
    sse_retry: Duration,
    calibration_interval: Duration,
    connect_timeout: Duration,
    read_timeout: Duration,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(1),
            sse_retry: Duration::from_secs(30),
            calibration_interval: Duration::from_secs(300),
            connect_timeout: Duration::from_secs(4),
            read_timeout: Duration::from_secs(5),
        }
    }
}
impl Poller {
    pub fn new(url: String) -> Self {
        Self::with_timing(url, Timing::default())
    }
    fn with_timing(url: String, timing: Timing) -> Self {
        let configured = Arc::new(Mutex::new((0, url)));
        let latest = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let source = configured.clone();
        let dest = latest.clone();
        let shutdown = stop.clone();
        let worker = std::thread::Builder::new()
            .name("companion-status".into())
            .spawn(move || {
                let client = ureq::AgentBuilder::new()
                    .timeout_connect(timing.connect_timeout)
                    .timeout_read(timing.read_timeout)
                    .timeout_write(timing.connect_timeout)
                    .build();
                let mut previous_generation = None;
                let mut retry_at = Instant::now();
                while !shutdown.load(Ordering::Relaxed) {
                    let (generation, url) = source.lock().unwrap().clone();
                    if previous_generation != Some(generation) {
                        previous_generation = Some(generation);
                        retry_at = Instant::now();
                    }
                    let mut result = fetch_status(&client, &url);
                    let stream = result
                        .as_ref()
                        .ok()
                        .and_then(|v| v["companion_events_url"].as_str())
                        .and_then(|p| url::Url::parse(&url).ok()?.join(p).ok());
                    if let Ok(data) = &mut result {
                        data["_transport"] = "poll".into();
                    }
                    let baseline = result.as_ref().ok().cloned();
                    publish(&source, &dest, &shutdown, generation, result);
                    if let Some(stream) = stream.filter(|_| Instant::now() >= retry_at) {
                        let result = read_events(
                            &client,
                            stream.as_str(),
                            generation,
                            &source,
                            &dest,
                            &shutdown,
                            &url,
                            timing.calibration_interval,
                            baseline,
                        );
                        if shutdown.load(Ordering::Relaxed) {
                            break;
                        }
                        if source.lock().unwrap().0 != generation {
                            continue;
                        }
                        // SSE is an optimization, not the source of character availability. A
                        // dropped/invalid stream immediately falls back to a fresh HTTP snapshot.
                        if result.is_err() {
                            retry_at = Instant::now() + timing.sse_retry;
                            let mut fallback = fetch_status(&client, &url);
                            if let Ok(data) = &mut fallback {
                                data["_transport"] = "poll".into();
                            }
                            publish(&source, &dest, &shutdown, generation, fallback);
                        }
                    }
                    std::thread::park_timeout(timing.poll);
                }
            })
            .expect("status client thread");
        Self {
            url: configured,
            latest,
            stop,
            worker: Some(worker),
        }
    }
    pub fn set_url(&self, url: String) {
        {
            let mut g = self.url.lock().unwrap();
            g.0 += 1;
            g.1 = url;
        }
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }
}
impl Drop for Poller {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }
}
fn publish(
    source: &Mutex<(u64, String)>,
    dest: &Mutex<Option<(u64, Result<Value, String>)>>,
    stop: &AtomicBool,
    generation: u64,
    result: Result<Value, String>,
) {
    let configured = source.lock().unwrap();
    if configured.0 == generation && !stop.load(Ordering::Relaxed) {
        *dest.lock().unwrap() = Some((generation, result));
    }
}
fn fetch_status(client: &ureq::Agent, url: &str) -> Result<Value, String> {
    fetch_status_with_deadline(client, url, Duration::from_secs(6))
}
fn fetch_status_with_deadline(
    client: &ureq::Agent,
    url: &str,
    deadline: Duration,
) -> Result<Value, String> {
    if url.trim().is_empty() {
        return Err("请选择连接".into());
    }
    let response = client
        .get(url)
        .timeout(deadline)
        .set("Accept", "application/json")
        .call()
        .map_err(|e| format!("连接失败：{e}"))?;
    if response.header("Content-Type").is_some_and(|h| {
        h.split(';')
            .next()
            .unwrap_or("")
            .trim()
            .eq_ignore_ascii_case("text/event-stream")
    }) {
        return Err("此地址返回 SSE 事件流，请填写 status JSON 地址（例如 /status?view=companion），不要填写 /companion/events".into());
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1024 * 1024 {
        return Err("响应超过 1 MiB，请使用 ?view=companion".into());
    }
    decode(&bytes)
}
fn decode(bytes: &[u8]) -> Result<Value, String> {
    let data: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("状态不是有效 JSON：{e}"))?;
    if data.get("connected").and_then(Value::as_bool).is_none() || data.get("animation").is_none() {
        return Err("接口缺少桌宠状态，请更新 realm-agent".into());
    }
    Ok(data)
}
fn read_events(
    client: &ureq::Agent,
    url: &str,
    generation: u64,
    source: &Mutex<(u64, String)>,
    dest: &Mutex<Option<(u64, Result<Value, String>)>>,
    stop: &AtomicBool,
    status_url: &str,
    calibration_interval: Duration,
    mut last_state: Option<Value>,
) -> Result<(), String> {
    let response = client
        .get(url)
        .set("Accept", "text/event-stream")
        .call()
        .map_err(|e| e.to_string())?;
    if !response
        .header("Content-Type")
        .is_some_and(|h| h.starts_with("text/event-stream"))
    {
        return Err("接口不是 SSE 事件流".into());
    }
    let mut reader = BufReader::new(response.into_reader());
    let mut parser = SseData::default();
    let mut next_calibration = Instant::now() + calibration_interval;
    loop {
        if stop.load(Ordering::Relaxed) || source.lock().unwrap().0 != generation {
            return Ok(());
        }
        let mut line = String::new();
        let n = reader
            .by_ref()
            .take(256 * 1024 + 1)
            .read_line(&mut line)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("连接已关闭".into());
        }
        if n > 256 * 1024 {
            return Err("SSE 消息过大".into());
        }
        if let Some(data) = parser.line(&line)? {
            let mut data = decode(data.as_bytes())?;
            if last_state
                .as_ref()
                .is_none_or(|previous| !older_snapshot(previous, &data))
            {
                next_calibration = Instant::now() + calibration_interval;
                let changed = last_state.as_ref().is_none_or(|previous| {
                    previous["_transport"] != "sse" || state_key(previous) != state_key(&data)
                });
                data["_transport"] = Value::String("sse".into());
                last_state = Some(data.clone());
                if changed {
                    publish(source, dest, stop, generation, Ok(data));
                }
            }
        }
        // A comment proves channel liveness, not current character state. Keep
        // this connection open while periodically checking the HTTP snapshot.
        if Instant::now() >= next_calibration {
            if let Ok(mut current) = fetch_status(client, status_url) {
                if last_state
                    .as_ref()
                    .is_none_or(|previous| !older_snapshot(previous, &current))
                {
                    let changed = last_state
                        .as_ref()
                        .is_none_or(|previous| state_key(previous) != state_key(&current));
                    current["_transport"] = "sse".into();
                    last_state = Some(current.clone());
                    // An identical calibration must not interrupt local dancing.
                    if changed {
                        publish(source, dest, stop, generation, Ok(current));
                    }
                }
            }
            // A failed check does not make a healthy SSE stream an offline actor.
            next_calibration = Instant::now() + calibration_interval;
        }
    }
}
fn state_key(frame: &Value) -> Value {
    let mut animation = frame["animation"].clone();
    if let Some(fields) = animation.as_object_mut() {
        fields.remove("dancing");
        let casting = fields
            .remove("cast_remaining_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            > 0;
        fields.insert("casting".into(), casting.into());
    }
    serde_json::json!([
        frame["connected"],
        frame["character"],
        frame["class"],
        frame["appearance"],
        animation,
        frame["visual_session"],
        frame["visual_sequence"]
    ])
}
fn older_snapshot(previous: &Value, current: &Value) -> bool {
    let Some(session) = previous["visual_session"].as_str() else {
        return false;
    };
    if current["visual_session"].as_str() != Some(session) {
        return false;
    }
    previous["now_ms"]
        .as_u64()
        .zip(current["now_ms"].as_u64())
        .is_some_and(|(p, c)| c < p)
        && previous["visual_sequence"]
            .as_u64()
            .zip(current["visual_sequence"].as_u64())
            .is_none_or(|(p, c)| c <= p)
}
#[derive(Default)]
struct SseData {
    data: String,
    event: String,
}
impl SseData {
    fn line(&mut self, line: &str) -> Result<Option<String>, String> {
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            let event = std::mem::take(&mut self.event);
            let data = std::mem::take(&mut self.data);
            return Ok((!data.is_empty()
                && matches!(event.as_str(), "" | "message" | "snapshot" | "state"))
            .then_some(data));
        }
        if let Some(value) = line.strip_prefix("event:") {
            self.event = value.trim().into();
        }
        if let Some(value) = line.strip_prefix("data:") {
            if !self.data.is_empty() {
                self.data.push('\n');
            }
            self.data.push_str(value.strip_prefix(' ').unwrap_or(value));
            if self.data.len() > 1024 * 1024 {
                return Err("SSE 数据过大".into());
            }
        }
        Ok(None)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sse_comments_do_not_replace_state_and_multiline_json_is_complete() {
        let mut p = SseData::default();
        assert_eq!(p.line(": keepalive\n").unwrap(), None);
        assert_eq!(p.line("\n").unwrap(), None);
        p.line("event: state\r\n").unwrap();
        p.line("data: {\"connected\":true,\n").unwrap();
        p.line("data: \"animation\":{}}\n").unwrap();
        let value = decode(p.line("\n").unwrap().unwrap().as_bytes()).unwrap();
        assert_eq!(value["connected"], true);
        assert!(p.line("\n").unwrap().is_none());
    }
    #[test]
    fn calibration_ignores_clock_only_changes_but_updates_class_and_rejects_old_same_session_frames(
    ) {
        let previous = serde_json::json!({"connected":true,"character":"Ada","class":8,"animation":{"moving":false},"now_ms":200,"visual_session":"A","visual_sequence":10});
        let mut current = previous.clone();
        current["now_ms"] = 250.into();
        assert_eq!(state_key(&previous), state_key(&current));
        current["class"] = 1.into();
        assert_ne!(state_key(&previous), state_key(&current));
        current["now_ms"] = 100.into();
        assert!(older_snapshot(&previous, &current));
        current["visual_sequence"] = 11.into();
        assert!(!older_snapshot(&previous, &current));
        current["visual_session"] = "B".into();
        current["now_ms"] = 0.into();
        current["visual_sequence"] = 0.into();
        assert!(!older_snapshot(&previous, &current));
    }
    #[test]
    fn empty_address_is_an_idle_selection_prompt_without_request() {
        assert_eq!(
            fetch_status(&ureq::AgentBuilder::new().build(), "").unwrap_err(),
            "请选择连接"
        );
    }
    #[test]
    fn status_and_stream_urls_preserve_proxy_prefix() {
        let base = url::Url::parse("http://127.0.0.1:20001/ada/status?view=companion").unwrap();
        assert_eq!(
            base.join("companion/events").unwrap().as_str(),
            "http://127.0.0.1:20001/ada/companion/events"
        );
        assert!(decode(b"{}").is_err());
        assert!(decode(b"not json").is_err());
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;
    use std::{
        io::Write,
        net::{TcpListener, TcpStream},
        sync::atomic::AtomicUsize,
    };
    struct Server {
        address: std::net::SocketAddr,
        stop: Arc<AtomicBool>,
        status_down: Arc<AtomicBool>,
        mode: Arc<AtomicUsize>,
        polls: Arc<AtomicUsize>,
        streams: Arc<AtomicUsize>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Server {
        fn new(mode: usize) -> Self {
            Self::at("127.0.0.1:0".parse().unwrap(), mode)
        }
        fn at(address: std::net::SocketAddr, mode: usize) -> Self {
            let listener = TcpListener::bind(address).unwrap();
            let address = listener.local_addr().unwrap();
            listener.set_nonblocking(true).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let down = Arc::new(AtomicBool::new(false));
            let mode = Arc::new(AtomicUsize::new(mode));
            let polls = Arc::new(AtomicUsize::new(0));
            let streams = Arc::new(AtomicUsize::new(0));
            let (halt, d, m, p, s) = (
                stop.clone(),
                down.clone(),
                mode.clone(),
                polls.clone(),
                streams.clone(),
            );
            let thread = std::thread::spawn(move || {
                while !halt.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let (h, d, m, p, s) =
                                (halt.clone(), d.clone(), m.clone(), p.clone(), s.clone());
                            std::thread::spawn(move || serve(stream, h, d, m, p, s));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(2))
                        }
                        Err(_) => break,
                    }
                }
            });
            Self {
                address,
                stop,
                status_down: down,
                mode,
                polls,
                streams,
                thread: Some(thread),
            }
        }
        fn url(&self) -> String {
            format!("http://{}/status", self.address)
        }
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Relaxed);
            self.thread.take().unwrap().join().unwrap();
        }
    }
    fn serve(
        mut stream: TcpStream,
        stop: Arc<AtomicBool>,
        down: Arc<AtomicBool>,
        mode: Arc<AtomicUsize>,
        polls: Arc<AtomicUsize>,
        streams: Arc<AtomicUsize>,
    ) {
        stream
            .set_read_timeout(Some(Duration::from_millis(200)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() {
            return;
        }
        let sse = line.contains("/companion/events");
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                break;
            }
        }
        let response = |stream: &mut TcpStream, code: usize, body: &str| {
            let _=write!(stream,"HTTP/1.1 {code} TEST\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
        };
        if !sse {
            let n = polls.fetch_add(1, Ordering::Relaxed);
            if down.load(Ordering::Relaxed) {
                response(&mut stream, 503, "{}");
                return;
            }
            let mode = mode.load(Ordering::Relaxed);
            if mode == 14 && n > 0 {
                std::thread::sleep(Duration::from_millis(120));
            }
            let moving = if mode == 11 {
                n > 0
            } else if mode >= 10 {
                false
            } else {
                n % 2 == 0
            };
            response(&mut stream,200,&serde_json::json!({"connected":true,"character":"Ada","animation":{"moving":moving},"now_ms":n,"companion_events_url":"companion/events"}).to_string());
            return;
        }
        streams.fetch_add(1, Ordering::Relaxed);
        match mode.load(Ordering::Relaxed) {
            404 | 503 => response(&mut stream, mode.load(Ordering::Relaxed), "{}"),
            1 => {
                let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: state\ndata: {{\"connected\":true,\"character\":\"Ada\",\"animation\":{{}}}}\n\n");
            }
            2 => {
                let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: invalid JSON\n\n");
            }
            3 => {
                let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n");
                while !stop.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
            mode @ 10..=14 => {
                let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: snapshot\ndata: {{\"connected\":true,\"character\":\"Ada\",\"animation\":{{\"moving\":false}}}}\n\n");
                let mut tick = 0;
                while !stop.load(Ordering::Relaxed) {
                    tick += 1;
                    let text = if mode == 12 {
                        format!("event: state\ndata: {{\"connected\":true,\"character\":\"Ada\",\"animation\":{{\"moving\":false}},\"now_ms\":{tick}}}\n\n")
                    } else if mode == 13 {
                        format!("event: heartbeat\ndata: {{\"tick\":{tick}}}\n\n")
                    } else {
                        ": keepalive\n\n".into()
                    };
                    if stream.write_all(text.as_bytes()).is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
            }
            _ => {
                let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: snapshot\ndata: {{\"connected\":true,\"character\":\"Ada\",\"animation\":{{}}}}\n\n");
                while !stop.load(Ordering::Relaxed) {
                    if write!(stream, ": keepalive\n\n").is_err() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(15));
                }
            }
        }
    }
    fn timing() -> Timing {
        Timing {
            poll: Duration::from_millis(30),
            sse_retry: Duration::from_millis(400),
            calibration_interval: Duration::from_secs(300),
            connect_timeout: Duration::from_millis(80),
            read_timeout: Duration::from_millis(80),
        }
    }
    fn wait(mut f: impl FnMut() -> bool) {
        let end = Instant::now() + Duration::from_secs(3);
        while !f() {
            assert!(Instant::now() < end, "condition timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn shutdown(mut p: Poller) {
        p.stop.store(true, Ordering::Relaxed);
        let worker = p.worker.take().unwrap();
        worker.thread().unpark();
        worker.join().unwrap();
    }
    #[test]
    fn unavailable_sse_preserves_changing_http_snapshots_and_backs_off() {
        for mode in [404, 503, 2, 3] {
            let s = Server::new(mode);
            let p = Poller::with_timing(s.url(), timing());
            wait(|| s.polls.load(Ordering::Relaxed) >= 5);
            assert_eq!(
                s.streams.load(Ordering::Relaxed),
                1,
                "failed SSE should not be retried on each HTTP poll"
            );
            let latest = p.latest.lock().unwrap().clone().unwrap().1.unwrap();
            assert_eq!(latest["connected"], true);
            assert_eq!(latest["_transport"], "poll");
            assert!(latest["now_ms"].as_u64().unwrap() >= 3);
            shutdown(p);
        }
    }
    #[test]
    fn abrupt_sse_close_immediately_fetches_status_then_recovers_sse_after_backoff() {
        let s = Server::new(1);
        let p = Poller::with_timing(s.url(), timing());
        wait(|| s.polls.load(Ordering::Relaxed) >= 3);
        assert_eq!(s.streams.load(Ordering::Relaxed), 1);
        assert_eq!(
            p.latest
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .1
                .as_ref()
                .unwrap()["_transport"],
            "poll"
        );
        s.mode.store(0, Ordering::Relaxed);
        wait(|| {
            p.latest
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|(_, v)| v.as_ref().is_ok_and(|v| v["_transport"] == "sse"))
        });
        shutdown(p);
    }
    #[test]
    fn real_http_failure_reports_offline_error_then_retries_and_recovers() {
        let s = Server::new(404);
        s.status_down.store(true, Ordering::Relaxed);
        let p = Poller::with_timing(s.url(), timing());
        wait(|| {
            p.latest
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|(_, v)| v.is_err())
        });
        s.status_down.store(false, Ordering::Relaxed);
        wait(|| {
            p.latest
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|(_, v)| v.as_ref().is_ok_and(|v| v["connected"] == true))
        });
        shutdown(p);
    }
    #[test]
    fn url_change_resets_sse_backoff_and_old_generations_cannot_publish() {
        let first = Server::new(404);
        let second = Server::new(0);
        let p = Poller::with_timing(
            first.url(),
            Timing {
                sse_retry: Duration::from_secs(30),
                calibration_interval: Duration::from_secs(300),
                ..timing()
            },
        );
        wait(|| first.streams.load(Ordering::Relaxed) == 1);
        p.set_url(second.url());
        wait(|| {
            p.latest
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|(g, v)| *g == 1 && v.as_ref().is_ok_and(|v| v["_transport"] == "sse"))
        });
        publish(&p.url, &p.latest, &p.stop, 0, Err("old request".into()));
        assert_eq!(p.latest.lock().unwrap().as_ref().unwrap().0, 1);
        shutdown(p);
    }
    #[test]
    fn refused_http_connection_recovers_when_same_endpoint_starts() {
        let reserved = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = reserved.local_addr().unwrap();
        drop(reserved);
        let p = Poller::with_timing(format!("http://{address}/status"), timing());
        wait(|| {
            p.latest
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|(_, v)| v.is_err())
        });
        let _server = Server::at(address, 404);
        wait(|| {
            p.latest
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|(_, v)| v.as_ref().is_ok_and(|v| v["connected"] == true))
        });
        shutdown(p);
    }
    #[test]
    fn accidentally_configured_sse_url_is_rejected_before_reading_endless_body() {
        let server = Server::new(0);
        let client = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(2))
            .build();
        let start = Instant::now();
        let error = fetch_status(
            &client,
            &format!("http://{}/companion/events", server.address),
        )
        .unwrap_err();
        assert!(
            error.contains("status JSON"),
            "unexpected response: {error}"
        );
        assert!(error.contains("/companion/events"));
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "SSE headers must fail immediately, never wait for EOF"
        );
    }
    #[test]
    fn slow_drip_json_body_has_overall_deadline_not_only_per_read_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut connection, _) = listener.accept().unwrap();
            connection
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let mut request = BufReader::new(connection.try_clone().unwrap());
            loop {
                let mut line = String::new();
                if request.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
            }
            write!(
                connection,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            for _ in 0..100 {
                if write!(connection, " ").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        });
        let client = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(2))
            .build();
        let start = Instant::now();
        assert!(fetch_status_with_deadline(
            &client,
            &format!("http://{address}/status"),
            Duration::from_millis(90)
        )
        .is_err());
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "a steady trickle must not evade the whole-response deadline"
        );
        server.join().unwrap();
    }
    fn calibration_timing() -> Timing {
        Timing {
            calibration_interval: Duration::from_millis(70),
            ..timing()
        }
    }
    fn sse_ready(p: &Poller) -> bool {
        p.latest
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|(_, r)| r.as_ref().is_ok_and(|d| d["_transport"] == "sse"))
    }
    #[test]
    fn heartbeat_only_stream_calibrates_http_without_reconnecting_or_republishing_identical_state()
    {
        for mode in [10, 13] {
            let s = Server::new(mode);
            let p = Poller::with_timing(s.url(), calibration_timing());
            wait(|| sse_ready(&p));
            p.latest.lock().unwrap().take();
            wait(|| s.polls.load(Ordering::Relaxed) >= 3);
            assert_eq!(s.streams.load(Ordering::Relaxed), 1);
            assert!(
                p.latest.lock().unwrap().is_none(),
                "clock-only HTTP calibration must not cancel local dancing"
            );
            shutdown(p);
        }
    }
    #[test]
    fn missing_state_edge_is_corrected_by_calibration_while_sse_stays_live() {
        let s = Server::new(11);
        let p = Poller::with_timing(s.url(), calibration_timing());
        wait(|| {
            p.latest.lock().unwrap().as_ref().is_some_and(|(_, r)| {
                r.as_ref()
                    .is_ok_and(|d| d["_transport"] == "sse" && d["animation"]["moving"] == true)
            })
        });
        assert_eq!(s.streams.load(Ordering::Relaxed), 1);
        assert!(s.polls.load(Ordering::Relaxed) >= 2);
        shutdown(p);
    }
    #[test]
    fn failed_calibration_does_not_report_offline_for_a_healthy_stream() {
        let s = Server::new(10);
        let p = Poller::with_timing(s.url(), calibration_timing());
        wait(|| sse_ready(&p));
        p.latest.lock().unwrap().take();
        s.status_down.store(true, Ordering::Relaxed);
        wait(|| s.polls.load(Ordering::Relaxed) >= 3);
        assert!(p.latest.lock().unwrap().is_none());
        assert_eq!(s.streams.load(Ordering::Relaxed), 1);
        shutdown(p);
    }
    #[test]
    fn real_state_events_reset_the_five_minute_watchdog() {
        let s = Server::new(12);
        let p = Poller::with_timing(s.url(), calibration_timing());
        wait(|| sse_ready(&p));
        p.latest.lock().unwrap().take();
        std::thread::sleep(Duration::from_millis(220));
        assert!(
            p.latest.lock().unwrap().is_none(),
            "duplicate clock-only SSE frames must not cancel local dancing"
        );
        assert_eq!(s.polls.load(Ordering::Relaxed), 1);
        shutdown(p);
    }
    #[test]
    fn changing_url_during_calibration_discards_old_generation() {
        let old = Server::new(14);
        let next = Server::new(0);
        let p = Poller::with_timing(old.url(), calibration_timing());
        wait(|| old.polls.load(Ordering::Relaxed) >= 2);
        p.set_url(next.url());
        wait(|| {
            p.latest
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|(generation, r)| {
                    *generation == 1 && r.as_ref().is_ok_and(|d| d["_transport"] == "sse")
                })
        });
        assert_eq!(p.latest.lock().unwrap().as_ref().unwrap().0, 1);
        shutdown(p);
    }
}
