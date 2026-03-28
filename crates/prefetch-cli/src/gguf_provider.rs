//! GGUF FileProvider — bridges the GGUF parser into the generic provider system.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use prefetch_core::providers::{FileLayout, FileProvider, Segment};
use prefetch_gguf::layout::LayerKind;

const GGUF_MAGIC: &[u8] = &[0x47, 0x47, 0x55, 0x46];

pub struct GgufProvider;

impl FileProvider for GgufProvider {
    fn name(&self) -> &str {
        "gguf"
    }

    fn can_handle(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if ext == "gguf" {
                return true;
            }
        }
        // Check magic bytes (handles Ollama blobs with no extension)
        if let Ok(mut file) = std::fs::File::open(path) {
            let mut magic = [0u8; 4];
            if file.read_exact(&mut magic).is_ok() {
                return magic == GGUF_MAGIC;
            }
        }
        false
    }

    fn analyze(&self, path: &Path) -> anyhow::Result<FileLayout> {
        let layout = prefetch_gguf::parse_gguf(path)?;

        let mut metadata = HashMap::new();
        if let Some(name) = &layout.model_name {
            metadata.insert("model_name".to_string(), name.clone());
        }
        if let Some(arch) = &layout.architecture {
            metadata.insert("architecture".to_string(), arch.clone());
        }
        if let Some(blocks) = layout.block_count {
            metadata.insert("block_count".to_string(), blocks.to_string());
        }
        metadata.insert("tensor_count".to_string(), layout.tensors.len().to_string());

        let segments: Vec<Segment> = layout
            .layer_groups
            .iter()
            .enumerate()
            .map(|(i, group)| Segment {
                name: format!("{}", group.kind),
                offset: group.file_offset_start,
                length: group.total_bytes,
                priority: match &group.kind {
                    LayerKind::TokenEmbedding => 0,
                    LayerKind::TransformerBlock(n) => 100 + *n,
                    LayerKind::OutputNorm => 10000,
                    LayerKind::OutputHead => 10001,
                    LayerKind::Other(_) => 20000 + i as u32,
                },
            })
            .collect();

        Ok(FileLayout {
            file_size: layout.file_size,
            format_name: format!(
                "GGUF ({})",
                layout.architecture.as_deref().unwrap_or("unknown")
            ),
            segments,
            metadata,
        })
    }
}
