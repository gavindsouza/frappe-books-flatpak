use std::fs::{self, OpenOptions, rename};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{Manager, RunEvent, State, Url, WebviewWindowBuilder};

const SITE: &str = "site1";
const PORT: u16 = 8020;
const REDIS_CONFS: [(&str, u16); 2] = [
    ("config/redis_queue.conf", 11000),
    ("config/redis_cache.conf", 13000),
];
const READY_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const LOG_BACKUPS: usize = 5;

fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}

#[derive(Clone)]
struct Logger {
    path: PathBuf,
}

impl Logger {
    fn new(path: PathBuf) -> Self {
        let _ = rotate_log_if_needed(&path);
        Self { path }
    }

    fn log(&self, msg: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let line = format!("[{}.{:03}] {}", now.as_secs(), now.subsec_millis(), msg);
        println!("{line}");
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
    }
}

fn rotate_log_if_needed(path: &PathBuf) {
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() > MAX_LOG_SIZE {
            for i in (1..LOG_BACKUPS).rev() {
                let old = path.with_extension(format!("log.{}", i));
                let new = path.with_extension(format!("log.{}", i + 1));
                let _ = rename(&old, &new);
            }
            let backup = path.with_extension("log.1");
            let _ = rename(path, &backup);
        }
    }
}

#[derive(Clone, serde::Serialize)]
struct Status {
    phase: String,
    ready_ms: Option<u64>,
    login_ms: Option<u64>,
    login_ok: bool,
    sid_prefix: Option<String>,
    owned_redis_ports: Vec<u16>,
    bench_spawned: bool,
    pid: u32,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            phase: "init".into(),
            ready_ms: None,
            login_ms: None,
            login_ok: false,
            sid_prefix: None,
            owned_redis_ports: Vec::new(),
            bench_spawned: false,
            pid: std::process::id(),
        }
    }
}

#[derive(Clone, serde::Serialize)]
struct DbInfo {
    path: String,
    exists: bool,
    size_mb: f64,
}

#[derive(Clone, serde::Serialize)]
struct ExportResult {
    success: bool,
    paths: Vec<String>,
    message: String,
}

#[derive(Clone, serde::Serialize)]
struct ImportResult {
    success: bool,
    message: String,
}

#[derive(Clone)]
struct Launcher {
    children: Arc<Mutex<Vec<Child>>>,
    status: Arc<Mutex<Status>>,
    logger: Logger,
}

impl Launcher {
    fn new(logger: Logger) -> Self {
        Self {
            children: Arc::new(Mutex::new(Vec::new())),
            status: Arc::new(Mutex::new(Status::default())),
            logger,
        }
    }

    fn set_phase(&self, phase: &str) {
        self.status.lock().unwrap().phase = phase.into();
        self.logger.log(&format!("phase: {phase}"));
    }

    fn add_child(&self, child: Child) {
        if let Ok(mut children) = self.children.lock() {
            children.push(child);
        }
    }

    fn reap(&self) {
        let mut children = match self.children.lock() {
            Ok(c) => c,
            Err(_) => return,
        };
        if children.is_empty() {
            return;
        }
        self.logger.log("app exit: terminating owned child process groups");
        for child in children.iter() {
            let pid = child.id() as i32;
            unsafe {
                libc::kill(-pid, libc::SIGTERM);
            }
        }
        thread::sleep(Duration::from_millis(500));
        for child in children.iter_mut() {
            let pid = child.id() as i32;
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
            let _ = child.wait();
            self.logger.log(&format!("app exit: killed pid group {pid}"));
        }
        children.clear();
    }

    fn watch_children(&self) {
        let children_arc = self.children.clone();
        let logger = self.logger.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_secs(5));
                if let Ok(mut children) = children_arc.lock() {
                    let mut i = 0;
                    while i < children.len() {
                        let exited = children[i].try_wait().unwrap_or(None).is_some();
                        if exited {
                            let pid = children[i].id();
                            logger.log(&format!("child process {pid} exited unexpectedly"));
                            children.remove(i);
                        } else {
                            i += 1;
                        }
                    }
                }
            }
        });
    }
}

#[tauri::command]
fn launcher_status(launcher: State<Launcher>) -> Status {
    launcher.status.lock().unwrap().clone()
}

fn port_open(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

fn spawn_in_group(cmd: &mut Command) -> std::io::Result<Child> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn()
}

