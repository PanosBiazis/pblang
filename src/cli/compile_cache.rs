use std::path::{Path, PathBuf};

const CACHE_VERSION: &str = "pblang-compile-cache-v1";

pub(crate) struct CompileFingerprint {
    pub(crate) key: String,
    pub(crate) input_count: usize,
    pub(crate) input_bytes: u64,
}

struct StableHasher {
    a: u64,
    b: u64,
}

impl StableHasher {
    fn new() -> Self {
        Self {
            a: 0xcbf29ce484222325,
            b: 0x84222325cbf29ce4,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.a ^= u64::from(*byte);
            self.a = self.a.wrapping_mul(0x100000001b3);
            self.b ^= u64::from(*byte).rotate_left(1);
            self.b = self.b.wrapping_mul(0x9e3779b185ebca87);
        }
    }

    fn update_str(&mut self, value: &str) {
        self.update(value.as_bytes());
        self.update(&[0]);
    }

    fn finish(&self) -> String {
        format!("{:016x}{:016x}", self.a, self.b)
    }
}

pub(crate) fn fingerprint_for_compile(
    script_path: &Path,
    backend: &str,
    out_path: &Path,
    opt_level: u8,
    cc: Option<&str>,
    target_triple: Option<&str>,
) -> std::io::Result<CompileFingerprint> {
    let mut hasher = StableHasher::new();
    hasher.update_str(CACHE_VERSION);
    hasher.update_str(env!("CARGO_PKG_VERSION"));
    hasher.update_str(backend);
    hasher.update_str(&format!("opt={opt_level}"));
    hasher.update_str(&format!("cc={}", cc.unwrap_or("")));
    hasher.update_str(&format!("target={}", target_triple.unwrap_or("")));
    hasher.update_str(&format!("out={}", out_path.display()));
    hash_current_executable_identity(&mut hasher);

    let mut files = compile_inputs(script_path);
    files.sort();
    files.dedup();

    let mut input_count = 0usize;
    let mut input_bytes = 0u64;
    for path in files {
        if !path.is_file() {
            continue;
        }
        let display = path.to_string_lossy().replace('\\', "/");
        let bytes = std::fs::read(&path)?;
        hasher.update_str(&display);
        hasher.update(&bytes);
        input_count += 1;
        input_bytes = input_bytes.saturating_add(bytes.len() as u64);
    }

    Ok(CompileFingerprint {
        key: hasher.finish(),
        input_count,
        input_bytes,
    })
}

pub(crate) fn is_compile_cache_hit(out_path: &Path, fingerprint: &CompileFingerprint) -> bool {
    if !out_path.is_file() {
        return false;
    }
    let Ok(sidecar) = std::fs::read_to_string(cache_sidecar_path(out_path)) else {
        return false;
    };
    sidecar
        .lines()
        .any(|line| line.trim() == format!("fingerprint={}", fingerprint.key))
}

pub(crate) fn write_compile_cache(
    out_path: &Path,
    fingerprint: &CompileFingerprint,
) -> std::io::Result<()> {
    let sidecar_path = cache_sidecar_path(out_path);
    if let Some(parent) = sidecar_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        sidecar_path,
        format!(
            "{CACHE_VERSION}\nfingerprint={}\ninputs={}\nbytes={}\n",
            fingerprint.key, fingerprint.input_count, fingerprint.input_bytes
        ),
    )
}

fn cache_sidecar_path(out_path: &Path) -> PathBuf {
    let name = out_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("a.out");
    out_path.with_file_name(format!("{name}.pblang-cache"))
}

fn hash_current_executable_identity(hasher: &mut StableHasher) {
    let Ok(exe) = std::env::current_exe() else {
        hasher.update_str("exe=<unknown>");
        return;
    };
    hasher.update_str(&format!("exe={}", exe.display()));
    if let Ok(meta) = std::fs::metadata(exe) {
        hasher.update_str(&format!("exe_len={}", meta.len()));
        if let Ok(modified) = meta.modified() {
            if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
                hasher.update_str(&format!("exe_mtime_ns={}", dur.as_nanos()));
            }
        }
    }
}

fn compile_inputs(script_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    files.push(script_path.to_path_buf());

    let Some(parent) = script_path.parent() else {
        return files;
    };
    let Some(man_dir) = pblang_pm::find_manifest_dir(parent) else {
        return files;
    };

    for name in ["pblang.toml", "pblang.lock"] {
        let path = man_dir.join(name);
        if path.is_file() {
            files.push(path);
        }
    }

    for entry in walkdir::WalkDir::new(&man_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let name = entry.file_name().to_string_lossy();
            if entry.file_type().is_dir() {
                name != "target"
                    && name != ".git"
                    && name != "node_modules"
                    && name != "dist"
                    && !name.starts_with(".pblang")
            } else {
                true
            }
        })
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if is_project_input(path) {
            files.push(path.to_path_buf());
        }
    }

    files
}

fn is_project_input(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if matches!(name, "pblang.toml" | "pblang.lock") {
        return true;
    }
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("sapp" | "sap" | "bpx" | "pb" | "pbx")
    )
}
