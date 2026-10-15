use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{Emitter, Manager};

struct SidecarHandle {
    child: Child,
    stdin: ChildStdin,
}

struct AppState {
    db: Mutex<Connection>,
    sidecar: Mutex<Option<SidecarHandle>>,
    data_dir: PathBuf,
    tasks_dir: PathBuf,
    project_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskRequest {
    mode: String,
    keyword: String,
    forum_name: String,
    targets: String,
    max_posts: u32,
    max_comments: u32,
    interval_seconds: f64,
    include_comments: bool,
    include_nested: bool,
    download_media: bool,
    browser_mode: String,
}

impl AppState {
    fn new(app: &tauri::App) -> Result<Self, Box<dyn std::error::Error>> {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let data_dir = if cfg!(debug_assertions) {
            project_root.join("data")
        } else {
            app.path().app_local_data_dir()?.join("data")
        };
        let tasks_dir = data_dir.join("tasks");
        fs::create_dir_all(&tasks_dir)?;
        let db = Connection::open(data_dir.join("tieba.sqlite3"))?;
        db.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS tasks (
               id TEXT PRIMARY KEY, name TEXT NOT NULL, mode TEXT NOT NULL,
               status TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
               progress INTEGER NOT NULL DEFAULT 0, posts_count INTEGER NOT NULL DEFAULT 0,
               comments_count INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS posts (
               id TEXT NOT NULL, task_id TEXT NOT NULL, title TEXT NOT NULL DEFAULT '',
               body TEXT NOT NULL DEFAULT '', forum TEXT NOT NULL DEFAULT '', author TEXT NOT NULL DEFAULT '',
               published_at TEXT NOT NULL DEFAULT '', replies INTEGER NOT NULL DEFAULT 0,
               media INTEGER NOT NULL DEFAULT 0, keyword TEXT NOT NULL DEFAULT '', payload TEXT NOT NULL,
               PRIMARY KEY(task_id, id), FOREIGN KEY(task_id) REFERENCES tasks(id)
             );
             CREATE TABLE IF NOT EXISTS comments (
               id TEXT NOT NULL, task_id TEXT NOT NULL, post_id TEXT NOT NULL, parent_id TEXT NOT NULL DEFAULT '',
               body TEXT NOT NULL DEFAULT '', author TEXT NOT NULL DEFAULT '', published_at TEXT NOT NULL DEFAULT '',
               floor TEXT NOT NULL DEFAULT '', nested INTEGER NOT NULL DEFAULT 0, payload TEXT NOT NULL,
               PRIMARY KEY(task_id, id), FOREIGN KEY(task_id) REFERENCES tasks(id)
             );
             CREATE TABLE IF NOT EXISTS attachments (
               id TEXT NOT NULL, task_id TEXT NOT NULL, post_id TEXT NOT NULL, comment_id TEXT NOT NULL DEFAULT '',
               kind TEXT NOT NULL DEFAULT '图片', source_url TEXT NOT NULL, relative_path TEXT NOT NULL DEFAULT '',
               status TEXT NOT NULL DEFAULT '待下载', payload TEXT NOT NULL,
               PRIMARY KEY(task_id, id), FOREIGN KEY(task_id) REFERENCES tasks(id)
             );
             CREATE TABLE IF NOT EXISTS logs (
               id INTEGER PRIMARY KEY AUTOINCREMENT, task_id TEXT NOT NULL DEFAULT '',
               time TEXT NOT NULL, level TEXT NOT NULL, message TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS idx_posts_forum ON posts(forum);
             CREATE INDEX IF NOT EXISTS idx_comments_post ON comments(post_id);
             CREATE INDEX IF NOT EXISTS idx_attachments_post ON attachments(post_id);
             CREATE INDEX IF NOT EXISTS idx_logs_task ON logs(task_id, id DESC);",
        )?;
        db.execute(
            "UPDATE tasks SET status='stopped', updated_at=?1 WHERE status='running'",
            [now_string()],
        )?;
        for (key, value) in [
            ("browserMode", "\"existing\""),
            ("maxPostsDefault", "20"),
            ("maxCommentsDefault", "50"),
            ("intervalSecondsDefault", "1.5"),
        ] {
            db.execute(
                "INSERT OR IGNORE INTO settings(key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        Ok(Self {
            db: Mutex::new(db),
            sidecar: Mutex::new(None),
            data_dir,
            tasks_dir,
            project_root,
        })
    }
}

fn now_string() -> String {
    Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn task_status_label(status: &str) -> &str {
    match status {
        "running" => "运行中",
        "completed" => "已完成",
        "stopped" => "已停止",
        "failed" => "失败",
        _ => status,
    }
}

fn log_time_from_rfc3339(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|_| value.to_string())
}

fn create_task_id() -> String {
    format!("TB-{}", Utc::now().format("%Y%m%d-%H%M%S-%3f"))
}

fn insert_log(db: &Connection, task_id: &str, level: &str, message: &str) -> rusqlite::Result<()> {
    db.execute(
        "INSERT INTO logs(task_id, time, level, message) VALUES (?1, ?2, ?3, ?4)",
        params![task_id, now_string(), level, message],
    )?;
    Ok(())
}

fn save_event(state: &AppState, event: &Value) {
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
    let task_id = event.get("taskId").and_then(Value::as_str).unwrap_or("");
    let data = event.get("data").unwrap_or(&Value::Null);
    let Ok(db) = state.db.lock() else { return };
    let result: rusqlite::Result<()> = (|| {
        match event_type {
            "log" => {
                let message = event.get("message").and_then(Value::as_str).unwrap_or("");
                let level = event.get("level").and_then(Value::as_str).unwrap_or("信息");
                insert_log(&db, task_id, level, message)?;
            }
            "note" => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or("");
                if id.is_empty() {
                    return Ok(());
                }
                let mut stored_payload = data.clone();
                if let Some(object) = stored_payload.as_object_mut() {
                    object.insert("taskId".into(), json!(task_id));
                }
                let payload = stored_payload.to_string();
                db.execute(
                    "INSERT INTO posts(id, task_id, title, body, forum, author, published_at, replies, media, keyword, payload)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                     ON CONFLICT(task_id, id) DO UPDATE SET title=excluded.title, body=excluded.body,
                     forum=excluded.forum, author=excluded.author, published_at=excluded.published_at,
                     replies=excluded.replies, media=excluded.media, keyword=excluded.keyword, payload=excluded.payload",
                    params![
                        id, task_id,
                        data.get("title").and_then(Value::as_str).unwrap_or(""),
                        data.get("body").and_then(Value::as_str).unwrap_or(""),
                        data.get("forum").and_then(Value::as_str).unwrap_or(""),
                        data.get("author").and_then(Value::as_str).unwrap_or("匿名"),
                        data.get("time").and_then(Value::as_str).unwrap_or(""),
                        data.get("replies").and_then(Value::as_i64).unwrap_or(0),
                        data.get("media").and_then(Value::as_i64).unwrap_or(0),
                        data.get("keyword").and_then(Value::as_str).unwrap_or(""),
                        payload,
                    ],
                )?;
                db.execute("UPDATE tasks SET posts_count=(SELECT count(*) FROM posts WHERE task_id=?1), updated_at=?2 WHERE id=?1", params![task_id, now_string()])?;
            }
            "comment" => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or("");
                if id.is_empty() {
                    return Ok(());
                }
                let mut stored_payload = data.clone();
                if let Some(object) = stored_payload.as_object_mut() {
                    object.insert("taskId".into(), json!(task_id));
                }
                db.execute(
                    "INSERT INTO comments(id, task_id, post_id, parent_id, body, author, published_at, floor, nested, payload)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                     ON CONFLICT(task_id, id) DO UPDATE SET body=excluded.body, author=excluded.author,
                     published_at=excluded.published_at, floor=excluded.floor, nested=excluded.nested, payload=excluded.payload",
                    params![
                        id, task_id,
                        data.get("postId").and_then(Value::as_str).unwrap_or(""),
                        data.get("parentId").and_then(Value::as_str).unwrap_or(""),
                        data.get("body").and_then(Value::as_str).unwrap_or(""),
                        data.get("author").and_then(Value::as_str).unwrap_or("匿名"),
                        data.get("time").and_then(Value::as_str).unwrap_or(""),
                        data.get("floor").and_then(Value::as_str).unwrap_or(""),
                        data.get("nested").and_then(Value::as_i64).unwrap_or(0),
                        stored_payload.to_string(),
                    ],
                )?;
                db.execute("UPDATE tasks SET comments_count=(SELECT count(*) FROM comments WHERE task_id=?1), updated_at=?2 WHERE id=?1", params![task_id, now_string()])?;
            }
            "attachment" => {
                let id = data.get("id").and_then(Value::as_str).unwrap_or("");
                let source_url = data.get("sourceUrl").and_then(Value::as_str).unwrap_or("");
                if id.is_empty() || source_url.is_empty() {
                    return Ok(());
                }
                let mut stored_payload = data.clone();
                if let Some(object) = stored_payload.as_object_mut() {
                    object.insert("taskId".into(), json!(task_id));
                }
                db.execute(
                    "INSERT INTO attachments(id, task_id, post_id, comment_id, kind, source_url, relative_path, status, payload)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                     ON CONFLICT(task_id, id) DO UPDATE SET relative_path=excluded.relative_path, status=excluded.status, payload=excluded.payload",
                    params![
                        id, task_id,
                        data.get("postId").and_then(Value::as_str).unwrap_or(""),
                        data.get("commentId").and_then(Value::as_str).unwrap_or(""),
                        data.get("kind").and_then(Value::as_str).unwrap_or("图片"),
                        source_url,
                        data.get("relativePath").and_then(Value::as_str).unwrap_or(""),
                        data.get("status").and_then(Value::as_str).unwrap_or("待下载"),
                        stored_payload.to_string(),
                    ],
                )?;
                db.execute("UPDATE posts SET media=(SELECT count(*) FROM attachments WHERE task_id=?1 AND post_id=?2), payload=json_set(payload, '$.media', (SELECT count(*) FROM attachments WHERE task_id=?1 AND post_id=?2)) WHERE task_id=?1 AND id=?2", params![task_id, data.get("postId").and_then(Value::as_str).unwrap_or("")])?;
            }
            "progress" => {
                db.execute(
                    "UPDATE tasks SET progress=?2, posts_count=?3, comments_count=?4, updated_at=?5 WHERE id=?1",
                    params![
                        task_id,
                        data.get("progress").and_then(Value::as_i64).unwrap_or(0),
                        data.get("posts").and_then(Value::as_i64).unwrap_or(0),
                        data.get("comments").and_then(Value::as_i64).unwrap_or(0),
                        now_string(),
                    ],
                )?;
            }
            "finished" => {
                let status = data
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("completed");
                db.execute(
                    "UPDATE tasks SET status=?2, progress=?3, updated_at=?4 WHERE id=?1",
                    params![
                        task_id,
                        status,
                        if status == "completed" { 100 } else { 0 },
                        now_string()
                    ],
                )?;
                insert_log(
                    &db,
                    task_id,
                    if status == "completed" {
                        "成功"
                    } else {
                        "警告"
                    },
                    event
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("任务已结束"),
                )?;
            }
            "error" => {
                let message = event
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("采集服务发生错误");
                db.execute(
                    "UPDATE tasks SET status='failed', updated_at=?2 WHERE id=?1",
                    params![task_id, now_string()],
                )?;
                insert_log(&db, task_id, "错误", message)?;
            }
            _ => {}
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("Failed to persist sidecar event {event_type}: {error}");
    }
}

fn spawn_sidecar(app: &tauri::AppHandle, state: &Arc<AppState>) -> Result<(), String> {
    let mut slot = state
        .sidecar
        .lock()
        .map_err(|_| "采集进程状态不可用".to_string())?;
    if let Some(handle) = slot.as_mut() {
        if handle
            .child
            .try_wait()
            .map_err(|e| e.to_string())?
            .is_none()
        {
            return Ok(());
        }
        *slot = None;
    }

    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|path| path.join("tieba-sidecar.exe"));
    let mut command = if let Some(exe) = bundled.filter(|path| path.exists()) {
        Command::new(exe)
    } else {
        let sidecar_dir = state.project_root.join("python-sidecar");
        let script = sidecar_dir.join("main.py");
        let mut command = Command::new("uv");
        command
            .args(["run", "--no-dev", "--project"])
            .arg(&sidecar_dir)
            .args(["python", "-u"])
            .arg(script);
        command.current_dir(&sidecar_dir);
        command
    };
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 Python 采集服务：{error}。请确认 uv 已安装。"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法连接采集服务输入流".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取采集服务输出流".to_string())?;
    if let Some(stderr) = child.stderr.take() {
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().flatten() {
                eprintln!("[tieba-sidecar] {line}");
            }
        });
    }
    let app_handle = app.clone();
    let state_for_reader = state.clone();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().flatten() {
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                eprintln!("Ignoring invalid sidecar JSONL event");
                continue;
            };
            save_event(&state_for_reader, &event);
            let _ = app_handle.emit("crawler:event", event);
        }
        if let Ok(db) = state_for_reader.db.lock() {
            let _ = db.execute(
                "UPDATE tasks SET status='stopped', updated_at=?1 WHERE status='running'",
                [now_string()],
            );
        }
    });
    *slot = Some(SidecarHandle { child, stdin });
    Ok(())
}

