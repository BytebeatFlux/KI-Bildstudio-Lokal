use serde::Serialize;
use sysinfo::System;
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
#[derive(Serialize, Clone)]
pub struct HardwareInfo { pub gpu_name: String, pub vendor: String, pub recommended_backend: String, pub vram_mb: u64, pub ram_gb: f64 }
pub fn detect() -> HardwareInfo {
  let mut sys = System::new(); sys.refresh_memory();
  let ram_gb = ((sys.total_memory() as f64) / 1073741824.0 * 10.0).round() / 10.0;
  let mut gpu_name = "Keine dedizierte GPU (CPU)".to_string();
  let mut vendor = "CPU".to_string(); let mut vram_mb = 0u64; let mut best_score = 0u64;
  unsafe { if let Ok(fac) = CreateDXGIFactory1::<IDXGIFactory1>() {
    let mut i = 0;
    while let Ok(ad) = fac.EnumAdapters1(i) { i += 1;
      if let Ok(desc) = ad.GetDesc1() {
        if (desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32)) != 0 { continue; }
        let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(128);
        let name = String::from_utf16_lossy(&desc.Description[..len]).trim().to_string();
        let mb = (desc.DedicatedVideoMemory as u64) / 1048576;
        let v_bonus = if desc.VendorId == 0x10DE { 100000 } else if desc.VendorId == 0x1002 { 50000 } else { 10000 };
        let score = mb + v_bonus;
        if score > best_score { best_score = score; gpu_name = name; vram_mb = mb;
          vendor = match desc.VendorId { 0x10DE => "NVIDIA", 0x1002 => "AMD", 0x8086 => "Intel", _ => "GPU" }.to_string();
        }
      }
    }
  }}
  let recommended_backend = if vendor == "NVIDIA" { "cuda12".to_string() } else if vendor == "AMD" || vendor == "Intel" { "vulkan".to_string() } else { "cpu".to_string() };
  HardwareInfo { gpu_name, vendor, recommended_backend, vram_mb, ram_gb }
}
