use crate::hardware;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

#[derive(Serialize, Deserialize, Clone)]
pub struct ModelManifest { pub id: String, pub name: String, pub family: String, pub description: String, pub min_ram_gb: u32, pub steps: u32, pub cfg: f32, pub prefix: String, pub supports_ref: bool, pub diffusion_file: String, #[serde(default)] pub uncond_file: String, pub llm_file: String, pub llm_vision_file: String, pub vae_file: String, pub lora_file: String, pub extra_args: Vec<String> }
#[derive(Serialize, Clone)]
pub struct CatalogItem { pub id: String, pub name: String, pub summary: String, pub size_gb: f32, pub min_ram_gb: u32, pub license: String, pub installed: bool, pub supports_ref: bool, pub tag: String, pub speed: String }
#[derive(Serialize, Clone)]
pub struct CatalogView { pub items: Vec<CatalogItem>, pub hidden_count: usize, pub hw_summary: String }
#[derive(Serialize, Clone)]
pub struct ScanResult { pub ready: Vec<ModelManifest>, pub loose_hints: Vec<String> }
#[derive(Serialize, Clone)]
pub struct DlProgress { pub model_id: String, pub model_name: String, pub file_idx: usize, pub file_total: usize, pub percent: u8, pub detail: String }
struct FileSpec { url: &'static str, filename: &'static str }
struct CatDef { m: ModelManifest, size_gb: f32, license: &'static str, tag: &'static str, vulkan_ok: bool, files: &'static [FileSpec] }

fn catalog_defs() -> Vec<CatDef> { vec![
  CatDef { size_gb: 16.8, license: "Ideogram Non-Comm.", tag: "Design & Text Nr.1", vulkan_ok: true, m: ModelManifest { id: "ideogram-4".into(), name: "Ideogram 4.0 (Design, Poster & Text-Profi)".into(), family: "ideogram4".into(), description: "Weltklasse bei Text im Bild, Postern & Layouts (automatisches JSON-Prompting)".into(), min_ram_gb: 24, steps: 28, cfg: 4.0, prefix: "".into(), supports_ref: false, uncond_file: "ideogram4_uncond-Q4_0.gguf".into(), diffusion_file: "ideogram4-Q4_0.gguf".into(), llm_file: "Qwen3-VL-8B-Instruct-Q4_1.gguf".into(), llm_vision_file: "".into(), vae_file: "flux2-vae.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/leejet/ideogram-4-GGUF/resolve/main/ideogram4-Q4_0.gguf", filename: "ideogram4-Q4_0.gguf" },
    FileSpec { url: "https://huggingface.co/leejet/ideogram-4-GGUF/resolve/main/ideogram4_uncond-Q4_0.gguf", filename: "ideogram4_uncond-Q4_0.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/Qwen3-VL-8B-Instruct-Q4_1.gguf", filename: "Qwen3-VL-8B-Instruct-Q4_1.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/flux2-dev/resolve/main/split_files/vae/flux2-vae.safetensors", filename: "flux2-vae.safetensors" }
  ]},
  CatDef { size_gb: 11.5, license: "FLUX Non-Comm.", tag: "4-Step Pro & Edit", vulkan_ok: true, m: ModelManifest { id: "flux2-klein-9b".into(), name: "FLUX.2 Klein 9B (4-Step Pro & Edit)".into(), family: "flux2".into(), description: "Starke Detailtreue & praezises Referenz-Editing in nur 4 Steps".into(), min_ram_gb: 16, steps: 4, cfg: 1.0, prefix: "".into(), supports_ref: true, uncond_file: "".into(), diffusion_file: "flux-2-klein-9b-Q4_K_M.gguf".into(), llm_file: "qwen_3_8b_fp8mixed.safetensors".into(), llm_vision_file: "".into(), vae_file: "flux2-vae.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/FLUX.2-klein-9B-GGUF/resolve/main/flux-2-klein-9b-Q4_K_M.gguf", filename: "flux-2-klein-9b-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/flux2-klein-9B/resolve/main/split_files/text_encoders/qwen_3_8b_fp8mixed.safetensors", filename: "qwen_3_8b_fp8mixed.safetensors" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/flux2-dev/resolve/main/split_files/vae/flux2-vae.safetensors", filename: "flux2-vae.safetensors" }
  ]},
  CatDef { size_gb: 6.7, license: "Apache-2.0", tag: "Allround Detail", vulkan_ok: true, m: ModelManifest { id: "z-image-base".into(), name: "Z-Image Base (28-Step Allrounder)".into(), family: "zimage".into(), description: "Ungestillte Z-Image Basis-Variante mit hoher Motiv- und Stilvielfalt".into(), min_ram_gb: 16, steps: 28, cfg: 4.0, prefix: "".into(), supports_ref: false, uncond_file: "".into(), diffusion_file: "z-image-Q4_K_M.gguf".into(), llm_file: "Qwen3-4B-Q4_K_M.gguf".into(), llm_vision_file: "".into(), vae_file: "ae.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/Z-Image-GGUF/resolve/main/z-image-Q4_K_M.gguf", filename: "z-image-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf", filename: "Qwen3-4B-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/z_image_turbo/resolve/main/split_files/vae/ae.safetensors", filename: "ae.safetensors" }
  ]},
  CatDef { size_gb: 5.3, license: "Apache-2.0", tag: "4-Step Leichtgewicht", vulkan_ok: true, m: ModelManifest { id: "flux2-klein-4b".into(), name: "FLUX.2 Klein 4B (4-Step Schnell & Edit)".into(), family: "flux2".into(), description: "Extrem schneller 4-Step Allrounder inkl. Referenz-Edit (ab 8 GB RAM)".into(), min_ram_gb: 8, steps: 4, cfg: 1.0, prefix: "".into(), supports_ref: true, uncond_file: "".into(), diffusion_file: "flux-2-klein-4b-Q5_K_S.gguf".into(), llm_file: "Qwen3-4B-Q4_K_M.gguf".into(), llm_vision_file: "".into(), vae_file: "flux2-vae.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/FLUX.2-klein-4B-GGUF/resolve/main/flux-2-klein-4b-Q5_K_S.gguf", filename: "flux-2-klein-4b-Q5_K_S.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf", filename: "Qwen3-4B-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/flux2-dev/resolve/main/split_files/vae/flux2-vae.safetensors", filename: "flux2-vae.safetensors" }
  ]},
  CatDef { size_gb: 7.3, license: "Apache-2.0", tag: "Schnell & Text", vulkan_ok: true, m: ModelManifest { id: "z-image-turbo".into(), name: "Z-Image-Turbo (Schnell, Logos & Text)".into(), family: "zimage".into(), description: "Ultraschnell (8 Steps), scharfe Logos & Text im Bild".into(), min_ram_gb: 16, steps: 8, cfg: 1.0, prefix: "".into(), supports_ref: false, uncond_file: "".into(), diffusion_file: "z-image-turbo-Q5_K_M.gguf".into(), llm_file: "Qwen3-4B-Q4_K_M.gguf".into(), llm_vision_file: "".into(), vae_file: "ae.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/Z-Image-Turbo-GGUF/resolve/main/z-image-turbo-Q5_K_M.gguf", filename: "z-image-turbo-Q5_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf", filename: "Qwen3-4B-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/z_image_turbo/resolve/main/split_files/vae/ae.safetensors", filename: "ae.safetensors" }
  ]},
  CatDef { size_gb: 7.4, license: "Apache-2.0 / MIT", tag: "Vektor .SVG", vulkan_ok: true, m: ModelManifest { id: "z-image-vektor".into(), name: "Z-Image Vektor + .SVG Export".into(), family: "zimage_svg".into(), description: "Flat-Vektorgrafiken + direkter .SVG-Vektorexport".into(), min_ram_gb: 16, steps: 9, cfg: 1.0, prefix: "<lora:zimage-svg-lora:1> flat clean vector graphic on solid white background, crisp bold shapes, ".into(), supports_ref: false, uncond_file: "".into(), diffusion_file: "z-image-turbo-Q5_K_M.gguf".into(), llm_file: "Qwen3-4B-Q4_K_M.gguf".into(), llm_vision_file: "".into(), vae_file: "ae.safetensors".into(), lora_file: "zimage-svg-lora.safetensors".into(), extra_args: vec!["--lora-apply-mode".into(),"at_runtime".into()] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/Z-Image-Turbo-GGUF/resolve/main/z-image-turbo-Q5_K_M.gguf", filename: "z-image-turbo-Q5_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf", filename: "Qwen3-4B-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/z_image_turbo/resolve/main/split_files/vae/ae.safetensors", filename: "ae.safetensors" },
    FileSpec { url: "https://huggingface.co/JosefKuchar/svg-generator/resolve/main/zimage-svg-lora.safetensors", filename: "zimage-svg-lora.safetensors" }
  ]},
  CatDef { size_gb: 6.8, license: "FLUX Community", tag: "Foto & Referenzen", vulkan_ok: true, m: ModelManifest { id: "flux3-klein".into(), name: "FLUX.3 Image Klein (Foto & Referenzen)".into(), family: "flux3".into(), description: "Hohe Fotoqualitaet & Referenzbild-Steuerung".into(), min_ram_gb: 16, steps: 16, cfg: 1.0, prefix: "".into(), supports_ref: true, uncond_file: "".into(), diffusion_file: "flux-3-image-klein-UD-Q5_K_XL.gguf".into(), llm_file: "Qwen3-VL-4B-Instruct-Q4_K_M.gguf".into(), llm_vision_file: "".into(), vae_file: "flux2-vae.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/flux-3-image-klein-GGUF/resolve/main/flux-3-image-klein-UD-Q5_K_XL.gguf", filename: "flux-3-image-klein-UD-Q5_K_XL.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-VL-4B-Instruct-GGUF/resolve/main/Qwen3-VL-4B-Instruct-Q4_K_M.gguf", filename: "Qwen3-VL-4B-Instruct-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/flux2-dev/resolve/main/split_files/vae/flux2-vae.safetensors", filename: "flux2-vae.safetensors" }
  ]},
  CatDef { size_gb: 19.1, license: "Apache-2.0", tag: "S/W & Bearbeiten", vulkan_ok: true, m: ModelManifest { id: "qwen-image-edit-2511".into(), name: "Qwen-Image-Edit-2511 (4-Step S/W & Bearbeiten)".into(), family: "qwen_edit".into(), description: "Spezialist fuer S/W-Kolorierung & Foto-Restaurierung".into(), min_ram_gb: 24, steps: 4, cfg: 1.0, prefix: "<lora:Qwen-Image-Edit-2511-Lightning-4steps-V1.0-bf16:1> ".into(), supports_ref: true, uncond_file: "".into(), diffusion_file: "qwen-image-edit-2511-Q4_K_M.gguf".into(), llm_file: "Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf".into(), llm_vision_file: "qwen25vl7b-mmproj-F16.gguf".into(), vae_file: "qwen_image_vae.safetensors".into(), lora_file: "Qwen-Image-Edit-2511-Lightning-4steps-V1.0-bf16.safetensors".into(), extra_args: vec!["--lora-apply-mode".into(),"at_runtime".into(),"--sampling-method".into(),"euler".into(),"--flow-shift".into(),"3".into(),"--model-args".into(),"qwen_image_zero_cond_t=true".into()] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/Qwen-Image-Edit-2511-GGUF/resolve/main/qwen-image-edit-2511-Q4_K_M.gguf", filename: "qwen-image-edit-2511-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen2.5-VL-7B-Instruct-GGUF/resolve/main/Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf", filename: "Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen2.5-VL-7B-Instruct-GGUF/resolve/main/mmproj-F16.gguf", filename: "qwen25vl7b-mmproj-F16.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/Qwen-Image_ComfyUI/resolve/main/split_files/vae/qwen_image_vae.safetensors", filename: "qwen_image_vae.safetensors" },
    FileSpec { url: "https://huggingface.co/lightx2v/Qwen-Image-Edit-2511-Lightning/resolve/main/Qwen-Image-Edit-2511-Lightning-4steps-V1.0-bf16.safetensors", filename: "Qwen-Image-Edit-2511-Lightning-4steps-V1.0-bf16.safetensors" }
  ]},
  CatDef { size_gb: 17.6, license: "MiniMax License", tag: "High-Detail & Edit", vulkan_ok: true, m: ModelManifest { id: "minimax-h3-image".into(), name: "MiniMax-H3-Image (6-Step Detail & Edit)".into(), family: "minimax_h3".into(), description: "Hochdetailierte 6-Step Generierung & praezises Bild-Editing".into(), min_ram_gb: 24, steps: 6, cfg: 1.0, prefix: "".into(), supports_ref: true, uncond_file: "".into(), diffusion_file: "minimax_h3_image_pruned-UD-Q3_K_XL.gguf".into(), llm_file: "p-h3-image_text_encoder_int8_convrot.safetensors".into(), llm_vision_file: "".into(), vae_file: "minimax_h3_image_vae_fp16.safetensors".into(), lora_file: "".into(), extra_args: vec!["--sampling-method".into(),"euler".into(),"--sigmas".into(),"minimax-h3-image".into()] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/MiniMax-H3-Image-GGUF/resolve/main/minimax_h3_image_pruned-UD-Q3_K_XL.gguf", filename: "minimax_h3_image_pruned-UD-Q3_K_XL.gguf" },
    FileSpec { url: "https://huggingface.co/PrunaAI/p-h3-image-comfy/resolve/main/text_encoders/p-h3-image_text_encoder_int8_convrot.safetensors", filename: "p-h3-image_text_encoder_int8_convrot.safetensors" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/MiniMax-H3-Image/resolve/main/vae/minimax_h3_image_vae_fp16.safetensors", filename: "minimax_h3_image_vae_fp16.safetensors" }
  ]},
  CatDef { size_gb: 6.6, license: "FLUX Community", tag: "Basis-Qualitaet", vulkan_ok: true, m: ModelManifest { id: "flux3-klein-base".into(), name: "FLUX.3 Image Klein Base (Feindetails)".into(), family: "flux3".into(), description: "Ungestillte Basis-Variante fuer maximale Texturtreue".into(), min_ram_gb: 16, steps: 28, cfg: 4.0, prefix: "".into(), supports_ref: true, uncond_file: "".into(), diffusion_file: "flux-3-image-klein-base-Q4_K_M.gguf".into(), llm_file: "Qwen3-VL-4B-Instruct-Q4_K_M.gguf".into(), llm_vision_file: "".into(), vae_file: "flux2-vae.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/flux-3-image-klein-base-GGUF/resolve/main/flux-3-image-klein-base-Q4_K_M.gguf", filename: "flux-3-image-klein-base-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-VL-4B-Instruct-GGUF/resolve/main/Qwen3-VL-4B-Instruct-Q4_K_M.gguf", filename: "Qwen3-VL-4B-Instruct-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/flux2-dev/resolve/main/split_files/vae/flux2-vae.safetensors", filename: "flux2-vae.safetensors" }
  ]},
  CatDef { size_gb: 12.6, license: "Apache-2.0", tag: "Poster & Typo", vulkan_ok: true, m: ModelManifest { id: "ernie-image-turbo".into(), name: "ERNIE-Image-Turbo (8-Step Poster & Text)".into(), family: "ernie".into(), description: "Stark bei Postern, Layouts, Typografie & komplexen Prompts".into(), min_ram_gb: 16, steps: 8, cfg: 1.0, prefix: "".into(), supports_ref: false, uncond_file: "".into(), diffusion_file: "ernie-image-turbo-UD-Q4_K_M.gguf".into(), llm_file: "ministral-3-3b.safetensors".into(), llm_vision_file: "".into(), vae_file: "flux2-vae.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/ERNIE-Image-Turbo-GGUF/resolve/main/ernie-image-turbo-UD-Q4_K_M.gguf", filename: "ernie-image-turbo-UD-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/ERNIE-Image/resolve/main/text_encoders/ministral-3-3b.safetensors", filename: "ministral-3-3b.safetensors" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/ERNIE-Image/resolve/main/vae/flux2-vae.safetensors", filename: "flux2-vae.safetensors" }
  ]},
  CatDef { size_gb: 13.2, license: "Qwen Research", tag: "8-Step Allrounder", vulkan_ok: true, m: ModelManifest { id: "qwen-image-2.1-turbo".into(), name: "Qwen-Image 2.1 Turbo (8-Step Allrounder)".into(), family: "qwen21".into(), description: "Schneller 7B-Allrounder mit sauberem Text-Rendering".into(), min_ram_gb: 16, steps: 8, cfg: 1.0, prefix: "".into(), supports_ref: false, uncond_file: "".into(), diffusion_file: "qwen_image_2.1_turbo_int8_convrot.safetensors".into(), llm_file: "Qwen3-VL-8B-Instruct-Q4_1.gguf".into(), llm_vision_file: "".into(), vae_file: "qwen_image_2.1_vae_bf16.safetensors".into(), lora_file: "".into(), extra_args: vec!["--sampling-method".into(),"euler".into(),"--sigmas".into(),"qwen-image-2.1-turbo".into()] }, files: &[
    FileSpec { url: "https://huggingface.co/Comfy-Org/Qwen-Image-2.1/resolve/main/diffusion_models/qwen_image_2.1_turbo_int8_convrot.safetensors", filename: "qwen_image_2.1_turbo_int8_convrot.safetensors" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/Qwen3-VL-8B-Instruct-Q4_1.gguf", filename: "Qwen3-VL-8B-Instruct-Q4_1.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/Qwen-Image-2.1/resolve/main/vae/qwen_image_2.1_vae_bf16.safetensors", filename: "qwen_image_2.1_vae_bf16.safetensors" }
  ]},
  CatDef { size_gb: 11.2, license: "Qwen Research", tag: "Basis + Edit & RGBA", vulkan_ok: true, m: ModelManifest { id: "qwen-image-2.1-q4".into(), name: "Qwen-Image 2.1 Basis (Q4_K_M + Edit & RGBA)".into(), family: "qwen21".into(), description: "Vollwertige 7B-Basisversion mit Bild-Editing & Transparenz".into(), min_ram_gb: 16, steps: 24, cfg: 5.5, prefix: "".into(), supports_ref: true, uncond_file: "".into(), diffusion_file: "qwen-image-2.1-Q4_K_M.gguf".into(), llm_file: "Qwen3-VL-8B-Instruct-Q4_1.gguf".into(), llm_vision_file: "qwen3vl8b-mmproj-F16.gguf".into(), vae_file: "qwen_image_2.1_vae_bf16.safetensors".into(), lora_file: "".into(), extra_args: vec!["--sampling-method".into(),"euler".into()] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/Qwen-Image-2.1-GGUF/resolve/main/qwen-image-2.1-Q4_K_M.gguf", filename: "qwen-image-2.1-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/Qwen3-VL-8B-Instruct-Q4_1.gguf", filename: "Qwen3-VL-8B-Instruct-Q4_1.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/mmproj-F16.gguf", filename: "qwen3vl8b-mmproj-F16.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/Qwen-Image-2.1/resolve/main/vae/qwen_image_2.1_vae_bf16.safetensors", filename: "qwen_image_2.1_vae_bf16.safetensors" }
  ]},
  CatDef { size_gb: 10.9, license: "Qwen Research", tag: "8-Step + Edit", vulkan_ok: true, m: ModelManifest { id: "qwen-image-2.1-turbo-q4".into(), name: "Qwen-Image 2.1 Turbo GGUF (8-Step + Edit)".into(), family: "qwen21".into(), description: "Schnelles 8-Step GGUF inkl. Referenz-Edit & RGBA".into(), min_ram_gb: 16, steps: 8, cfg: 1.0, prefix: "".into(), supports_ref: true, uncond_file: "".into(), diffusion_file: "qwen-image-2.1-turbo-Q4_K_M.gguf".into(), llm_file: "Qwen3-VL-8B-Instruct-Q4_1.gguf".into(), llm_vision_file: "qwen3vl8b-mmproj-F16.gguf".into(), vae_file: "qwen_image_2.1_vae_bf16.safetensors".into(), lora_file: "".into(), extra_args: vec!["--sampling-method".into(),"euler".into(),"--sigmas".into(),"qwen-image-2.1-turbo".into()] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/Qwen-Image-2.1-Turbo-GGUF/resolve/main/qwen-image-2.1-turbo-Q4_K_M.gguf", filename: "qwen-image-2.1-turbo-Q4_K_M.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/Qwen3-VL-8B-Instruct-Q4_1.gguf", filename: "Qwen3-VL-8B-Instruct-Q4_1.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3-VL-8B-Instruct-GGUF/resolve/main/mmproj-F16.gguf", filename: "qwen3vl8b-mmproj-F16.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/Qwen-Image-2.1/resolve/main/vae/qwen_image_2.1_vae_bf16.safetensors", filename: "qwen_image_2.1_vae_bf16.safetensors" }
  ]},
  CatDef { size_gb: 13.2, license: "Apache-2.0", tag: "Basis Layout & Text", vulkan_ok: true, m: ModelManifest { id: "moss-image-base".into(), name: "MOSS-Image Base (Layout & Text-Profi)".into(), family: "moss".into(), description: "Ungestillte Basisversion fuer exakte Layouts & Text-Platzierung".into(), min_ram_gb: 24, steps: 40, cfg: 3.0, prefix: "".into(), supports_ref: false, uncond_file: "".into(), diffusion_file: "MOSS-Image-Base-UD-Q4_K_XL.gguf".into(), llm_file: "Qwen3.5-9B-Q8_0.gguf".into(), llm_vision_file: "".into(), vae_file: "flux2-vae.safetensors".into(), lora_file: "".into(), extra_args: vec!["--flow-shift".into(),"5".into()] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/MOSS-Image-Base-GGUF/resolve/main/MOSS-Image-Base-UD-Q4_K_XL.gguf", filename: "MOSS-Image-Base-UD-Q4_K_XL.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q8_0.gguf", filename: "Qwen3.5-9B-Q8_0.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/flux2-dev/resolve/main/split_files/vae/flux2-vae.safetensors", filename: "flux2-vae.safetensors" }
  ]},
  CatDef { size_gb: 13.2, license: "Apache-2.0", tag: "Layout & Szenen", vulkan_ok: true, m: ModelManifest { id: "moss-image-sft".into(), name: "MOSS-Image-SFT (Szenen & Komposition)".into(), family: "moss".into(), description: "Praezise Szenen-Komposition & saubere Bildaufteilung".into(), min_ram_gb: 24, steps: 30, cfg: 3.0, prefix: "".into(), supports_ref: false, uncond_file: "".into(), diffusion_file: "MOSS-Image-SFT-UD-Q4_K_XL.gguf".into(), llm_file: "Qwen3.5-9B-Q8_0.gguf".into(), llm_vision_file: "".into(), vae_file: "flux2-vae.safetensors".into(), lora_file: "".into(), extra_args: vec!["--flow-shift".into(),"5".into()] }, files: &[
    FileSpec { url: "https://huggingface.co/unsloth/MOSS-Image-SFT-GGUF/resolve/main/MOSS-Image-SFT-UD-Q4_K_XL.gguf", filename: "MOSS-Image-SFT-UD-Q4_K_XL.gguf" },
    FileSpec { url: "https://huggingface.co/unsloth/Qwen3.5-9B-GGUF/resolve/main/Qwen3.5-9B-Q8_0.gguf", filename: "Qwen3.5-9B-Q8_0.gguf" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/flux2-dev/resolve/main/split_files/vae/flux2-vae.safetensors", filename: "flux2-vae.safetensors" }
  ]},
  CatDef { size_gb: 24.0, license: "Qwen Research", tag: "Workstation 48GB+", vulkan_ok: true, m: ModelManifest { id: "qwen-image-2.1-bf16".into(), name: "Qwen-Image 2.1 BF16 (Workstation)".into(), family: "qwen21".into(), description: "Unkomprimierte Studio-Variante (ab 48 GB RAM)".into(), min_ram_gb: 48, steps: 40, cfg: 4.0, prefix: "".into(), supports_ref: true, uncond_file: "".into(), diffusion_file: "qwen_image_2.1_bf16.safetensors".into(), llm_file: "qwen3.5_9b_qwen_image_2.1_pe_i2i.int8_convrot.safetensors".into(), llm_vision_file: "".into(), vae_file: "qwen_image_2.1_vae_bf16.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/Comfy-Org/Qwen-Image-2.1/resolve/main/diffusion_models/qwen_image_2.1_bf16.safetensors", filename: "qwen_image_2.1_bf16.safetensors" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/Qwen-Image-2.1/resolve/main/text_encoders/qwen3.5_9b_qwen_image_2.1_pe_i2i.int8_convrot.safetensors", filename: "qwen3.5_9b_qwen_image_2.1_pe_i2i.int8_convrot.safetensors" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/Qwen-Image-2.1/resolve/main/vae/qwen_image_2.1_vae_bf16.safetensors", filename: "qwen_image_2.1_vae_bf16.safetensors" }
  ]},
  CatDef { size_gb: 24.0, license: "Apache-2.0", tag: "Workstation 48GB+", vulkan_ok: true, m: ModelManifest { id: "ernie-image-bf16".into(), name: "ERNIE-Image Full BF16 (Workstation)".into(), family: "ernie".into(), description: "Volle 16-Bit Basisvariante (ab 48 GB RAM)".into(), min_ram_gb: 48, steps: 50, cfg: 4.0, prefix: "".into(), supports_ref: false, uncond_file: "".into(), diffusion_file: "ernie-image.safetensors".into(), llm_file: "ministral-3-3b.safetensors".into(), llm_vision_file: "".into(), vae_file: "flux2-vae.safetensors".into(), lora_file: "".into(), extra_args: vec![] }, files: &[
    FileSpec { url: "https://huggingface.co/Comfy-Org/ERNIE-Image/resolve/main/diffusion_models/ernie-image.safetensors", filename: "ernie-image.safetensors" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/ERNIE-Image/resolve/main/text_encoders/ministral-3-3b.safetensors", filename: "ministral-3-3b.safetensors" },
    FileSpec { url: "https://huggingface.co/Comfy-Org/ERNIE-Image/resolve/main/vae/flux2-vae.safetensors", filename: "flux2-vae.safetensors" }
  ]}
] }
fn find_subfolders(root: &Path) -> Vec<PathBuf> {
  let mut dirs = Vec::new();
  if let Ok(entries) = fs::read_dir(root) { for ent in entries.flatten() { let p = ent.path(); if p.is_dir() { dirs.push(p); } } }
  dirs
}
fn folder_ready(dir: &Path, m: &ModelManifest) -> bool {
  let req = [&m.diffusion_file, &m.uncond_file, &m.llm_file, &m.vae_file, &m.llm_vision_file];
  let base_ok = req.iter().all(|f| f.is_empty() || dir.join(f).exists());
  let lora_ok = m.lora_file.is_empty() || dir.join(&m.lora_file).exists() || Path::new("C:/KI/sd/loras").join(&m.lora_file).exists();
  base_ok && lora_ok
}
fn fits_hw(c: &CatDef, hw: &hardware::HardwareInfo) -> bool {
  let ram_ok = hw.ram_gb + 1.5 >= c.m.min_ram_gb as f64;
  let bk_ok = if hw.recommended_backend == "vulkan" || hw.recommended_backend == "cpu" { c.vulkan_ok } else { true };
  ram_ok && bk_ok
}
pub fn get_catalog(models_dir: &str) -> CatalogView {
  let hw = hardware::detect(); let root = Path::new(models_dir);
  let subs = if models_dir.is_empty() || !root.exists() { vec![] } else { find_subfolders(root) };
  let all = catalog_defs(); let total = all.len();
  let items: Vec<CatalogItem> = all.into_iter().filter(|c| fits_hw(c, &hw)).map(|c| {
    let inst = subs.iter().any(|p| folder_ready(p, &c.m));
    let spd = if c.m.steps <= 8 { format!("Sehr schnell ({} Steps)", c.m.steps) } else if c.m.steps <= 16 { format!("Schnell ({} Steps)", c.m.steps) } else { format!("Detail ({} Steps)", c.m.steps) };
    CatalogItem { id: c.m.id, name: c.m.name, summary: c.m.description, size_gb: c.size_gb, min_ram_gb: c.m.min_ram_gb, license: c.license.into(), installed: inst, supports_ref: c.m.supports_ref, tag: c.tag.into(), speed: spd }
  }).collect();
  let hidden_count = total.saturating_sub(items.len());
  let hw_summary = format!("{} ({}) | {:.1} GB RAM", hw.gpu_name, hw.recommended_backend.to_uppercase(), hw.ram_gb);
  CatalogView { items, hidden_count, hw_summary }
}
pub fn scan_models(models_dir: &str) -> ScanResult {
  let hw = hardware::detect(); let mut ready = Vec::new(); let mut loose_hints = Vec::new(); let root = Path::new(models_dir);
  if models_dir.is_empty() || !root.exists() { return ScanResult { ready, loose_hints }; }
  let subs = find_subfolders(root);
  for p in &subs {
    let mf = p.join("model.json");
    if mf.exists() { if let Ok(txt) = fs::read_to_string(&mf) { if let Ok(m) = serde_json::from_str::<ModelManifest>(&txt) {
      if folder_ready(p, &m) { if hw.ram_gb + 1.5 >= m.min_ram_gb as f64 { ready.push(m); } }
      else { loose_hints.push(format!("Unvollstaendig: {}", m.name)); }
    } } }
  }
  for def in catalog_defs() {
    if fits_hw(&def, &hw) && !ready.iter().any(|x| x.id == def.m.id) {
      if let Some(sub) = subs.iter().find(|p| folder_ready(p, &def.m)) {
        let mut m = def.m; let sub_n = sub.file_name().unwrap_or_default().to_string_lossy();
        m.description = format!("{} (Unterordner: {})", m.description, sub_n); ready.push(m);
      }
    }
  }
  if let Ok(entries) = fs::read_dir(root) { for ent in entries.flatten() { let p = ent.path();
    if p.is_file() { let n = p.file_name().unwrap_or_default().to_string_lossy().to_string();
      if n.ends_with(".gguf") || n.ends_with(".safetensors") { loose_hints.push(format!("Lose Datei im Hauptordner (bitte in Unterordner verschieben): {}", n)); }
    }
  }}
  ScanResult { ready, loose_hints }
}
pub async fn download_model(app: AppHandle, models_dir: String, model_id: String) -> Result<ModelManifest, String> {
  let def = catalog_defs().into_iter().find(|c| c.m.id == model_id).ok_or("Unbekanntes Modell")?;
  let target = PathBuf::from(&models_dir).join(&def.m.id); fs::create_dir_all(&target).map_err(|e| e.to_string())?;
  let client = reqwest::Client::builder().user_agent("BytebeatFluxStudio/0.1").build().map_err(|e| e.to_string())?;
  let file_total = def.files.len();
  for (idx, fs_item) in def.files.iter().enumerate() {
    let out_p = target.join(fs_item.filename); if out_p.exists() { continue; }
    let mut linked = false;
    for sib in find_subfolders(Path::new(&models_dir)) {
      let cand = sib.join(fs_item.filename);
      if cand.exists() && !sib.join(format!("{}.part", fs_item.filename)).exists() && fs::hard_link(&cand, &out_p).is_ok() { linked = true; break; }
    }
    if !linked {
      let lc = Path::new("C:/KI/sd/loras").join(fs_item.filename);
      if lc.exists() && fs::hard_link(&lc, &out_p).is_ok() { linked = true; }
    }
    if linked {
      let _ = app.emit("model_progress", DlProgress { model_id: model_id.clone(), model_name: def.m.name.clone(), file_idx: idx + 1, file_total, percent: 99, detail: "Hardlink erstellt (0 MB)".into() });
      continue;
    }
    let part_p = target.join(format!("{}.part", fs_item.filename));
    let existing = fs::metadata(&part_p).map(|m| m.len()).unwrap_or(0);
    let mut req = client.get(fs_item.url); if existing > 0 { req = req.header("Range", format!("bytes={}-", existing)); }
    let res = req.send().await.map_err(|e| e.to_string())?;
    if !res.status().is_success() { return Err(format!("HTTP {} bei {}", res.status(), fs_item.filename)); }
    let total = existing + res.content_length().unwrap_or(0); let mut got = existing;
    let mut file = tokio::fs::OpenOptions::new().create(true).append(existing > 0).write(true).open(&part_p).await.map_err(|e| e.to_string())?;
    let mut st = res.bytes_stream(); let mut last_emit = std::time::Instant::now();
    while let Some(ch) = st.next().await {
      let b = ch.map_err(|e| e.to_string())?; file.write_all(&b).await.map_err(|e| e.to_string())?; got += b.len() as u64;
      if last_emit.elapsed().as_millis() >= 200 || (total > 0 && got >= total) {
        last_emit = std::time::Instant::now();
        let pct = if total > 0 { ((got * 100) / total).min(99) as u8 } else { 50 };
        let detail = if total >= 1073741824 { format!("{:.2} / {:.2} GB", got as f64 / 1073741824.0, total as f64 / 1073741824.0) } else if total > 0 { format!("{:.0} / {:.0} MB", got as f64 / 1048576.0, total as f64 / 1048576.0) } else { format!("{:.0} MB", got as f64 / 1048576.0) };
        let _ = app.emit("model_progress", DlProgress { model_id: model_id.clone(), model_name: def.m.name.clone(), file_idx: idx + 1, file_total, percent: pct, detail });
      }
    }
    file.flush().await.map_err(|e| e.to_string())?; drop(file); fs::rename(&part_p, &out_p).map_err(|e| e.to_string())?;
  }
  let m_json = serde_json::to_string_pretty(&def.m).map_err(|e| e.to_string())?;
  fs::write(target.join("model.json"), m_json).map_err(|e| e.to_string())?;
  let _ = app.emit("model_progress", DlProgress { model_id, model_name: def.m.name.clone(), file_idx: file_total, file_total, percent: 100, detail: "Einsatzbereit".into() });
  Ok(def.m)
}