fn send_sidecar(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    message: Value,
) -> Result<(), String> {
    spawn_sidecar(app, state)?;
    let mut slot = state
        .sidecar
        .lock()
        .map_err(|_| "采集进程状态不可用".to_string())?;
    let handle = slot.as_mut().ok_or_else(|| "采集服务未启动".to_string())?;
    let mut writer = BufWriter::new(&mut handle.stdin);
    serde_json::to_writer(&mut writer, &message).map_err(|error| error.to_string())?;
    writer
        .write_all(b"\n")
        .map_err(|error| format!("无法发送采集请求：{error}"))?;
    writer
        .flush()
        .map_err(|error| format!("无法刷新采集请求：{error}"))?;
    Ok(())
}

fn task_json(row: (String, String, String, String, String, i64, i64, i64)) -> Value {
    let (id, name, mode, status, updated_at, progress, posts, comments) = row;
    json!({
        "id": id,
        "name": name,
        "mode": mode,
        "status": task_status_label(&status),
        "time": log_time_from_rfc3339(&updated_at),
        "posts": posts,
        "comments": comments,
        "progress": progress,
    })
}

fn query_payloads(db: &Connection, sql: &str, query: &str) -> Result<Vec<Value>, String> {
    let needle = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
    let mut statement = db.prepare(sql).map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![needle], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?;
    rows.map(|row| {
        let raw = row.map_err(|error| error.to_string())?;
        serde_json::from_str::<Value>(&raw).map_err(|error| error.to_string())
    })
    .collect()
}

