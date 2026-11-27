use crate::{config::{self, AppConfig}, engine, hardware, models::ModelManifest};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use std::{fs, os::windows::process::CommandExt, path::{Path, PathBuf}, process::{Child, Command, Stdio}, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, time::Duration};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;
const BELOW_NORMAL: u32 = 0x00004000;
const CREATE_NO_WINDOW: u32 = 0x08000000;
pub struct RunnerState { pub proc: Mutex<Option<Child>>, pub cur_model: Mutex<Option<ModelManifest>>, pub loading: AtomicBool, pub batch_cancel: AtomicBool, pub last_job: Mutex<Option<String>>, pub last_err: Mutex<String> }
impl Default for RunnerState { fn default() -> Self { Self { proc: Mutex::new(None), cur_model: Mutex::new(None), loading: AtomicBool::new(false), batch_cancel: AtomicBool::new(false), last_job: Mutex::new(None), last_err: Mutex::new(String::new()) } } }
fn log_path() -> PathBuf { config::app_data_dir().join("sd_server.log") }
fn resolve_dir(models_dir: &str, m: &ModelManifest) -> PathBuf {
  let root = Path::new(models_dir); let exact = root.join(&m.id);
  if exact.join(&m.diffusion_file).exists() { return exact; }
  if let Ok(entries) = fs::read_dir(root) { for ent in entries.flatten() { let p = ent.path();
    if p.is_dir() && p.join(&m.diffusion_file).exists() && (m.llm_file.is_empty() || p.join(&m.llm_file).exists()) && (m.vae_file.is_empty() || p.join(&m.vae_file).exists()) { return p; }
  }}
  PathBuf::from(models_dir)
}
fn build_args(cfg: &AppConfig, m: &ModelManifest, is_cli: bool) -> Vec<String> {
  let mdir = resolve_dir(&cfg.models_dir, m); let hw = hardware::detect();
  let bk = if cfg.backend_choice == "auto" { hw.recommended_backend } else { cfg.backend_choice.clone() };
  let mut a = vec!["--diffusion-model".into(), mdir.join(&m.diffusion_file).to_string_lossy().into(), "--llm".into(), mdir.join(&m.llm_file).to_string_lossy().into(), "--vae".into(), mdir.join(&m.vae_file).to_string_lossy().into()];
  if !m.uncond_file.is_empty() { a.push("--uncond-diffusion-model".into()); a.push(mdir.join(&m.uncond_file).to_string_lossy().into()); }
  if !m.llm_vision_file.is_empty() { a.push("--llm_vision".into()); a.push(mdir.join(&m.llm_vision_file).to_string_lossy().into()); }
  if !m.lora_file.is_empty() {
    let ld = if mdir.join(&m.lora_file).exists() { mdir.clone() } else if Path::new("C:/KI/sd/loras").join(&m.lora_file).exists() { PathBuf::from("C:/KI/sd/loras") } else { mdir.clone() };
    a.push("--lora-model-dir".into()); a.push(ld.to_string_lossy().into());
  }
  a.extend(m.extra_args.clone());
  a.extend(["--auto-fit".into(), "on".into(), "--vae-tiling".into(), "--flash-attn".into(), "auto".into(), "-s".into(), "-1".into()]);
  if bk == "vulkan" && hw.vram_mb <= 4096 {
    let max_gb = ((hw.ram_gb * 0.35).round() as u64).clamp(4, 14);
    a.extend(["--max-vram".into(), format!("vulkan0={}", max_gb)]);
  }
  if !is_cli { a.extend(["--steps".into(), m.steps.to_string(), "--cfg-scale".into(), m.cfg.to_string()]); }
  a
}
pub fn stop_server(st: &RunnerState) {
  if let Ok(mut g) = st.proc.lock() { if let Some(mut c) = g.take() { let _ = c.kill(); let _ = c.wait(); } }
  let _ = Command::new("taskkill").args(["/IM", "sd-server.exe", "/F"]).creation_flags(CREATE_NO_WINDOW).stdout(Stdio::null()).stderr(Stdio::null()).status();
  if let Ok(mut m) = st.cur_model.lock() { *m = None; }
  st.loading.store(false, Ordering::SeqCst);
}
pub fn start_server(st: &Arc<RunnerState>, cfg: AppConfig, m: ModelManifest) -> Result<(), String> {
  stop_server(st); std::thread::sleep(Duration::from_millis(150));
  if let Ok(mut e) = st.last_err.lock() { e.clear(); }
  let exe = engine::find_server_exe().ok_or("Engine nicht installiert")?;
  st.loading.store(true, Ordering::SeqCst);
  let mut cmd = Command::new(exe);
  cmd.args(["--listen-ip", "127.0.0.1", "--listen-port", "1235"]).args(build_args(&cfg, &m, false))
    .env("GGML_VK_ALLOW_SYSMEM_FALLBACK", "1").env("GGML_VK_MAX_NODES_PER_SUBMIT", "32").creation_flags(CREATE_NO_WINDOW);
  if let Ok(f1) = fs::File::create(log_path()) {
    if let Ok(f2) = f1.try_clone() { cmd.stdout(Stdio::from(f1)).stderr(Stdio::from(f2)); }
  }
  let child = cmd.spawn().map_err(|e| e.to_string())?;
  if let Ok(mut g) = st.proc.lock() { *g = Some(child); }
  if let Ok(mut cm) = st.cur_model.lock() { *cm = Some(m); }
  Ok(())
}
#[derive(Serialize, Clone)] pub struct ServerStatus { pub ready: bool, pub loading: bool, pub model_id: String, pub model_name: String, pub error: String }
pub async fn server_status(st: &RunnerState) -> ServerStatus {
  let (mid, mname) = st.cur_model.lock().ok().and_then(|g| g.clone()).map(|m| (m.id, m.name)).unwrap_or_default();
  let mut ex_code: Option<String> = None;
  let running = st.proc.lock().ok().and_then(|mut g| g.as_mut().map(|c| match c.try_wait() { Ok(None) => true, Ok(Some(es)) => { ex_code = Some(es.to_string()); false }, Err(_) => false })).unwrap_or(false);
  if !running {
    st.loading.store(false, Ordering::SeqCst);
    if let Some(code) = ex_code {
      if let Ok(mut g) = st.proc.lock() { *g = None; }
      if let Ok(mut cm) = st.cur_model.lock() { *cm = None; }
      let tail = fs::read_to_string(log_path()).unwrap_or_default().lines().rev().filter(|l| !l.trim().is_empty()).take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ");
      if let Ok(mut e) = st.last_err.lock() { *e = if tail.is_empty() { format!("Beendet ({})", code) } else { tail }; }
    }
    let err = st.last_err.lock().map(|e| e.clone()).unwrap_or_default();
    return ServerStatus { ready: false, loading: false, model_id: String::new(), model_name: String::new(), error: err };
  }
  let ok = reqwest::Client::builder().timeout(Duration::from_millis(900)).build().unwrap().get("http://127.0.0.1:1235/sdcpp/v1/capabilities").send().await.map(|r| r.status().is_success()).unwrap_or(false);
  if ok { st.loading.store(false, Ordering::SeqCst); }
  ServerStatus { ready: ok, loading: !ok && st.loading.load(Ordering::SeqCst), model_id: mid, model_name: mname, error: String::new() }
}
#[derive(Deserialize)] pub struct GenReq { pub prompt: String, pub negative_prompt: String, pub width: u32, pub height: u32, pub steps: u32, pub cfg: f32, pub seed: i64, pub ref_images: Vec<String> }
#[derive(Serialize, Clone)] pub struct GenResult { pub data_url: String, pub saved_path: String, pub seed: i64 }
#[derive(Serialize, Clone)] pub struct SvgResult { pub saved_path: String, pub svg_text: String }
#[derive(Deserialize)] pub struct BatchReq { pub input_dir: String, pub prompt: String, pub negative_prompt: String, pub width: u32, pub height: u32, pub steps: u32, pub cfg: f32, pub skip_existing: bool, pub max_ref_px: u32 }
#[derive(Serialize, Clone)] pub struct BatchEvt { pub done: usize, pub total: usize, pub current: String, pub status: String }
fn ts_now() -> String { format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()) }
pub async fn generate_image(app: AppHandle, st: &RunnerState, cfg: AppConfig, req: GenReq) -> Result<GenResult, String> {
  let m = st.cur_model.lock().ok().and_then(|g| g.clone()).ok_or("Kein Modell geladen")?;
  let mut prompt = req.prompt.clone(); if !m.prefix.is_empty() && !prompt.starts_with(&m.prefix[..m.prefix.len().min(10)]) { prompt = format!("{}{}", m.prefix, prompt); }
  if m.family == "ideogram4" && !prompt.trim_start().starts_with('{') {
    prompt = serde_json::json!({ "high_level_description": prompt, "style_description": { "aesthetics": "clean high quality design, sharp readable typography, detailed rendering", "lighting": "balanced natural studio lighting" }, "compositional_deconstruction": { "canvas": "clean upright layout, clear composition" } }).to_string();
  }
  let mut body = serde_json::json!({ "prompt": prompt, "width": req.width, "height": req.height, "seed": req.seed, "sample_params": { "sample_steps": req.steps, "guidance": { "txt_cfg": req.cfg } } });
  if !req.negative_prompt.is_empty() { body["negative_prompt"] = serde_json::json!(req.negative_prompt); }
  if m.supports_ref && !req.ref_images.is_empty() { body["ref_images"] = serde_json::json!(req.ref_images); }
  let cl = reqwest::Client::builder().timeout(Duration::from_secs(1800)).build().map_err(|e| e.to_string())?;
  let j: serde_json::Value = cl.post("http://127.0.0.1:1235/sdcpp/v1/img_gen").json(&body).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
  let jid = j.get("id").and_then(|x| x.as_str()).ok_or_else(|| format!("Startfehler: {}", j))?.to_string();
  if let Ok(mut lj) = st.last_job.lock() { *lj = Some(jid.clone()); }
  loop { tokio::time::sleep(Duration::from_millis(900)).await;
    let stj: serde_json::Value = cl.get(format!("http://127.0.0.1:1235/sdcpp/v1/jobs/{}", jid)).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    let done_s = stj.pointer("/progress/completed").and_then(|x| x.as_u64()).unwrap_or(0); let tot_s = stj.pointer("/progress/total").and_then(|x| x.as_u64()).unwrap_or(req.steps as u64); let status = stj.get("status").and_then(|x| x.as_str()).unwrap_or("").to_lowercase();
    let _ = app.emit("gen_progress", serde_json::json!({"completed": done_s, "total": tot_s, "status": &status})); if status == "completed" {
      let b64 = stj.pointer("/result/images/0/b64_json").and_then(|x| x.as_str()).ok_or("Kein Bild erhalten")?;
      let seed = stj.pointer("/result/images/0/seed").and_then(|x| x.as_i64()).unwrap_or(req.seed);
      let raw = B64.decode(b64).map_err(|e| e.to_string())?;
      let out_dir = PathBuf::from(&cfg.output_dir); fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
      let out_p = out_dir.join(format!("{}_{}.png", m.id, ts_now())); fs::write(&out_p, &raw).map_err(|e| e.to_string())?;
      return Ok(GenResult { data_url: format!("data:image/png;base64,{}", b64), saved_path: out_p.to_string_lossy().into(), seed });
    } else if status == "failed" || status == "cancelled" { return Err(format!("Generierung {}", status)); }
  }
}
pub async fn cancel_generate(st: &RunnerState) { let id = st.last_job.lock().ok().and_then(|g| g.clone()); if let Some(jid) = id { let _ = reqwest::Client::new().post(format!("http://127.0.0.1:1235/sdcpp/v1/jobs/{}/cancel", jid)).send().await; } }
pub fn export_jpg(cfg: &AppConfig, data_url: &str) -> Result<String, String> {
  let b64 = data_url.split(",").nth(1).ok_or("Ungueltiges Bild")?;
  let raw = B64.decode(b64).map_err(|e| e.to_string())?;
  let img = image::load_from_memory(&raw).map_err(|e| e.to_string())?;
  let out_dir = PathBuf::from(&cfg.output_dir); fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
  let out_jpg = out_dir.join(format!("bild_{}.jpg", ts_now()));
  img.to_rgb8().save(&out_jpg).map_err(|e| e.to_string())?;
  Ok(out_jpg.to_string_lossy().into())
}
pub fn export_svg(cfg: &AppConfig, data_url: &str) -> Result<SvgResult, String> {
  let b64 = data_url.split(",").nth(1).ok_or("Ungueltiges Bild")?;
  let raw = B64.decode(b64).map_err(|e| e.to_string())?;
  let tmp = config::app_data_dir().join("tmp_svg.png"); fs::write(&tmp, &raw).map_err(|e| e.to_string())?;
  let out_dir = PathBuf::from(&cfg.output_dir); fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
  let out_svg = out_dir.join(format!("vektor_{}.svg", ts_now()));
  vtracer::convert_image_to_svg(&tmp, &out_svg, vtracer::Config::default()).map_err(|e| e.to_string())?;
  let svg_text = fs::read_to_string(&out_svg).map_err(|e| e.to_string())?;
  Ok(SvgResult { saved_path: out_svg.to_string_lossy().into(), svg_text })
}
pub async fn run_batch(app: AppHandle, st: Arc<RunnerState>, cfg: AppConfig, m: ModelManifest, req: BatchReq) -> Result<usize, String> {
  stop_server(&st); st.batch_cancel.store(false, Ordering::SeqCst);
  let cli = engine::engine_dir().join("sd-cli.exe"); let cli = if cli.exists() { cli } else { PathBuf::from("C:/KI/sd/sd-cli.exe") };
  if !cli.exists() { return Err("sd-cli.exe fehlt".into()); }
  let mut files = Vec::new();
  for ent in WalkDir::new(&req.input_dir).max_depth(3).into_iter().flatten() { if ent.file_type().is_file() {
    let ext = ent.path().extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "bmp") { files.push(ent.path().to_path_buf()); }
  }}
  let total = files.len(); let out_dir = PathBuf::from(&cfg.output_dir); fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
  let mut prompt = req.prompt.clone(); if !m.prefix.is_empty() && !prompt.starts_with(&m.prefix[..m.prefix.len().min(10)]) { prompt = format!("{}{}", m.prefix, prompt); }
  if m.family == "ideogram4" && !prompt.trim_start().starts_with('{') {
    prompt = serde_json::json!({ "high_level_description": prompt, "style_description": { "aesthetics": "clean high quality design, sharp readable typography, detailed rendering", "lighting": "balanced natural studio lighting" }, "compositional_deconstruction": { "canvas": "clean upright layout, clear composition" } }).to_string();
  }
  let mut done_cnt = 0usize;
  for (idx, fp) in files.iter().enumerate() {
    if st.batch_cancel.load(Ordering::SeqCst) { let _ = app.emit("batch_progress", BatchEvt { done: idx, total, current: "Abgebrochen".into(), status: "cancelled".into() }); break; }
    let stem = fp.file_stem().and_then(|s| s.to_str()).unwrap_or("img"); let out_p = out_dir.join(format!("{}_out.png", stem));
    if req.skip_existing && out_p.exists() { done_cnt += 1; continue; }
    let _ = app.emit("batch_progress", BatchEvt { done: idx, total, current: stem.into(), status: "running".into() });
    let tmp_ref = config::app_data_dir().join("batch_ref.png");
    if let Ok(im) = image::open(fp) { let mx = req.max_ref_px.max(512); let scaled = if im.width() > mx || im.height() > mx { im.resize(mx, mx, FilterType::Triangle) } else { im }; let _ = scaled.save(&tmp_ref); }
    let mut cmd = Command::new(&cli);
    cmd.args(build_args(&cfg, &m, true)).args(["-p", &prompt, "-W", &req.width.to_string(), "-H", &req.height.to_string(), "--steps", &req.steps.to_string(), "--cfg-scale", &req.cfg.to_string(), "-o", &out_p.to_string_lossy()]);
    if !req.negative_prompt.is_empty() { cmd.args(["-n", &req.negative_prompt]); }
    if m.supports_ref && tmp_ref.exists() { cmd.args(["-r", &tmp_ref.to_string_lossy()]); }
    let ok = cmd.env("GGML_VK_ALLOW_SYSMEM_FALLBACK", "1").env("GGML_VK_MAX_NODES_PER_SUBMIT", "32").creation_flags(BELOW_NORMAL | CREATE_NO_WINDOW).stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if ok { done_cnt += 1; }
  }
  let _ = app.emit("batch_progress", BatchEvt { done: total, total, current: "Fertig".into(), status: "done".into() }); Ok(done_cnt)
}



