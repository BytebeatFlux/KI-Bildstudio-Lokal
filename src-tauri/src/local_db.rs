use crate::config;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use image::{imageops::FilterType, DynamicImage, ImageFormat};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::{fs, io::Cursor, path::Path};
use walkdir::WalkDir;
#[derive(Serialize, Clone)] pub struct LocalHit { pub path: String, pub title: String, pub tags: String, pub data_url: String, pub distance: u32 }
#[derive(Serialize, Clone)] pub struct IndexStats { pub indexed: usize, pub total: usize }
fn open_db() -> Result<Connection, String> {
  let conn = Connection::open(config::app_data_dir().join("local_images.db")).map_err(|e| e.to_string())?;
  conn.execute_batch("CREATE TABLE IF NOT EXISTS imgs (path TEXT PRIMARY KEY, mtime INTEGER, dhash INTEGER, tags TEXT, thumb TEXT);").map_err(|e| e.to_string())?;
  Ok(conn)
}
fn dhash(img: &DynamicImage) -> i64 {
  let g = img.resize_exact(9, 8, FilterType::Triangle).to_luma8(); let mut h = 0u64;
  for y in 0..8 { for x in 0..8 { if g.get_pixel(x, y)[0] > g.get_pixel(x + 1, y)[0] { h |= 1u64 << (y * 8 + x); } } }
  h as i64
}
fn thumb_url(img: &DynamicImage) -> String {
  let t = img.thumbnail(360, 360); let mut buf = Vec::new();
  let _ = t.write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg);
  format!("data:image/jpeg;base64,{}", B64.encode(&buf))
}
fn auto_tags(p: &Path) -> String {
  p.to_string_lossy().replace("\\", " ").replace("/", " ").replace("_", " ").replace("-", " ").to_lowercase()
}
pub fn index_folder(dir: &str) -> Result<IndexStats, String> {
  if dir.is_empty() || !Path::new(dir).exists() { return Ok(IndexStats { indexed: 0, total: 0 }); }
  let conn = open_db()?; let mut added = 0usize;
  for ent in WalkDir::new(dir).max_depth(6).into_iter().flatten() {
    if !ent.file_type().is_file() { continue; }
    let p = ent.path(); let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "bmp") { continue; }
    let ps = p.to_string_lossy().to_string();
    let mt = fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
    let prev: Option<i64> = conn.query_row("SELECT mtime FROM imgs WHERE path=?1", params![&ps], |r| r.get(0)).ok();
    if prev == Some(mt) { continue; }
    if let Ok(img) = image::open(p) {
      let dh = dhash(&img); let th = thumb_url(&img); let tg = auto_tags(p);
      let _ = conn.execute("INSERT OR REPLACE INTO imgs(path,mtime,dhash,tags,thumb) VALUES(?1,?2,?3,?4,?5)", params![ps, mt, dh, tg, th]);
      added += 1;
    }
  }
  let total: i64 = conn.query_row("SELECT COUNT(*) FROM imgs", [], |r| r.get(0)).unwrap_or(0);
  Ok(IndexStats { indexed: added, total: total as usize })
}
pub fn search_text(q: &str, limit: usize) -> Vec<LocalHit> {
  let Ok(conn) = open_db() else { return vec![]; }; let mut out = Vec::new();
  let words: Vec<String> = q.split_whitespace().map(|w| w.to_lowercase()).filter(|w| w.len() > 1).collect();
  let Ok(mut st) = conn.prepare("SELECT path, tags, thumb FROM imgs LIMIT 4000") else { return vec![]; };
  let Ok(rows) = st.query_map([], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?))) else { return vec![]; };
  for row in rows.flatten() {
    if words.is_empty() || words.iter().all(|w| row.1.contains(w)) {
      let title = Path::new(&row.0).file_name().unwrap_or_default().to_string_lossy().to_string();
      out.push(LocalHit { path: row.0, title, tags: row.1, data_url: row.2, distance: 0 });
      if out.len() >= limit { break; }
    }
  }
  out
}
pub fn search_similar(data_url: &str, limit: usize) -> Result<Vec<LocalHit>, String> {
  let b64 = data_url.split(",").nth(1).ok_or("Ungueltiges Bild")?;
  let raw = B64.decode(b64).map_err(|e| e.to_string())?;
  let img = image::load_from_memory(&raw).map_err(|e| e.to_string())?; let target = dhash(&img) as u64;
  let conn = open_db()?; let mut st = conn.prepare("SELECT path, dhash, tags, thumb FROM imgs").map_err(|e| e.to_string())?;
  let mut hits: Vec<LocalHit> = st.query_map([], |r| {
    let path: String = r.get(0)?; let dh: i64 = r.get(1)?; let tags: String = r.get(2)?; let th: String = r.get(3)?;
    let dist = ((dh as u64) ^ target).count_ones();
    let title = Path::new(&path).file_name().unwrap_or_default().to_string_lossy().to_string();
    Ok(LocalHit { path, title, tags, data_url: th, distance: dist })
  }).map_err(|e| e.to_string())?.flatten().collect();
  hits.sort_by_key(|h| h.distance); hits.truncate(limit); Ok(hits)
}