#[tauri::command]
fn get_tasks(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<Value>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库不可用".to_string())?;
    let mut statement = db
        .prepare("SELECT id,name,mode,status,updated_at,progress,posts_count,comments_count FROM tasks ORDER BY created_at DESC")
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        })
        .map_err(|error| error.to_string())?;
    rows.map(|row| row.map(task_json).map_err(|error| error.to_string()))
        .collect()
}

#[tauri::command]
fn get_posts(state: tauri::State<'_, Arc<AppState>>, query: String) -> Result<Vec<Value>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库不可用".to_string())?;
    query_payloads(&db, "SELECT payload FROM posts WHERE title LIKE ?1 ESCAPE '\\' OR body LIKE ?1 ESCAPE '\\' OR forum LIKE ?1 ESCAPE '\\' OR keyword LIKE ?1 ESCAPE '\\' ORDER BY rowid DESC", &query)
}

#[tauri::command]
fn get_comments(
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
) -> Result<Vec<Value>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库不可用".to_string())?;
    query_payloads(&db, "SELECT payload FROM comments WHERE body LIKE ?1 ESCAPE '\\' OR author LIKE ?1 ESCAPE '\\' OR post_id LIKE ?1 ESCAPE '\\' ORDER BY rowid DESC", &query)
}

#[tauri::command]
fn get_attachments(
    state: tauri::State<'_, Arc<AppState>>,
    query: String,
) -> Result<Vec<Value>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库不可用".to_string())?;
    let mut rows = query_payloads(&db, "SELECT payload FROM attachments WHERE source_url LIKE ?1 ESCAPE '\\' OR relative_path LIKE ?1 ESCAPE '\\' OR post_id LIKE ?1 ESCAPE '\\' ORDER BY rowid DESC", &query)?;
    for row in &mut rows {
        if let Some(relative) = row.get("relativePath").and_then(Value::as_str) {
            if !relative.is_empty() {
                let absolute = state
                    .data_dir
                    .join("tasks")
                    .join(row.get("taskId").and_then(Value::as_str).unwrap_or(""))
                    .join(relative);
                if absolute.exists() {
                    row["localPath"] = json!(absolute.to_string_lossy());
                }
            }
        }
    }
    Ok(rows)
}