fn spawn_pipe<R: Read + Send + 'static>(mut src: R, path: PathBuf) {
    thread::spawn(move || {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();
        let mut buf = [0u8; 8192];
        loop {
            match src.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if let Some(f) = file.as_mut() {
                        let _ = f.write_all(&buf[..n]);
                        let _ = f.flush();
                    }
                }
            }
        }
    });
}

fn http_raw(port: u16, request: &str) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    stream.write_all(request.as_bytes()).ok()?;
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => data.extend_from_slice(&buf[..n]),
        }
    }
    String::from_utf8(data).ok()
}

fn ping(port: u16) -> bool {
    let req = format!(
        "GET /api/method/ping HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    );
    http_raw(port, &req)
        .map(|r| r.starts_with("HTTP/1.") && r.lines().next().is_some_and(|l| l.contains(" 200 ")))
        .unwrap_or(false)
}

fn wait_ready(port: u16, timeout: Duration, logger: &Logger) -> Option<Duration> {
    let start = Instant::now();
    loop {
        if ping(port) {
            return Some(start.elapsed());
        }
        if start.elapsed() >= timeout {
            logger.log(&format!("readiness timeout after {}s", timeout.as_secs()));
            return None;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn login(port: u16, user: &str, pass: &str) -> Option<String> {
    let body = format!("{{\"usr\":\"{user}\",\"pwd\":\"{pass}\"}}");
    let req = format!(
        "POST /api/method/login HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{}",
        body.len(),
        body
    );
    let resp = http_raw(port, &req)?;
    let status_line = resp.lines().next()?;
    if !status_line.contains(" 200 ") {
        return None;
    }
    for line in resp.lines() {
        if line.len() > 11 && line[..11].eq_ignore_ascii_case("set-cookie:") {
            let value = line[11..].trim();
            if let Some(sid) = value.strip_prefix("sid=") {
                let sid = sid.split(';').next().unwrap_or("").trim();
                if !sid.is_empty() {
                    return Some(sid.to_string());
                }
            }
        }
    }
    None
}

fn init_script(user: &str, pass: &str) -> String {
    format!(
        r#"(function () {{
  var user = {user:?};
  var pass = {pass:?};
  function log(tag, val) {{
    try {{
      fetch("/app-log/" + tag + "/" + encodeURIComponent(val), {{ cache: "no-store" }});
    }} catch (e) {{}}
  }}
  function loggedUser() {{
    return fetch("/api/method/frappe.auth.get_logged_user", {{ credentials: "include", cache: "no-store" }})
      .then(function (r) {{ return r.json(); }})
      .then(function (d) {{ return (d && d.message) || "no-message"; }});
  }}
  function inPageLogin() {{
    return fetch("/api/method/login", {{
      method: "POST",
      headers: {{ "Content-Type": "application/json" }},
      credentials: "include",
      cache: "no-store",
      body: JSON.stringify({{ usr: user, pwd: pass }})
    }}).then(function (r) {{
      return r.json().then(function (d) {{ return {{ status: r.status, d: d }}; }});
    }});
  }}
  function goBooksIfAuthed(msg, tag) {{
    if (msg !== "Guest" && msg !== "no-message") {{
      if (location.pathname === "/" || location.pathname === "/login") {{
        location.replace("/books");
      }}
      return true;
    }}
    return false;
  }}
  function run() {{
    loggedUser().then(function (msg) {{
      log("auth1", msg + "@" + location.pathname);
      if (goBooksIfAuthed(msg)) {{
        return;
      }}
      return inPageLogin().then(function (res) {{
        log("login", res.status + ":" + ((res.d && res.d.message) || "err"));
        return loggedUser().then(function (msg2) {{
          log("auth2", msg2 + "@" + location.pathname);
          goBooksIfAuthed(msg2);
        }});
      }});
    }}).catch(function (e) {{ log("js-err", String(e)); }});
  }}
  if (document.readyState === "loading") {{
    document.addEventListener("DOMContentLoaded", run);
  }} else {{
    run();
  }}
}})();"#
    )
}

fn ensure_site_if_missing(bench_dir: &Path, bench_bin: &str, site: &str) {
    let sites_dir = bench_dir.join("sites");
    let site_dir = sites_dir.join(site);
    if !site_dir.exists() {
        let mut cmd = Command::new(bench_bin);
        cmd.current_dir(bench_dir)
            .arg("new-site")
            .arg(site)
            .arg("--admin-password")
            .arg("admin")
            .arg("--db-type")
            .arg("sqlite")
            .arg("--force")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _ = cmd.status();
    }
}

fn migrate_site_if_needed(bench_dir: &Path, bench_bin: &str, site: &str) {
    let mut cmd = Command::new(bench_bin);
    cmd.current_dir(bench_dir)
        .arg("--site")
        .arg(site)
        .arg("migrate")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = cmd.status();
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = app.path().app_log_dir()?;
    fs::create_dir_all(&log_dir)?;
    let logger = Logger::new(log_dir.join("launcher.log"));
    logger.log("=== books-app launcher starting ===");
    let launcher = Launcher::new(logger.clone());
    launcher.watch_children();
    app.manage(launcher.clone());

    let bench_dir = PathBuf::from(env_or(
        "BOOKS_BENCH_DIR",
        "/app/books/bench",
    ));
    let bench_bin = env_or(
        "BOOKS_BENCH_BIN",
        "bench",
    );
    let redis_bin = env_or("BOOKS_REDIS_BIN", "/app/bin/redis-server");
    let admin_user = env_or("BOOKS_ADMIN_USER", "Administrator");
    let admin_pass = env_or("BOOKS_ADMIN_PASSWORD", "admin");

    if !bench_dir.exists() {
        logger.log(&format!("WARNING: bench_dir does not exist: {}", bench_dir.display()));
    }

    ensure_site_if_missing(&bench_dir, &bench_bin, SITE);
    migrate_site_if_needed(&bench_dir, &bench_bin, SITE);

    launcher.set_phase("starting redis");
    for (conf, port) in REDIS_CONFS {
        if port_open(port) {
            logger.log(&format!("redis {port}: already listening, reusing"));
            continue;
        }
        let conf_path = bench_dir.join(conf);
        if !conf_path.exists() {
            logger.log(&format!("redis {port}: conf not found {}", conf_path.display()));
            continue;
        }
        let mut cmd = Command::new(&redis_bin);
        cmd.arg(&conf_path.to_string_lossy().to_string())
            .arg("--port")
            .arg(port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match spawn_in_group(&mut cmd) {
            Ok(mut child) => {
                let pid = child.id();
                let out = log_dir.join(format!("redis_{port}.log"));
                if let Some(o) = child.stdout.take() {
                    spawn_pipe(o, out.clone());
                }
                if let Some(e) = child.stderr.take() {
                    spawn_pipe(e, out);
                }
                launcher.add_child(child);
                launcher.status.lock().unwrap().owned_redis_ports.push(port);
                logger.log(&format!("redis {port}: spawned pid {pid}"));
                thread::sleep(Duration::from_millis(500));
                if !port_open(port) {
                    thread::sleep(Duration::from_secs(5));
                }
                if !port_open(port) {
                    logger.log(&format!("redis {port}: not listening"));
                }
            }
            Err(e) => logger.log(&format!("redis {port}: spawn failed: {e}")),
        }
    }

    launcher.set_phase("starting bench serve");
    let bench_spawned = if port_open(PORT) {
        logger.log(&format!("bench serve: port {PORT} already in use"));
        false
    } else {
        let mut cmd = Command::new(&bench_bin);
        cmd.current_dir(&bench_dir)
            .arg("--site")
            .arg(SITE)
            .arg("serve")
            .arg("--port")
            .arg(PORT.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match spawn_in_group(&mut cmd) {
            Ok(mut child) => {
                let pid = child.id();
                let out = log_dir.join("bench_serve.log");
                if let Some(o) = child.stdout.take() {
                    spawn_pipe(o, out.clone());
                }
                if let Some(e) = child.stderr.take() {
                    spawn_pipe(e, out);
                }
                launcher.add_child(child);
                logger.log(&format!("bench serve: spawned pid {pid}"));
                true
            }
            Err(e) => {
                logger.log(&format!("bench serve: spawn failed: {e}"));
                false
            }
        }
    };
    launcher.status.lock().unwrap().bench_spawned = bench_spawned;

    launcher.set_phase("waiting for /api/method/ping");
    let ready = wait_ready(PORT, READY_TIMEOUT, &logger);
    {
        let mut st = launcher.status.lock().unwrap();
        st.ready_ms = ready.map(|d| d.as_millis() as u64);
        st.phase = if ready.is_some() { "ready".into() } else { "ready-timeout".into() };
    }
    match ready {
        Some(d) => logger.log(&format!("ready: OK after {}ms", d.as_millis())),
        None => logger.log("ready: FAILED within 60s"),
    }

    let sid = if ready.is_some() {
        launcher.set_phase("auto-login");
        let t1 = Instant::now();
        match login(PORT, &admin_user, &admin_pass) {
            Some(sid) => {
                let ms = t1.elapsed().as_millis() as u64;
                logger.log(&format!("login: OK as {admin_user} in {ms}ms"));
                {
                    let mut st = launcher.status.lock().unwrap();
                    st.login_ok = true;
                    st.login_ms = Some(ms);
                    st.sid_prefix = Some(sid[..sid.len().min(8)].to_string());
                    st.phase = "login-ok".into();
                }
                Some(sid)
            }
            None => {
                logger.log("login: FAILED (manual login)");
                launcher.status.lock().unwrap().phase = "login-failed".into();
                None
            }
        }
    } else {
        None
    };

    launcher.set_phase("opening window");
    let url = Url::parse(&format!("http://127.0.0.1:{PORT}/books"))?;
    let mut builder = WebviewWindowBuilder::new(
        app,
        "main",
        tauri::WebviewUrl::External(url),
    )
        .title("Frappe Books")
        .inner_size(1200.0, 900.0)
        .min_inner_size(800.0, 600.0);
    if sid.is_some() {
        builder = builder.initialization_script(&init_script(&admin_user, &admin_pass));
    }
    if let Err(e) = builder.build() {
        logger.log(&format!("window: failed to open: {e}"));
        launcher.reap();
        return Err(e.into());
    }
    logger.log("window: opened");
    launcher.status.lock().unwrap().phase = "window-open".into();
    logger.log("=== launcher setup complete ===");
    Ok(())
}

#[tauri::command]
fn get_db_info(bench_dir: String) -> Vec<DbInfo> {
    let mut infos = Vec::new();
    let site_dir = PathBuf::from(bench_dir).join("sites").join(SITE);
    let db_dir = site_dir.join("db");
    if db_dir.exists() {
        let _ = fs::read_dir(&db_dir).map(|entries| {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map(|e| e == "db").unwrap_or(false) {
                    let size = fs::metadata(&path).map(|m| m.len() as f64 / (1024.0 * 1024.0)).unwrap_or(0.0);
                    infos.push(DbInfo {
                        path: path.to_string_lossy().to_string(),
                        exists: true,
                        size_mb: (size * 100.0).round() / 100.0,
                    });
                }
            }
        });
    }
    infos
}

#[tauri::command]
async fn export_db(bench_dir: String, target_dir: String) -> Result<ExportResult, String> {
    let site_dir = PathBuf::from(&bench_dir).join("sites").join(SITE);
    let db_dir = site_dir.join("db");
    let target = PathBuf::from(&target_dir);
    if !target.exists() {
        fs::create_dir_all(&target).map_err(|e| e.to_string())?;
    }
    let mut exported = Vec::new();
    if db_dir.exists() {
        let entries: Vec<_> = fs::read_dir(&db_dir).map_err(|e| e.to_string())?.flatten().collect();
        for entry in entries {
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext == "db" || ext == "sqlite" || ext == "sqlite3" || ext == "wal" || ext == "shm" || ext == "journal" {
                    let filename = path.file_name().unwrap().to_string_lossy().to_string();
                    let dst = target.join(&filename);
                    fs::copy(&path, &dst).map_err(|e| e.to_string())?;
                    exported.push(dst.to_string_lossy().to_string());
                }
            }
        }
    }
    let count = exported.len();
    Ok(ExportResult {
        success: true,
        paths: exported,
        message: format!("Exported {count} files"),
    })
}

#[tauri::command]
async fn import_db(bench_dir: String, source_path: String) -> Result<ImportResult, String> {
    let site_dir = PathBuf::from(&bench_dir).join("sites").join(SITE);
    let db_dir = site_dir.join("db");
    fs::create_dir_all(&db_dir).map_err(|e| e.to_string())?;
    let src = PathBuf::from(&source_path);
    if !src.exists() || !src.is_file() {
        return Ok(ImportResult {
            success: false,
            message: "Source file does not exist or is not a file".to_string(),
        });
    }
    let filename = src.file_name().unwrap().to_string_lossy().to_string();
    let dst = db_dir.join(&filename);
    fs::copy(&src, &dst).map_err(|e| e.to_string())?;
    Ok(ImportResult {
        success: true,
        message: format!("Imported {} to {}", filename, dst.display()),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            launcher_status,
            get_db_info,
            export_db,
            import_db
        ])
        .setup(setup)
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|handle, event| match event {
        RunEvent::ExitRequested { .. } | RunEvent::Exit => {
            handle.state::<Launcher>().reap();
        }
        _ => {}
    });
}