#[tauri::command]
fn get_logs(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<Value>, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库不可用".to_string())?;
    let mut statement = db
        .prepare("SELECT id,task_id,time,level,message FROM logs ORDER BY id DESC LIMIT 1000")
        .map_err(|error| error.to_string())?;
    let rows = statement.query_map([], |row| {
        let time: String = row.get(2)?;
        Ok(json!({ "id": row.get::<_, i64>(0)?, "taskId": row.get::<_, String>(1)?, "time": log_time_from_rfc3339(&time), "level": row.get::<_, String>(3)?, "message": row.get::<_, String>(4)? }))
    }).map_err(|error| error.to_string())?;
    rows.map(|row| row.map_err(|error| error.to_string()))
        .collect()
}

fn read_setting(db: &Connection, key: &str, default: Value) -> Result<Value, String> {
    let raw = db
        .query_row("SELECT value FROM settings WHERE key=?1", [key], |row| {
            row.get::<_, String>(0)
        })
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(raw
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or(default))
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, Arc<AppState>>) -> Result<Value, String> {
    let db = state
        .db
        .lock()
        .map_err(|_| "本地数据库不可用".to_string())?;
    Ok(json!({
        "browserMode": read_setting(&db, "browserMode", json!("existing"))?,
        "maxPostsDefault": read_setting(&db, "maxPostsDefault", json!(20))?,
        "maxCommentsDefault": read_setting(&db, "maxCommentsDefault", json!(50))?,
        "intervalSecondsDefault": read_setting(&db, "intervalSecondsDefault", json!(1.5))?,
        "archiveRoot": state.tasks_dir.to_string_lossy(),
    }))
}

#[tauri::command]
fn save_settings(state: tauri::State<'_, Arc<AppState>>, settings: Value) -> Result<(), String> {
    let mode = settings
        .get("browserMode")
        .and_then(Value::as_str)
        .unwrap_or("existing");
    if !["existing", "managed"].contains(&mode) {
        return Err("浏览器模式无效".into());
    }
    let max_posts = settings
        .get("maxPostsDefault")
        .and_then(Value::as_u64)
        .unwrap_or(20)
        .clamp(1, 100);
    let max_comments = settings
        .get("maxCommentsDefault")
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .clamp(1, 100);
    let interval = settings
        .get("intervalSecondsDefault")
        .and_then(Value::as_f64)
        .unwrap_or(1.5)
        .max(1.5);
    {
        let db = state
            .db
            .lock()
            .map_err(|_| "本地数据库不可用".to_string())?;
        for (key, value) in [
            ("browserMode", json!(mode)),
            ("maxPostsDefault", json!(max_posts)),
            ("maxCommentsDefault", json!(max_comments)),
            ("intervalSecondsDefault", json!(interval)),
        ] {
            db.execute("INSERT INTO settings(key,value) VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value", params![key, value.to_string()]).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
fn start_task(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    request: TaskRequest,
) -> Result<Value, String> {
    if !["keyword", "detail", "creator"].contains(&request.mode.as_str()) {
        return Err("请选择有效的采集方式".into());
    }
    if !["existing", "managed"].contains(&request.browser_mode.as_str()) {
        return Err("浏览器模式无效".into());
    }
    if request.max_posts == 0 || request.max_posts > 100 {
        return Err("帖子数量需在 1 到 100 之间".into());
    }
    if request.max_comments == 0 || request.max_comments > 100 {
        return Err("每帖评论数量需在 1 到 100 之间".into());
    }
    if request.interval_seconds < 1.5 {
        return Err("请求间隔不能低于 1.5 秒".into());
    }
    if request.mode == "keyword" && request.keyword.trim().is_empty() {
        return Err("请先填写搜索关键词".into());
    }
    if request.mode != "keyword" && request.targets.trim().is_empty() {
        return Err("请填写帖子或主页链接".into());
    }
    let id = create_task_id();
    let name = match request.mode.as_str() {
        "keyword" => format!(
            "{} · 关键词搜索",
            if !request.keyword.trim().is_empty() {
                request.keyword.trim()
            } else {
                request.forum_name.trim()
            }
        ),
        "detail" => format!("指定帖子 · 详情采集"),
        _ => format!("吧主主页 · 内容采集"),
    };
    let timestamp = now_string();
    {
        let db = state
            .db
            .lock()
            .map_err(|_| "本地数据库不可用".to_string())?;
        db.execute("INSERT INTO tasks(id,name,mode,status,created_at,updated_at,progress) VALUES(?1,?2,?3,'running',?4,?4,0)", params![id, name, request.mode, timestamp]).map_err(|error| error.to_string())?;
        insert_log(&db, &id, "信息", "任务已创建，准备连接浏览器")
            .map_err(|error| error.to_string())?;
    }
    let payload = serde_json::to_value(&request).map_err(|error| error.to_string())?;
    if let Err(error) = send_sidecar(
        &app,
        &state,
        json!({
            "action": "start_task", "taskId": id, "name": name, "request": payload,
            "archiveDir": state.tasks_dir.join(&id).to_string_lossy(),
            "profileDir": state.data_dir.join("browser-profile").to_string_lossy(),
        }),
    ) {
        let db = state
            .db
            .lock()
            .map_err(|_| "本地数据库不可用".to_string())?;
        db.execute(
            "UPDATE tasks SET status='failed', updated_at=?2 WHERE id=?1",
            params![id, now_string()],
        )
        .map_err(|db_error| db_error.to_string())?;
        insert_log(&db, &id, "错误", &error).map_err(|db_error| db_error.to_string())?;
        return Err(error);
    }
    Ok(
        json!({ "id": id, "name": name, "mode": request.mode, "status": "运行中", "time": log_time_from_rfc3339(&timestamp), "posts": 0, "comments": 0, "progress": 0 }),
    )
}

#[tauri::command]
fn stop_task(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    task_id: String,
) -> Result<(), String> {
    send_sidecar(
        &app,
        &state,
        json!({ "action": "stop_task", "taskId": task_id }),
    )?;
    Ok(())
}

#[tauri::command]
fn browser_action(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    action: String,
    browser_mode: String,
) -> Result<(), String> {
    if !["check", "open_login"].contains(&action.as_str()) {
        return Err("不支持的浏览器操作".into());
    }
    if !["existing", "managed"].contains(&browser_mode.as_str()) {
        return Err("浏览器模式无效".into());
    }
    send_sidecar(
        &app,
        &state,
        json!({
            "action": action,
            "browserMode": browser_mode,
            "profileDir": state.data_dir.join("browser-profile").to_string_lossy(),
        }),
    )
}

#[tauri::command]
fn open_archive(
    state: tauri::State<'_, Arc<AppState>>,
    task_id: Option<String>,
) -> Result<(), String> {
    let path = task_id
        .map(|id| state.tasks_dir.join(id))
        .unwrap_or_else(|| state.tasks_dir.clone());
    fs::create_dir_all(&path).map_err(|error| error.to_string())?;
    #[cfg(target_os = "windows")]
    Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(&path)
        .spawn()
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(&path)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn flatten_cell(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Array(items) => items
            .iter()
            .map(flatten_cell)
            .collect::<Vec<_>>()
            .join("; "),
        Value::Object(_) => value.to_string(),
    }
}

fn export_rows(rows: &[Value], path: &Path, format: &str) -> Result<(), String> {
    match format.to_ascii_lowercase().as_str() {
        "json" => fs::write(
            path,
            serde_json::to_vec_pretty(rows).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string()),
        "jsonl" => {
            let file = fs::File::create(path).map_err(|error| error.to_string())?;
            let mut writer = BufWriter::new(file);
            for row in rows {
                serde_json::to_writer(&mut writer, row).map_err(|error| error.to_string())?;
                writer.write_all(b"\n").map_err(|error| error.to_string())?;
            }
            writer.flush().map_err(|error| error.to_string())
        }
        "csv" => {
            let headers = rows
                .iter()
                .filter_map(Value::as_object)
                .flat_map(|row| row.keys().cloned())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let mut writer = csv::Writer::from_path(path).map_err(|error| error.to_string())?;
            writer
                .write_record(&headers)
                .map_err(|error| error.to_string())?;
            for row in rows {
                let object = row.as_object();
                let record = headers
                    .iter()
                    .map(|key| {
                        object
                            .and_then(|object| object.get(key))
                            .map(flatten_cell)
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>();
                writer
                    .write_record(record)
                    .map_err(|error| error.to_string())?;
            }
            writer.flush().map_err(|error| error.to_string())
        }
        "excel" | "xlsx" => {
            let headers = rows
                .iter()
                .filter_map(Value::as_object)
                .flat_map(|row| row.keys().cloned())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let mut workbook = rust_xlsxwriter::Workbook::new();
            let worksheet = workbook.add_worksheet();
            for (column, header) in headers.iter().enumerate() {
                worksheet
                    .write_string(0, column as u16, header)
                    .map_err(|error| error.to_string())?;
            }
            for (row_index, row) in rows.iter().enumerate() {
                let object = row.as_object();
                for (column, header) in headers.iter().enumerate() {
                    let cell = object
                        .and_then(|object| object.get(header))
                        .map(flatten_cell)
                        .unwrap_or_default();
                    worksheet
                        .write_string((row_index + 1) as u32, column as u16, cell)
                        .map_err(|error| error.to_string())?;
                }
            }
            workbook.save(path).map_err(|error| error.to_string())
        }
        _ => Err("导出格式仅支持 CSV、Excel、JSON、JSONL".into()),
    }
}

fn relative_export_media_path(
    export_task: Option<&str>,
    attachment_task: &str,
    relative: &str,
) -> String {
    let relative = relative.replace('\\', "/");
    match export_task {
        None => format!("tasks/{attachment_task}/{relative}"),
        Some(export_task) if export_task == attachment_task => format!("../{relative}"),
        Some(_) => format!("../../{attachment_task}/{relative}"),
    }
}

#[tauri::command]
fn export_data(
    state: tauri::State<'_, Arc<AppState>>,
    scope: String,
    format: String,
    query: String,
) -> Result<String, String> {
    if !["posts", "comments", "media", "logs"].contains(&scope.as_str()) {
        return Err("请选择有效的数据类型".into());
    }
    if !["csv", "excel", "json", "jsonl"].contains(&format.to_ascii_lowercase().as_str()) {
        return Err("不支持的导出格式".into());
    }
    let mut rows = {
        let db = state
            .db
            .lock()
            .map_err(|_| "本地数据库不可用".to_string())?;
        let filter = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        if scope == "logs" {
            let mut statement = db.prepare("SELECT id,task_id,time,level,message FROM logs WHERE message LIKE ?1 ESCAPE '\\' ORDER BY id DESC").map_err(|error| error.to_string())?;
            let matches = statement
                .query_map([filter], |row| {
                    Ok(json!({
                        "id": row.get::<_, i64>(0)?, "taskId": row.get::<_, String>(1)?,
                        "time": row.get::<_, String>(2)?, "level": row.get::<_, String>(3)?,
                        "message": row.get::<_, String>(4)?,
                    }))
                })
                .map_err(|error| error.to_string())?;
            matches
                .map(|row| row.map_err(|error| error.to_string()))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            let sql = match scope.as_str() {
            "comments" => "SELECT payload FROM comments WHERE body LIKE ?1 ESCAPE '\\' OR author LIKE ?1 ESCAPE '\\' OR post_id LIKE ?1 ESCAPE '\\' ORDER BY rowid DESC",
            "media" => "SELECT payload FROM attachments WHERE source_url LIKE ?1 ESCAPE '\\' OR relative_path LIKE ?1 ESCAPE '\\' OR post_id LIKE ?1 ESCAPE '\\' ORDER BY rowid DESC",
            _ => "SELECT payload FROM posts WHERE title LIKE ?1 ESCAPE '\\' OR body LIKE ?1 ESCAPE '\\' OR forum LIKE ?1 ESCAPE '\\' OR keyword LIKE ?1 ESCAPE '\\' ORDER BY rowid DESC",
        };
            let mut statement = db.prepare(sql).map_err(|error| error.to_string())?;
            let payloads = statement
                .query_map([filter], |row| row.get::<_, String>(0))
                .map_err(|error| error.to_string())?;
            payloads
                .map(|payload| {
                    payload.map_err(|error| error.to_string()).and_then(|raw| {
                        serde_json::from_str::<Value>(&raw).map_err(|error| error.to_string())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    };
    if rows.is_empty() {
        return Err("当前筛选条件没有可导出的数据".into());
    }
    let latest: Option<String> = {
        let db = state
            .db
            .lock()
            .map_err(|_| "本地数据库不可用".to_string())?;
        db.query_row(
            "SELECT id FROM tasks ORDER BY created_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
    };
    let export_dir = latest
        .as_ref()
        .map(|id| state.tasks_dir.join(id).join("exports"))
        .unwrap_or_else(|| state.data_dir.join("exports"));
    fs::create_dir_all(&export_dir).map_err(|error| error.to_string())?;
    if scope == "media" {
        for row in &mut rows {
            let task_id = row.get("taskId").and_then(Value::as_str).unwrap_or("");
            let relative = row
                .get("relativePath")
                .and_then(Value::as_str)
                .unwrap_or("");
            if !task_id.is_empty() && !relative.is_empty() {
                row["relativePath"] = json!(relative_export_media_path(
                    latest.as_deref(),
                    task_id,
                    relative
                ));
            }
        }
    }
    let ext = if format.eq_ignore_ascii_case("excel") {
        "xlsx".to_string()
    } else {
        format.to_ascii_lowercase()
    };
    let path = export_dir.join(format!(
        "贴吧数据-{}.{}",
        Utc::now().format("%Y%m%d-%H%M%S"),
        ext
    ));
    export_rows(&rows, &path, &format)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
fn open_source_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://tieba.baidu.com/") || url.chars().any(char::is_control) {
        return Err("仅允许打开贴吧来源链接".into());
    }
    #[cfg(target_os = "windows")]
    Command::new("explorer.exe")
        .arg(&url)
        .spawn()
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(url)
        .spawn()
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{export_rows, relative_export_media_path};
    use serde_json::json;
    use std::fs;

    #[test]
    fn exported_media_paths_resolve_from_the_export_folder() {
        assert_eq!(
            relative_export_media_path(Some("task-new"), "task-new", "media/pic.jpg"),
            "../media/pic.jpg"
        );
        assert_eq!(
            relative_export_media_path(Some("task-new"), "task-old", "media/pic.jpg"),
            "../../task-old/media/pic.jpg"
        );
        assert_eq!(
            relative_export_media_path(None, "task-old", "media\\pic.jpg"),
            "tasks/task-old/media/pic.jpg"
        );
    }

    #[test]
    fn all_export_formats_write_readable_files() {
        let directory = std::env::temp_dir().join(format!(
            "tieba-export-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default(),
        ));
        fs::create_dir_all(&directory).unwrap();
        let rows = vec![json!({"id": "123", "body": "中文,\n内容", "tags": ["一", "二"]})];
        for (format, extension) in [
            ("CSV", "csv"),
            ("Excel", "xlsx"),
            ("JSON", "json"),
            ("JSONL", "jsonl"),
        ] {
            let path = directory.join(format!("export.{extension}"));
            export_rows(&rows, &path, format).unwrap();
            let bytes = fs::read(&path).unwrap();
            assert!(!bytes.is_empty(), "{format} export should not be empty");
            match format {
                "CSV" => assert!(String::from_utf8(bytes).unwrap().contains("中文,\n内容")),
                "Excel" => assert!(bytes.starts_with(b"PK\x03\x04")),
                "JSON" => assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()[0]["id"],
                    "123"
                ),
                "JSONL" => assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["id"],
                    "123"
                ),
                _ => unreachable!(),
            }
        }
        fs::remove_dir_all(directory).unwrap();
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let state = AppState::new(app)?;
            app.manage(Arc::new(state));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_tasks,
            get_posts,
            get_comments,
            get_attachments,
            get_logs,
            get_settings,
            save_settings,
            start_task,
            stop_task,
            browser_action,
            open_archive,
            export_data,
            open_source_url,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                if let Some(state) = app.try_state::<Arc<AppState>>() {
                    if let Ok(mut slot) = state.sidecar.lock() {
                        if let Some(mut handle) = slot.take() {
                            let _ = serde_json::to_writer(
                                &mut handle.stdin,
                                &json!({"action":"shutdown"}),
                            );
                            let _ = handle.stdin.write_all(b"\n");
                            let _ = handle.stdin.flush();
                            thread::sleep(std::time::Duration::from_millis(250));
                            let _ = handle.child.kill();
                        }
                    }
                }
            }
        });
}
