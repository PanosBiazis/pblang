use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

// ──────────────────────────────────────────────────────────────
// Recognized source/module file extensions
// ──────────────────────────────────────────────────────────────

/// File extensions (without the leading dot) that the toolchain treats as
/// pblang modules/scripts. `.sapp`/`.sap` are the historical source
/// extensions and `.bpx` the bytecode container; `.pbx`/`.pb`/`.bp` are
/// additional accepted extensions.
pub const MODULE_EXTENSIONS: &[&str] = &["sapp", "sap", "bpx", "pbx", "pb", "bp"];

/// True if `ext` (an extension without the dot, e.g. from `Path::extension`)
/// is a recognized pblang module extension.
pub fn is_module_ext(ext: &str) -> bool {
    MODULE_EXTENSIONS.contains(&ext)
}

/// True if `name` ends with a recognized `.<ext>` module suffix.
pub fn has_module_extension(name: &str) -> bool {
    MODULE_EXTENSIONS.iter().any(|ext| {
        name.len() > ext.len() + 1
            && name.ends_with(ext)
            && name.as_bytes()[name.len() - ext.len() - 1] == b'.'
    })
}

// ──────────────────────────────────────────────────────────────
// Manifest structs
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub package: Option<PackageSection>,
    #[serde(default)]
    pub dependencies: std::collections::BTreeMap<String, DependencySpec>,
    /// Optional `[workspace]` section. When present, this manifest acts as a
    /// workspace root and `members` lists the member package directories.
    /// A manifest may carry both `[package]` and `[workspace]` (a "hybrid"
    /// workspace where the root is itself a member) or only `[workspace]`
    /// (a "virtual" workspace with no root package of its own).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceSection>,
}

/// `[workspace]` table contents.
///
/// Members are path patterns relative to the workspace root. Each pattern is
/// either a literal directory (e.g. `"tools/foo"`) or a trailing-`*` wildcard
/// (e.g. `"crates/*"`) which expands to every immediate subdirectory of the
/// stem that contains its own `pblang.toml`. `exclude` is consulted **after**
/// expansion and uses the same syntax (literal or trailing-`*`); any member
/// matched by an exclude pattern is dropped.
///
/// Glob expansion deliberately does **not** support arbitrary `**` or middle-
/// segment wildcards. The two supported forms cover the Cargo-style layout
/// that pblang workspaces actually need; richer globbing would pull in the
/// `glob` crate for marginal value.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WorkspaceSection {
    /// Member path patterns (literal or trailing-`*`).
    #[serde(default)]
    pub members: Vec<String>,
    /// Patterns to drop after member expansion.
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct PackageSection {
    pub name: Option<String>,
    pub version: Option<String>,
    /// Custom package entry point (relative path inside the package root).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// Human-readable description for the registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Target OS platform hint (e.g. "linux", "windows", "macos", "any").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_platform: Option<String>,
    /// Minimum pblang runtime version required (semver-ish, e.g. "0.3.0").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_runtime: Option<String>,
}

/// Dependency specification in pblang.toml.
/// Supports three forms:
///   `"dep" = "1.2.0"`          → version string (registry packages starting with @)
///   `"dep" = { path = "..." }` → local path dep
///   `"dep" = { git = "..." }`  → git dep (future support)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DependencySpec {
    /// Simple version string (used for registry deps like `"@pblang/http" = "1.0.0"`).
    Version(String),
    /// Detailed table form.
    Detailed {
        #[serde(default)]
        path: Option<String>,
        /// Additional candidate paths, tried in order after `path` when resolving a
        /// local path dependency (first existing wins). Lets one committed manifest
        /// resolve the same dependency across different checkout layouts (e.g. a Linux
        /// `../../pl-dev/pblang/stdlib` vs a Windows `../pblang/stdlib`) without any
        /// per-machine edits. Each entry also supports `${VAR}` env expansion and a
        /// leading `~`.
        #[serde(default)]
        paths: Option<Vec<String>>,
        #[serde(default)]
        git: Option<String>,
        #[serde(default)]
        rev: Option<String>,
        #[serde(default)]
        version: Option<String>,
    },
}

impl DependencySpec {
    /// Returns the local path, if any.
    pub fn path(&self) -> Option<&str> {
        match self {
            DependencySpec::Version(_) => None,
            DependencySpec::Detailed { path, .. } => path.as_deref(),
        }
    }

    /// All local path candidates in priority order: `path` first (if set), then any
    /// `paths` fallbacks. The resolver tries each until one exists on disk, so a
    /// single manifest can resolve across machine layouts.
    pub fn path_candidates(&self) -> Vec<&str> {
        let mut out = Vec::new();
        if let DependencySpec::Detailed { path, paths, .. } = self {
            if let Some(p) = path.as_deref() {
                out.push(p);
            }
            if let Some(list) = paths {
                for p in list {
                    out.push(p.as_str());
                }
            }
        }
        out
    }

    /// Returns the git URL, if any.
    pub fn git(&self) -> Option<&str> {
        match self {
            DependencySpec::Version(_) => None,
            DependencySpec::Detailed { git, .. } => git.as_deref(),
        }
    }

    /// Returns the version/range string, if any.
    pub fn version_str(&self) -> Option<&str> {
        match self {
            DependencySpec::Version(v) => Some(v.as_str()),
            DependencySpec::Detailed { version, .. } => version.as_deref(),
        }
    }

    pub fn rev(&self) -> Option<&str> {
        match self {
            DependencySpec::Version(_) => None,
            DependencySpec::Detailed { rev, .. } => rev.as_deref(),
        }
    }
}

/// Expand `${VAR}` environment references inside a string. Unknown variables expand
/// to empty. UTF-8 safe; a `${` with no closing `}` is left verbatim.
fn expand_env_vars(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find("${") {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + 2..];
        if let Some(end) = after.find('}') {
            let var = &after[..end];
            if let Ok(val) = std::env::var(var) {
                out.push_str(&val);
            }
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[pos..]);
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

/// Expand a dependency path string: a leading `~` becomes the home directory and
/// `${VAR}` references are substituted. Best-effort and cross-platform.
fn expand_dep_path(raw: &str) -> String {
    let mut s = raw.to_string();
    if s == "~" || s.starts_with("~/") || s.starts_with("~\\") {
        if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            s = format!("{}{}", home.to_string_lossy(), &s[1..]);
        }
    }
    expand_env_vars(&s)
}

// ──────────────────────────────────────────────────────────────
// Lockfile structs
// ──────────────────────────────────────────────────────────────

pub const LOCKFILE_CONTENT_HASH_ALGORITHM: &str = "pblang-package-tree-v2";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Lockfile {
    pub manifest_sha256: String,
    #[serde(default)]
    pub content_hash_algorithm: String,
    pub dependencies: std::collections::BTreeMap<String, LockedDep>,
}

impl Lockfile {
    /// Returns true if this lockfile uses the v2 format (has extended per-dep fields).
    pub fn is_v2(&self) -> bool {
        self.dependencies.values().any(|d| {
            d.name.is_some()
                || d.version.is_some()
                || d.source.is_some()
                || d.resolved.is_some()
                || d.fetch_url.is_some()
        })
    }

    /// Migrate a v1 lockfile (path+sha256 only) to v2 by enriching with
    /// available metadata.  Path-only deps stay path-only; registry deps get
    /// source="registry".
    pub fn migrate_to_v2(&mut self, manifest_dir: &Path) -> Result<(), String> {
        for (_name, dep) in self.dependencies.iter_mut() {
            if dep.source.is_some() {
                continue; // already v2
            }
            // Heuristic: if path looks like a cache entry (hash dir name), it's a registry dep.
            let dep_path = Path::new(&dep.path);
            if let Some(parent) = dep_path.parent() {
                if parent.ends_with("cache") {
                    dep.source = Some("registry".to_string());
                }
            }
            // Try to extract name/version from the path
            if let Some(fname) = dep_path.file_name().and_then(|n| n.to_str()) {
                // Registry cache entries are named by archive sha256
                if fname.len() == 64 && fname.chars().all(|c| c.is_ascii_hexdigit()) {
                    dep.source = Some("registry".to_string());
                }
            }
            // If the path is inside manifest dir, it's a local path dep
            if dep_path.starts_with(manifest_dir) {
                dep.source.get_or_insert_with(|| "path".to_string());
            }
        }
        Ok(())
    }
}

impl LockedDep {
    /// Check if this entry has the extended v2 fields populated.
    pub fn is_v2_entry(&self) -> bool {
        self.name.is_some()
            || self.version.is_some()
            || self.source.is_some()
            || self.resolved.is_some()
            || self.fetch_url.is_some()
    }

    /// Set the v2 metadata fields on this lock entry.
    pub fn set_v2_metadata(
        &mut self,
        name: Option<String>,
        version: Option<String>,
        source: Option<String>,
        resolved: Option<String>,
        fetch_url: Option<String>,
    ) {
        self.name = name;
        self.version = version;
        self.source = source;
        self.resolved = resolved;
        self.fetch_url = fetch_url;
    }
}

/// Read a current hash-v2 lockfile and enrich legacy per-dependency metadata.
/// Lockfiles without an explicit content-hash algorithm require an auditable
/// refresh and are rejected by `parse_lockfile` before metadata migration.
pub fn read_lockfile_with_migration(
    lock_path: &Path,
    manifest_dir: &Path,
) -> Result<(Lockfile, bool), String> {
    let toml_str = std::fs::read_to_string(lock_path)
        .map_err(|e| format!("read lockfile '{}': {e}", lock_path.display()))?;
    let mut lock = parse_lockfile(&toml_str)?;
    if lock.is_v2() {
        return Ok((lock, false));
    }
    lock.migrate_to_v2(manifest_dir)?;
    Ok((lock, true))
}

/// Lockfile v2: path + sha256 required; name/version/source/resolved/fetch_url optional.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LockedDep {
    pub path: String,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_url: Option<String>,
    /// Tarball/archive checksum when `sha256` stores unpacked tree content hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_sha256: Option<String>,
}

// ──────────────────────────────────────────────────────────────
// Registry types
// ──────────────────────────────────────────────────────────────

/// Registry index: packages -> version -> { url, sha256, description, dependencies }.
pub type RegistryIndex =
    std::collections::HashMap<String, std::collections::HashMap<String, RegistryRelease>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryRelease {
    pub url: String,
    #[serde(default)]
    pub sha256: Option<String>,
    /// Human-readable description for the package.
    #[serde(default)]
    pub description: Option<String>,
    /// Package dependencies as name -> version mapping.
    #[serde(default)]
    pub dependencies: std::collections::BTreeMap<String, String>,
}

/// Search result returned by `search_registry`.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub name: String,
    pub version: String,
    pub description: String,
}

/// The source location of a registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrySource {
    /// GitHub-hosted registry: owner/repo pair.
    GitHub { owner: String, repo: String },
    /// GitLab-hosted registry: optional custom host, owner, repo.
    GitLab {
        host: Option<String>,
        owner: String,
        repo: String,
    },
    /// Direct HTTP(S) registry URL.
    DirectHttp { url: String },
}

impl RegistrySource {
    /// Return the base URL for API calls.
    pub fn base_url(&self) -> String {
        match self {
            RegistrySource::GitHub { owner, repo } => {
                format!("https://api.github.com/repos/{owner}/{repo}/releases")
            }
            RegistrySource::GitLab { host, owner, repo } => {
                let h = host.as_deref().unwrap_or("https://gitlab.com");
                format!("{h}/api/v4/projects/{owner}%2F{repo}")
            }
            RegistrySource::DirectHttp { url } => url.clone(),
        }
    }

    /// Construct from a URL string, attempting to detect the source type.
    pub fn from_url(url: &str) -> Self {
        if url.starts_with("file://") {
            return RegistrySource::DirectHttp {
                url: url.to_string(),
            };
        }
        if let Some(rest) = url.strip_prefix("https://github.com/") {
            let parts: Vec<&str> = rest.trim_end_matches('/').split('/').collect();
            if parts.len() >= 2 {
                return RegistrySource::GitHub {
                    owner: parts[0].to_string(),
                    repo: parts[1].to_string(),
                };
            }
        }
        if let Some(rest) = url.strip_prefix("https://gitlab.com/") {
            let parts: Vec<&str> = rest.trim_end_matches('/').split('/').collect();
            if parts.len() >= 2 {
                return RegistrySource::GitLab {
                    host: None,
                    owner: parts[0].to_string(),
                    repo: parts[1].to_string(),
                };
            }
        }
        RegistrySource::DirectHttp {
            url: url.trim_end_matches('/').to_string(),
        }
    }
}

/// Structured package metadata as returned by a registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub dependencies: std::collections::BTreeMap<String, String>,
    /// SHA-256 checksum of the package tarball.
    #[serde(default)]
    pub checksum: Option<String>,
    /// URL to download the package tarball.
    #[serde(default)]
    pub download_url: Option<String>,
}

/// A client for interacting with a pblang package registry.
pub struct RegistryClient {
    base_url: String,
    token: Option<String>,
    allow_unsigned_registry_dev: bool,
}

impl RegistryClient {
    /// Create a new `RegistryClient` for the given registry base URL.
    pub fn new(base_url: String) -> Self {
        RegistryClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: None,
            allow_unsigned_registry_dev: unsigned_registry_dev_enabled(),
        }
    }

    /// Create a new `RegistryClient` with an auth token.
    pub fn with_token(base_url: String, token: String) -> Self {
        RegistryClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            token: Some(token),
            allow_unsigned_registry_dev: unsigned_registry_dev_enabled(),
        }
    }

    /// Explicit development-only compatibility for unsigned registries.
    /// Never enable this for release, CI publication, or multi-user installs.
    pub fn allow_unsigned_registry_for_development(&mut self) {
        self.allow_unsigned_registry_dev = true;
    }

    /// Set or replace the auth token.
    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    /// Return the base URL of this client.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Fetch package metadata and tarball for a specific name and version.
    ///
    /// Returns `(PackageMetadata, tarball_bytes)` on success.
    pub fn fetch(
        &self,
        package_name: &str,
        version: &str,
    ) -> Result<(PackageMetadata, Vec<u8>), String> {
        let index_url = format!("{}/index.json", self.base_url);
        let index_bytes =
            fetch_authenticated_registry_index(&index_url, self.allow_unsigned_registry_dev)?;
        let index = parse_registry_index(&index_bytes)?;

        let releases = index
            .get(package_name)
            .ok_or_else(|| format!("registry: package '{}' not in index", package_name))?;
        let release = releases.get(version).ok_or_else(|| {
            format!(
                "registry: version {} not found for '{}'",
                version, package_name
            )
        })?;

        let expected = release.sha256.as_deref().ok_or_else(|| {
            format!(
                "registry: sha256 required for {} {} but index has no sha256",
                package_name, version
            )
        })?;

        let release_fetch_url = resolve_registry_release_url(&index_url, &release.url);
        let tarball_bytes = fetch_url_with_limit(
            &release_fetch_url,
            MAX_REGISTRY_ARCHIVE_BYTES,
            "registry archive",
        )?;

        let actual = hex::encode(Sha256::digest(&tarball_bytes));
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(format!(
                "registry: checksum mismatch for {} {}: expected {} got {}",
                package_name, version, expected, actual
            ));
        }

        let metadata = PackageMetadata {
            name: package_name.to_string(),
            version: version.to_string(),
            description: release.description.clone().unwrap_or_default(),
            dependencies: release.dependencies.clone(),
            checksum: release.sha256.clone(),
            download_url: Some(release_fetch_url),
        };

        Ok((metadata, tarball_bytes))
    }

    /// Search the registry for packages matching `query`.
    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        search_registry(&self.base_url, query)
    }

    /// Publish a package directory to this registry.
    pub fn publish(&self, package_dir: &Path) -> Result<(), String> {
        let token = self.token.as_deref().unwrap_or("");
        publish_package(&self.base_url, package_dir, token)
    }

    /// Install a package into the local cache, returning the cache path and metadata.
    pub fn install(
        &self,
        package_name: &str,
        version: &str,
        cache_dir: &Path,
        _require_sha: bool,
    ) -> Result<(PathBuf, PackageMetadata), String> {
        let (metadata, tarball_bytes) = self.fetch(package_name, version)?;

        let archive_sha256 = hex::encode(Sha256::digest(&tarball_bytes));
        let (cache_entry, _) =
            install_verified_registry_archive(cache_dir, &archive_sha256, &tarball_bytes)?;

        let mut meta = metadata;
        meta.checksum = Some(archive_sha256);

        Ok((cache_entry, meta))
    }
}

// ──────────────────────────────────────────────────────────────
// Network helpers
// ──────────────────────────────────────────────────────────────

/// Fetch bytes from URL (file:// or http(s)://).
/// Fetch bytes from a URL.
///
/// Supports `file://` (absolute local paths) and `http(s)://`.
///
/// BUG FIX: The original implementation called `trim_start_matches('/')` which
/// stripped ALL leading slashes from absolute paths, turning `/tmp/foo/index.json`
/// into the relative path `tmp/foo/index.json`. This caused all file:// registry
/// tests to fail. Now the path is preserved verbatim after the `file://` prefix.
pub fn fetch_url(url: &str) -> Result<Vec<u8>, String> {
    fetch_url_with_limit(url, MAX_REGISTRY_ARCHIVE_BYTES, "registry response")
}

fn fetch_url_with_limit(url: &str, limit: usize, label: &str) -> Result<Vec<u8>, String> {
    if url.starts_with("file:") {
        let path = local_file_registry_path(url)
            .ok_or_else(|| format!("{label}: file URL is not an absolute local path"))?;
        let file =
            std::fs::File::open(&path).map_err(|e| format!("read file {}: {e}", path.display()))?;
        let size = file
            .metadata()
            .map_err(|e| format!("stat file {}: {e}", path.display()))?
            .len();
        if size > limit as u64 {
            return Err(format!(
                "{label}: payload of {size} bytes exceeds cap of {limit} bytes"
            ));
        }
        let mut bytes = Vec::with_capacity(size as usize);
        file.take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("read file {}: {e}", path.display()))?;
        if bytes.len() > limit {
            return Err(format!(
                "{label}: payload exceeds cap of {limit} bytes while reading"
            ));
        }
        return Ok(bytes);
    }
    if url.starts_with("http://")
        && std::env::var("PBLANG_ALLOW_INSECURE_HTTP").ok().as_deref() != Some("1")
    {
        return Err(
            "insecure registry URL rejected: http:// requires PBLANG_ALLOW_INSECURE_HTTP=1"
                .to_string(),
        );
    }
    // Registry fetches never follow redirects. A redirect changes the trust
    // boundary after policy has classified the original URL, so callers must
    // resolve and authorize the final URL explicitly.
    let agent = ureq::AgentBuilder::new().redirects(0).build();
    let resp = agent
        .get(url)
        .call()
        .map_err(|e| format!("GET {}: {e}", url))?;
    if !(200..300).contains(&resp.status()) {
        return Err(format!(
            "GET {}: unexpected HTTP status {} (redirects are not allowed)",
            url,
            resp.status()
        ));
    }
    let mut bytes = Vec::new();
    resp.into_reader()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read body: {e}"))?;
    if bytes.len() > limit {
        return Err(format!(
            "{label}: payload exceeds cap of {limit} bytes while reading"
        ));
    }
    Ok(bytes)
}

/// Default cap on registry-index JSON payload size (10 MB). Bounds memory
/// consumption when fetching from untrusted/networked registries.
pub const MAX_REGISTRY_INDEX_BYTES: usize = 10 * 1024 * 1024;
/// Detached publisher signature sidecars are small, bounded metadata.
pub const MAX_REGISTRY_SIGNATURE_BYTES: usize = 16 * 1024;
/// Maximum accepted compressed registry package size (64 MiB).
pub const MAX_REGISTRY_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
/// Maximum total regular-file content extracted from one package (256 MiB).
pub const MAX_REGISTRY_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum size of one extracted regular file (64 MiB).
pub const MAX_REGISTRY_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
/// Maximum number of archive entries, including directories.
pub const MAX_REGISTRY_ENTRIES: usize = 4096;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryIndexSignature {
    pub schema: String,
    pub publisher: String,
    pub key_id: String,
    pub algorithm: String,
    pub index_sha256: String,
    pub signature: String,
}

fn trusted_registry_keys_from_bytes(
    bytes: &[u8],
) -> Result<std::collections::BTreeMap<String, String>, String> {
    if bytes.len() > MAX_REGISTRY_SIGNATURE_BYTES {
        return Err("registry trust store exceeds 16 KiB cap".to_string());
    }
    let keys: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(bytes).map_err(|e| format!("registry trust store JSON: {e}"))?;
    if keys.is_empty() {
        return Err("registry trust store contains no publisher keys".to_string());
    }
    Ok(keys)
}

/// Verify a detached Ed25519 registry-index signature against an operator-owned
/// publisher trust store. The signature is over the exact index bytes, so even
/// whitespace changes require a new publisher signature.
pub fn verify_registry_index_signature(
    index_bytes: &[u8],
    signature_bytes: &[u8],
    trust_store_bytes: &[u8],
) -> Result<String, String> {
    if signature_bytes.len() > MAX_REGISTRY_SIGNATURE_BYTES {
        return Err("registry signature sidecar exceeds 16 KiB cap".to_string());
    }
    let sidecar: RegistryIndexSignature = serde_json::from_slice(signature_bytes)
        .map_err(|e| format!("registry signature JSON: {e}"))?;
    if sidecar.schema != "pblang-registry-signature-v1" || sidecar.algorithm != "ed25519" {
        return Err("registry signature uses an unsupported schema or algorithm".to_string());
    }
    if sidecar.publisher.is_empty() || sidecar.publisher.len() > 128 {
        return Err("registry signature has an invalid publisher identity".to_string());
    }
    let actual_index_sha = hex::encode(Sha256::digest(index_bytes));
    if !sidecar.index_sha256.eq_ignore_ascii_case(&actual_index_sha) {
        return Err("registry signature does not match the fetched index digest".to_string());
    }
    let trusted = trusted_registry_keys_from_bytes(trust_store_bytes)?;
    let public_key_hex = trusted
        .get(&sidecar.publisher)
        .ok_or_else(|| format!("registry publisher '{}' is not trusted", sidecar.publisher))?;
    let public_key = hex::decode(public_key_hex)
        .map_err(|_| "registry trust store contains malformed public key hex".to_string())?;
    if public_key.len() != 32 {
        return Err("registry Ed25519 public key must be exactly 32 bytes".to_string());
    }
    let actual_key_id = hex::encode(Sha256::digest(&public_key));
    if !sidecar.key_id.eq_ignore_ascii_case(&actual_key_id) {
        return Err(
            "registry signature key id does not match the trusted publisher key".to_string(),
        );
    }
    let signature = base64::engine::general_purpose::STANDARD
        .decode(sidecar.signature.as_bytes())
        .map_err(|_| "registry signature is not valid base64".to_string())?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &public_key)
        .verify(index_bytes, &signature)
        .map_err(|_| "registry publisher signature verification failed".to_string())?;
    Ok(sidecar.publisher)
}

fn registry_trust_store_bytes() -> Result<Vec<u8>, String> {
    let path = std::env::var("PBLANG_REGISTRY_TRUSTED_KEYS_FILE").map_err(|_| {
        "signed registry required: set PBLANG_REGISTRY_TRUSTED_KEYS_FILE to an operator-owned publisher trust store (or explicitly opt into development-only unsigned mode)".to_string()
    })?;
    let path = PathBuf::from(path);
    let metadata = std::fs::symlink_metadata(&path).map_err(|e| {
        format!(
            "cannot inspect registry trust store {}: {e}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("registry trust store must be a regular non-symlink file".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let owner = metadata.uid();
        let current = unsafe { libc::geteuid() };
        if (owner != current && owner != 0) || metadata.permissions().mode() & 0o022 != 0 {
            return Err("registry trust store must be owned by the current user or root and not group/world writable".to_string());
        }
    }
    let bytes = std::fs::read(&path)
        .map_err(|e| format!("cannot read registry trust store {}: {e}", path.display()))?;
    if bytes.len() > MAX_REGISTRY_SIGNATURE_BYTES {
        return Err("registry trust store exceeds 16 KiB cap".to_string());
    }
    Ok(bytes)
}

fn fetch_authenticated_registry_index(
    index_url: &str,
    allow_unsigned_registry_dev: bool,
) -> Result<Vec<u8>, String> {
    let unsigned_local_fixture =
        unsigned_registry_dev_allowed(index_url, allow_unsigned_registry_dev)?;
    let index_bytes = fetch_url_with_limit(index_url, MAX_REGISTRY_INDEX_BYTES, "registry index")?;
    if unsigned_local_fixture {
        return Ok(index_bytes);
    }
    let signature_url = format!("{index_url}.sig.json");
    let signature_bytes = fetch_url_with_limit(
        &signature_url,
        MAX_REGISTRY_SIGNATURE_BYTES,
        "registry signature",
    )
    .map_err(|e| format!("signed registry required; cannot fetch {signature_url}: {e}"))?;
    let trust_store = registry_trust_store_bytes()?;
    verify_registry_index_signature(&index_bytes, &signature_bytes, &trust_store)?;
    Ok(index_bytes)
}

fn loopback_registry_url(raw: &str) -> bool {
    // Backslashes have parser-dependent authority semantics. Reject them
    // before using the same standards-compliant parser for policy decisions.
    if raw.contains('\\') {
        return false;
    }
    let Ok(parsed) = url::Url::parse(raw) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return false;
    }
    match parsed.host() {
        Some(url::Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(host)) => host.is_loopback(),
        Some(url::Host::Ipv6(host)) => host.is_loopback(),
        None => false,
    }
}

fn local_file_registry_path(raw: &str) -> Option<PathBuf> {
    if raw.contains('\\') {
        return None;
    }
    #[cfg(windows)]
    if raw.to_ascii_lowercase().starts_with("file:////") {
        return None;
    }
    let parsed = url::Url::parse(raw).ok()?;
    if parsed.scheme() != "file"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.host().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || parsed.path().starts_with("//")
    {
        return None;
    }
    let path = parsed.to_file_path().ok()?;
    if !path.is_absolute() {
        return None;
    }
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let allowed_local_disk = path.components().next().is_some_and(|component| {
            matches!(
                component,
                Component::Prefix(prefix)
                    if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
            )
        });
        if !allowed_local_disk {
            return None;
        }
    }
    Some(path)
}

fn unsigned_registry_dev_allowed(index_url: &str, requested: bool) -> Result<bool, String> {
    if !requested {
        return Ok(false);
    }
    if !cfg!(debug_assertions) {
        return Err("unsigned registry development mode is disabled in release builds".to_string());
    }
    if local_file_registry_path(index_url).is_some() || loopback_registry_url(index_url) {
        return Ok(true);
    }
    Err(
        "unsigned registry development mode is restricted to file:// and loopback fixtures"
            .to_string(),
    )
}

fn unsigned_registry_dev_enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var("PBLANG_ALLOW_UNSIGNED_REGISTRY_DEV")
            .ok()
            .as_deref()
            == Some("1")
}

/// Parse registry index JSON. Bytes longer than `MAX_REGISTRY_INDEX_BYTES`
/// are rejected before the parse to bound memory consumption.
pub fn parse_registry_index(json: &[u8]) -> Result<RegistryIndex, String> {
    if json.len() > MAX_REGISTRY_INDEX_BYTES {
        return Err(format!(
            "index JSON: payload of {} bytes exceeds cap of {} bytes",
            json.len(),
            MAX_REGISTRY_INDEX_BYTES
        ));
    }
    let root: serde_json::Value =
        serde_json::from_slice(json).map_err(|e| format!("index JSON: {e}"))?;
    let packages = root
        .get("packages")
        .and_then(|p| p.as_object())
        .ok_or_else(|| "index: missing 'packages' object".to_string())?;
    let mut index = RegistryIndex::new();
    for (pkg_name, versions_val) in packages {
        let versions = versions_val
            .as_object()
            .ok_or_else(|| format!("index: package '{}' not object", pkg_name))?;
        let mut releases = std::collections::HashMap::new();
        for (ver, release_val) in versions {
            let release_obj = release_val
                .as_object()
                .ok_or_else(|| format!("index: release {}/{} not object", pkg_name, ver))?;
            let url = release_obj
                .get("url")
                .and_then(|u| u.as_str())
                .ok_or_else(|| format!("index: release {}/{} missing 'url'", pkg_name, ver))?
                .to_string();
            let sha256 = release_obj
                .get("sha256")
                .and_then(|s| s.as_str())
                .map(String::from);
            if let Some(ref digest) = sha256 {
                if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return Err(format!(
                        "index: release {pkg_name}/{ver} has malformed sha256"
                    ));
                }
            }
            let description = release_obj
                .get("description")
                .and_then(|s| s.as_str())
                .map(String::from);
            let dependencies_val = release_obj.get("dependencies");
            let dependencies: std::collections::BTreeMap<String, String> = dependencies_val
                .and_then(|d| d.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect()
                })
                .unwrap_or_default();
            releases.insert(
                ver.clone(),
                RegistryRelease {
                    url,
                    sha256,
                    description,
                    dependencies,
                },
            );
        }
        index.insert(pkg_name.clone(), releases);
    }
    Ok(index)
}

/// Unpack .tar.gz bytes into dest_dir.
///
/// SECURITY FIX #7: Uses entry-by-entry unpacking with path validation to prevent
/// "Zip Slip" path traversal attacks where a malicious tarball contains entries
/// with `../` components that would escape dest_dir.
pub fn unpack_tarball_gz(tar_gz: &[u8], dest_dir: &Path) -> Result<(), String> {
    use std::path::Component;
    if tar_gz.len() > MAX_REGISTRY_ARCHIVE_BYTES {
        return Err(format!(
            "registry archive: compressed payload of {} bytes exceeds cap of {} bytes",
            tar_gz.len(),
            MAX_REGISTRY_ARCHIVE_BYTES
        ));
    }
    let dest_existed = dest_dir.exists();
    let dest_dir = std::fs::canonicalize(dest_dir)
        .or_else(|_| {
            std::fs::create_dir_all(dest_dir).map_err(|e| format!("create_dir_all: {e}"))?;
            std::fs::canonicalize(dest_dir).map_err(|e| format!("canonicalize dest: {e}"))
        })
        .map_err(|e: String| e)?;

    let extraction = (|| {
        let dec = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(dec);
        let mut entry_count = 0usize;
        let mut unpacked_bytes = 0u64;

        for entry in archive.entries().map_err(|e| format!("tar entries: {e}"))? {
            entry_count = entry_count
                .checked_add(1)
                .ok_or_else(|| "registry archive: entry count overflow".to_string())?;
            if entry_count > MAX_REGISTRY_ENTRIES {
                return Err(format!(
                    "registry archive: entry count exceeds cap of {MAX_REGISTRY_ENTRIES}"
                ));
            }

            let mut entry = entry.map_err(|e| format!("tar entry: {e}"))?;
            let entry_path = entry.path().map_err(|e| format!("tar path: {e}"))?;
            let entry_type = entry.header().entry_type();
            if matches!(entry_type, tar::EntryType::Symlink | tar::EntryType::Link) {
                return Err(format!(
                    "SECURITY: tarball link entries are not allowed: {}",
                    entry_path.display()
                ));
            }
            if !matches!(
                entry_type,
                tar::EntryType::Regular | tar::EntryType::Directory
            ) {
                return Err(format!(
                    "SECURITY: tarball entry type is not allowed: {}",
                    entry_path.display()
                ));
            }

            // Validate: reject any path that would escape dest_dir.
            for component in entry_path.components() {
                match component {
                    Component::ParentDir => {
                        return Err(format!(
                            "SECURITY: tarball entry '{}' contains '..', rejected (Zip Slip)",
                            entry_path.display()
                        ));
                    }
                    Component::RootDir | Component::Prefix(_) => {
                        return Err(format!(
                            "SECURITY: tarball entry '{}' is absolute, rejected",
                            entry_path.display()
                        ));
                    }
                    _ => {}
                }
            }

            let entry_bytes = if entry_type == tar::EntryType::Regular {
                entry
                    .header()
                    .size()
                    .map_err(|e| format!("tar size: {e}"))?
            } else {
                0
            };
            if entry_bytes > MAX_REGISTRY_ENTRY_BYTES {
                return Err(format!(
                    "registry archive: entry '{}' is {} bytes, cap is {} bytes",
                    entry_path.display(),
                    entry_bytes,
                    MAX_REGISTRY_ENTRY_BYTES
                ));
            }
            unpacked_bytes = unpacked_bytes
                .checked_add(entry_bytes)
                .ok_or_else(|| "registry archive: unpacked size overflow".to_string())?;
            if unpacked_bytes > MAX_REGISTRY_UNPACKED_BYTES {
                return Err(format!(
                    "registry archive: unpacked content exceeds cap of {} bytes",
                    MAX_REGISTRY_UNPACKED_BYTES
                ));
            }

            entry
                .unpack_in(&dest_dir)
                .map_err(|e| format!("tar unpack entry: {e}"))?;
        }
        Ok(())
    })();

    if extraction.is_err() && !dest_existed {
        let _ = std::fs::remove_dir_all(&dest_dir);
    }
    extraction
}

struct RegistryCacheInstallLock {
    file: std::fs::File,
}

impl Drop for RegistryCacheInstallLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

fn private_registry_cache_root(cache_root: &Path) -> Result<(), String> {
    if cache_root.as_os_str().is_empty() {
        return Err("registry cache root is empty".to_string());
    }
    let existed = cache_root.exists();
    if !existed {
        std::fs::create_dir_all(cache_root)
            .map_err(|e| format!("create private registry cache root: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(cache_root, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("protect registry cache root: {e}"))?;
        }
    }
    let mut metadata = std::fs::symlink_metadata(cache_root)
        .map_err(|e| format!("inspect registry cache root: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("registry cache root must be a regular directory, not a symlink".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let current = unsafe { libc::geteuid() };
        if metadata.uid() != current {
            return Err("registry cache root must be owned by the current user".to_string());
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            std::fs::set_permissions(cache_root, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("tighten registry cache root permissions: {e}"))?;
            metadata = std::fs::symlink_metadata(cache_root)
                .map_err(|e| format!("reinspect registry cache root: {e}"))?;
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("registry cache root permissions could not be tightened".to_string());
        }
    }
    Ok(())
}

fn lock_registry_cache(cache_root: &Path) -> Result<RegistryCacheInstallLock, String> {
    use std::fs::OpenOptions;
    let path = cache_root.join(".registry-install.lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let file = options
        .open(&path)
        .map_err(|e| format!("cannot open registry cache install lock safely: {e}"))?;
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|e| format!("cannot inspect registry cache install lock: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("registry cache install lock must be a regular non-symlink file".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(
                "registry cache install lock must be current-user-owned and mode 0600".to_string(),
            );
        }
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(RegistryCacheInstallLock { file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    return Err(
                        "registry cache remained busy beyond the 10 second lock deadline"
                            .to_string(),
                    );
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(error) => return Err(format!("lock registry cache install state: {error}")),
        }
    }
}

fn registry_cache_staging(cache_root: &Path, archive_sha256: &str) -> Result<PathBuf, String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before the Unix epoch".to_string())?
        .as_nanos();
    for attempt in 0..16u32 {
        let path = cache_root.join(format!(
            ".{archive_sha256}.staging-{}-{epoch}-{attempt}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
                        .map_err(|e| format!("protect registry cache staging: {e}"))?;
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("create registry cache staging: {error}")),
        }
    }
    Err("cannot allocate unique registry cache staging directory".to_string())
}

fn install_verified_registry_archive(
    cache_root: &Path,
    archive_sha256: &str,
    tarball_bytes: &[u8],
) -> Result<(PathBuf, String), String> {
    if archive_sha256.len() != 64 || !archive_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("registry archive cache identity is malformed".to_string());
    }
    private_registry_cache_root(cache_root)?;
    let _lock = lock_registry_cache(cache_root)?;
    let staging = registry_cache_staging(cache_root, archive_sha256)?;
    let staged_result = (|| {
        unpack_tarball_gz(tarball_bytes, &staging)?;
        validate_package_tree(&staging)?;
        hash_dir(&staging)
    })();
    let staged_hash = match staged_result {
        Ok(hash) => hash,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
    };
    let cache_entry = cache_root.join(archive_sha256.to_ascii_lowercase());
    if cache_entry.exists() {
        let existing = validate_package_tree(&cache_entry).and_then(|_| hash_dir(&cache_entry));
        let _ = std::fs::remove_dir_all(&staging);
        let existing_hash = existing?;
        if existing_hash != staged_hash {
            return Err(
                "registry cache collision: existing content does not match verified archive"
                    .to_string(),
            );
        }
        return Ok((cache_entry, existing_hash));
    }
    if let Err(error) = std::fs::rename(&staging, &cache_entry) {
        let existing = validate_package_tree(&cache_entry).and_then(|_| hash_dir(&cache_entry));
        let _ = std::fs::remove_dir_all(&staging);
        let existing_hash = existing
            .map_err(|_| format!("atomically publish verified registry cache entry: {error}"))?;
        if existing_hash != staged_hash {
            return Err(
                "registry cache collision during publication: content mismatch".to_string(),
            );
        }
    }
    Ok((cache_entry, staged_hash))
}

/// Default cache root.
pub fn cache_root(manifest_dir: Option<&Path>) -> PathBuf {
    if let Ok(p) = std::env::var("PBLANG_CACHE") {
        return PathBuf::from(p);
    }
    if let Some(man) = manifest_dir {
        let local = man.join(".pblang").join("cache");
        if local.exists() || man.join(".pblang").exists() {
            return local;
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    home.join(".pblang").join("cache")
}

/// Resolve a registry release URL relative to the index URL when needed.
pub fn resolve_registry_release_url(index_url: &str, release_url: &str) -> String {
    if release_url.starts_with("http://")
        || release_url.starts_with("https://")
        || release_url.starts_with("file://")
    {
        return release_url.to_string();
    }
    if let Some(path) = index_url.strip_prefix("file://") {
        let index_path = Path::new(path);
        if let Some(parent) = index_path.parent() {
            let joined = parent.join(release_url);
            let joined = joined.to_string_lossy().replace('\\', "/");
            return format!("file:///{}", joined.trim_start_matches('/'));
        }
    }
    if let Some(base) = http_index_base_url(index_url) {
        let rel = release_url.trim_start_matches('/');
        return format!("{base}/{rel}");
    }
    release_url.to_string()
}

fn http_index_base_url(index_url: &str) -> Option<String> {
    let without_query = index_url.split('?').next().unwrap_or(index_url);
    without_query
        .rfind('/')
        .map(|pos| without_query[..pos].to_string())
}

fn verify_tarball_hmac_sidecar(tarball_url: &str, tarball_bytes: &[u8]) -> Result<(), String> {
    let key = match std::env::var("PBLANG_SIGNING_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => return Ok(()),
    };
    let path = tarball_url
        .strip_prefix("file://")
        .map(Path::new)
        .filter(|p| p.is_file());
    let Some(path) = path else {
        return Ok(());
    };
    let sidecar = PathBuf::from(format!("{}.sha256", path.display()));
    if !sidecar.is_file() {
        return Ok(());
    }
    let expected =
        std::fs::read_to_string(&sidecar).map_err(|e| format!("read HMAC sidecar: {e}"))?;
    let expected = expected.trim();
    let got = compute_tarball_hmac(tarball_bytes, key.as_bytes());
    if !got.eq_ignore_ascii_case(expected) {
        return Err(format!(
            "HMAC signature mismatch for registry tarball (sidecar {})",
            sidecar.display()
        ));
    }
    Ok(())
}

/// Returns true when the directory contains at least one recognized module file.
pub fn package_has_module_files(package_dir: &Path) -> bool {
    let mut entries = vec![];
    let _ = collect_files(package_dir, package_dir, &mut entries);
    entries
        .iter()
        .any(|entry| entry.kind == b'f' && has_module_extension(&entry.relative_path))
}

/// Validate a package directory before publish/install (modules + optional entry).
pub fn validate_package_tree(package_dir: &Path) -> Result<(), String> {
    // Validate the complete tree before trusting manifest paths or scanning for
    // modules. This rejects symlinks and special files anywhere in the package,
    // including hidden and generated-looking directories.
    let _ = hash_dir(package_dir)?;
    let man_path = package_dir.join("pblang.toml");
    if man_path.is_file() {
        let man_bytes = read_regular_file_nofollow(&man_path)
            .map_err(|e| format!("read manifest {}: {e}", man_path.display()))?;
        let man =
            parse_manifest(std::str::from_utf8(&man_bytes).map_err(|_| "manifest not utf8")?)?;
        if let Some(entry) = man.package.as_ref().and_then(|p| p.entry.as_deref()) {
            let entry_rel = Path::new(entry);
            if entry_rel.as_os_str().is_empty()
                || entry_rel.components().any(|component| {
                    !matches!(
                        component,
                        std::path::Component::Normal(_) | std::path::Component::CurDir
                    )
                })
            {
                return Err(format!(
                    "package entry must stay within the package: {entry}"
                ));
            }
            let entry_path = package_dir.join(entry);
            let entry_meta = std::fs::symlink_metadata(&entry_path)
                .map_err(|_| format!("package entry not found: {}", entry_path.display()))?;
            if entry_meta.file_type().is_symlink() || !entry_meta.is_file() {
                return Err(format!(
                    "package entry is not a file (regular non-symlink required): {}",
                    entry_path.display()
                ));
            }
            let root = std::fs::canonicalize(package_dir)
                .map_err(|e| format!("canonicalize package root: {e}"))?;
            let canonical_entry = std::fs::canonicalize(&entry_path)
                .map_err(|e| format!("canonicalize package entry: {e}"))?;
            if !canonical_entry.starts_with(&root) {
                return Err(format!("package entry escapes package root: {entry}"));
            }
        }
    }
    if !package_has_module_files(package_dir) {
        return Err("package has no pblang module files (.sapp/.sap/.bpx)".to_string());
    }
    Ok(())
}

/// Verify unpacked dependency tree matches the locked content hash.
pub fn verify_locked_dep_content(dep: &LockedDep) -> Result<(), String> {
    let path = Path::new(&dep.path);
    if !path.is_dir() {
        return Err(format!("dependency path not found: {}", dep.path));
    }
    let got = hash_dir(path)?;
    if got != dep.sha256 {
        return Err(format!(
            "content hash mismatch for dependency '{}': expected {} got {}",
            dep.name.as_deref().unwrap_or("?"),
            dep.sha256,
            got
        ));
    }
    Ok(())
}

/// Resolve a git dependency to a cached checkout. Returns (path, content_sha256, resolved_key).
pub fn resolve_git_dep(
    git_url: &str,
    rev: &str,
    cache_root: &Path,
) -> Result<(PathBuf, String, String), String> {
    if !matches!(rev.len(), 40 | 64) || !rev.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("git dependency requires a full 40- or 64-hex commit id".to_string());
    }
    private_registry_cache_root(cache_root)?;
    let cache_key = hex::encode(Sha256::digest(format!("{git_url}#{rev}").as_bytes()));
    let git_cache = cache_root.join("git");
    if !git_cache.exists() {
        std::fs::create_dir(&git_cache).map_err(|e| format!("create git cache dir: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&git_cache, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("protect git cache dir: {e}"))?;
        }
    }
    let git_cache_meta =
        std::fs::symlink_metadata(&git_cache).map_err(|e| format!("inspect git cache dir: {e}"))?;
    if git_cache_meta.file_type().is_symlink() || !git_cache_meta.is_dir() {
        return Err("git cache path must be a regular non-symlink directory".to_string());
    }

    let checkout = git_cache.join(&cache_key);
    let staging = registry_cache_staging(cache_root, &format!("git-{cache_key}"))?;
    let prepared = (|| {
        let status = Command::new("git")
            .args(["clone", "--no-checkout", git_url])
            .arg(&staging)
            .status()
            .map_err(|e| format!("git clone failed to start: {e}"))?;
        if !status.success() {
            return Err(format!("git clone failed for {git_url}"));
        }
        let status = Command::new("git")
            .args(["checkout", "--detach", rev])
            .current_dir(&staging)
            .status()
            .map_err(|e| format!("git checkout failed to start: {e}"))?;
        if !status.success() {
            return Err(format!("git checkout {rev} failed for {git_url}"));
        }
        let resolved = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&staging)
            .output()
            .map_err(|e| format!("git rev-parse failed to start: {e}"))?;
        if !resolved.status.success() {
            return Err("git could not resolve the checked-out commit".to_string());
        }
        let resolved = std::str::from_utf8(&resolved.stdout)
            .map_err(|_| "git resolved commit is not UTF-8")?
            .trim();
        if !resolved.eq_ignore_ascii_case(rev) {
            return Err(format!(
                "git resolved commit mismatch: requested {rev}, resolved {resolved}"
            ));
        }
        let git_metadata = staging.join(".git");
        let metadata = std::fs::symlink_metadata(&git_metadata)
            .map_err(|e| format!("inspect cloned git metadata: {e}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("cloned git metadata is not a regular directory".to_string());
        }
        std::fs::remove_dir_all(&git_metadata)
            .map_err(|e| format!("freeze git dependency source snapshot: {e}"))?;
        validate_package_tree(&staging)?;
        hash_dir(&staging)
    })();
    let staged_hash = match prepared {
        Ok(hash) => hash,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
    };

    let _lock = lock_registry_cache(cache_root)?;
    if checkout.exists() {
        let existing_hash = validate_package_tree(&checkout).and_then(|_| hash_dir(&checkout));
        let _ = std::fs::remove_dir_all(&staging);
        let existing_hash = existing_hash?;
        if existing_hash != staged_hash {
            return Err(
                "git cache collision: existing content does not match the pinned commit"
                    .to_string(),
            );
        }
    } else if let Err(error) = std::fs::rename(&staging, &checkout) {
        let existing_hash = validate_package_tree(&checkout).and_then(|_| hash_dir(&checkout));
        let _ = std::fs::remove_dir_all(&staging);
        let existing_hash = existing_hash
            .map_err(|_| format!("atomically publish verified git cache entry: {error}"))?;
        if existing_hash != staged_hash {
            return Err(
                "git cache collision: concurrently published content does not match the pinned commit"
                    .to_string(),
            );
        }
    }
    let resolved = format!("{git_url}#{rev}");
    Ok((checkout, staged_hash, resolved))
}

/// Rehydrate or verify a locked dependency root (registry, git, or path).
pub fn resolve_dependency_root(
    man_dir: &Path,
    dep_name: &str,
    dep: &LockedDep,
) -> Result<PathBuf, String> {
    let path = Path::new(&dep.path);
    if path.is_dir() {
        return verify_locked_dep_content(dep).map(|_| path.to_path_buf());
    }

    match dep.source.as_deref() {
        Some("registry") => {
            let cache = cache_root(Some(man_dir));
            let root = if let Some(tarball_url) = dep.fetch_url.as_deref() {
                let archive_sha256 = dep.archive_sha256.as_deref().unwrap_or(dep.sha256.as_str());
                let cache_entry = cache.join(archive_sha256);
                if !cache_entry.is_dir() {
                    let tarball_bytes = fetch_url_with_limit(
                        tarball_url,
                        MAX_REGISTRY_ARCHIVE_BYTES,
                        "registry archive",
                    )?;
                    verify_tarball_hmac_sidecar(tarball_url, &tarball_bytes)?;
                    let (installed, _) =
                        install_verified_registry_archive(&cache, archive_sha256, &tarball_bytes)?;
                    if installed != cache_entry {
                        return Err(
                            "registry cache identity mismatch after rehydration".to_string()
                        );
                    }
                }
                cache_entry
            } else {
                let name = dep.name.as_deref().unwrap_or(dep_name);
                let version = dep.version.as_deref().ok_or_else(|| {
                    format!("lockfile missing version for registry dep '{dep_name}'")
                })?;
                let index_url = std::env::var("PBLANG_REGISTRY_INDEX").map_err(|_| {
                    format!(
                        "registry dep '{dep_name}' cache path missing; set PBLANG_REGISTRY_INDEX or refresh lockfile"
                    )
                })?;
                let (root, _, _, _) =
                    resolve_registry_dep(&index_url, &cache, name, version, false)?;
                root
            };
            verify_locked_dep_content(&LockedDep {
                path: root.to_string_lossy().to_string(),
                sha256: dep.sha256.clone(),
                ..dep.clone()
            })?;
            Ok(root)
        }
        Some("git") => {
            let resolved = dep
                .resolved
                .as_deref()
                .ok_or_else(|| format!("lockfile missing resolved git ref for '{dep_name}'"))?;
            let (git_url, rev) = resolved
                .rsplit_once('#')
                .ok_or_else(|| format!("invalid git resolved ref in lockfile: {resolved}"))?;
            let cache = cache_root(Some(man_dir));
            let (root, content_hash, _) = resolve_git_dep(git_url, rev, &cache)?;
            if content_hash != dep.sha256 {
                return Err(format!(
                    "content hash mismatch after rehydrating git dep '{dep_name}'"
                ));
            }
            Ok(root)
        }
        _ => Err(format!("dependency path not found: {}", dep.path)),
    }
}

fn verify_package_tree_for_path_dep(path: &Path) -> Result<(), String> {
    validate_package_tree(path)
}

fn resolve_package_entry_in_dir(package_dir: &Path) -> Result<PathBuf, String> {
    let man_path = package_dir.join("pblang.toml");
    if man_path.is_file() {
        let man_bytes = std::fs::read(&man_path).map_err(|e| format!("read manifest: {e}"))?;
        let man =
            parse_manifest(std::str::from_utf8(&man_bytes).map_err(|_| "manifest not utf8")?)?;
        if let Some(entry) = man.package.as_ref().and_then(|p| p.entry.as_deref()) {
            let entry_path = package_dir.join(entry);
            if entry_path.is_file() {
                return canonicalize_existing(&entry_path)
                    .ok_or_else(|| format!("package entry not found: {}", entry_path.display()));
            }
            if entry_path.is_dir() {
                return Err(format!(
                    "package entry is not a file: {}",
                    entry_path.display()
                ));
            }
            return Err(format!("package entry not found: {}", entry_path.display()));
        }
    }
    for candidate in [
        "main.sapp",
        "main.sap",
        "main.bpx",
        "main.pbx",
        "main.pb",
        "main.bp",
        "lib.sapp",
        "lib.sap",
        "lib.pbx",
        "lib.pb",
        "lib.bp",
    ] {
        let p = package_dir.join(candidate);
        if p.is_file() {
            return canonicalize_existing(&p)
                .ok_or_else(|| format!("import not found: {}", p.display()));
        }
    }
    package_has_module_files(package_dir);
    Err(format!(
        "package '{}' has no default entry; use pkg:path/file.sapp",
        package_dir.display()
    ))
}

fn resolve_package_subpath(dep_root: &Path, sub: &str) -> Result<PathBuf, String> {
    let sub = sub.trim_start_matches('/');
    if sub.is_empty() {
        return resolve_package_entry_in_dir(dep_root);
    }
    let sub_path = Path::new(sub);
    for component in sub_path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(format!(
                "import subpath '{}' must not escape the package root",
                sub
            ));
        }
    }
    let path = dep_root.join(sub_path);
    if path.is_dir() {
        return Err(format!("import subpath is not a file: {}", path.display()));
    }
    canonicalize_existing(&path).ok_or_else(|| format!("import not found: {}", path.display()))
}

/// Resolve one registry dependency.
/// Returns (unpacked path, content tree sha256, archive sha256, fetch URL).
pub fn resolve_registry_dep(
    index_url: &str,
    cache_root: &Path,
    name: &str,
    version: &str,
    require_registry_sha256: bool,
) -> Result<(PathBuf, String, String, String), String> {
    resolve_registry_dep_with_policy(
        index_url,
        cache_root,
        name,
        version,
        require_registry_sha256,
        unsigned_registry_dev_enabled(),
    )
}

/// Development-compatible resolver. `allow_unsigned_registry_dev` must only be
/// set by an explicit operator choice for local fixtures or migration work.
pub fn resolve_registry_dep_with_policy(
    index_url: &str,
    cache_root: &Path,
    name: &str,
    version: &str,
    _require_registry_sha256: bool,
    allow_unsigned_registry_dev: bool,
) -> Result<(PathBuf, String, String, String), String> {
    let index_bytes = fetch_authenticated_registry_index(index_url, allow_unsigned_registry_dev)?;
    let index = parse_registry_index(&index_bytes)?;
    let releases = index
        .get(name)
        .ok_or_else(|| format!("registry: package '{}' not in index", name))?;
    let release = releases
        .get(version)
        .ok_or_else(|| format!("registry: version {} not found for '{}'", version, name))?;
    if release.sha256.is_none() {
        return Err(format!(
            "registry: sha256 required for {} {} but index has no sha256",
            name, version
        ));
    }
    let release_fetch_url = resolve_registry_release_url(index_url, &release.url);
    let tarball_bytes = fetch_url_with_limit(
        &release_fetch_url,
        MAX_REGISTRY_ARCHIVE_BYTES,
        "registry archive",
    )?;
    verify_tarball_hmac_sidecar(&release_fetch_url, &tarball_bytes)?;
    let archive_sha256 = hex::encode(Sha256::digest(&tarball_bytes));
    if let Some(ref expected) = release.sha256 {
        if !expected.eq_ignore_ascii_case(&archive_sha256) {
            return Err(format!(
                "registry: checksum mismatch for {} {}: expected {} got {}",
                name, version, expected, archive_sha256
            ));
        }
    }
    let (cache_entry, content_sha256) =
        install_verified_registry_archive(cache_root, &archive_sha256, &tarball_bytes)?;
    Ok((
        cache_entry,
        content_sha256,
        archive_sha256,
        release_fetch_url,
    ))
}

/// Fetch and install a single registry package into the cache.
/// Returns (resolved_version, path_to_unpacked_dir).
pub fn fetch_and_install_package(
    registry_url: &str,
    name: &str,
    version: &str,
    cache_dir: &Path,
    require_sha: bool,
) -> Result<(String, PathBuf), String> {
    let (path, _content_sha, _archive_sha, _url) =
        resolve_registry_dep(registry_url, cache_dir, name, version, require_sha)?;
    Ok((version.to_string(), path))
}

// ──────────────────────────────────────────────────────────────
// Manifest / lockfile helpers
// ──────────────────────────────────────────────────────────────

pub fn parse_manifest(toml_str: &str) -> Result<Manifest, String> {
    toml::from_str::<Manifest>(toml_str).map_err(|e| format!("manifest parse error: {e}"))
}

pub fn parse_lockfile(toml_str: &str) -> Result<Lockfile, String> {
    let lock =
        toml::from_str::<Lockfile>(toml_str).map_err(|e| format!("lockfile parse error: {e}"))?;
    if lock.content_hash_algorithm != LOCKFILE_CONTENT_HASH_ALGORITHM {
        let found = if lock.content_hash_algorithm.is_empty() {
            "missing/legacy"
        } else {
            lock.content_hash_algorithm.as_str()
        };
        return Err(format!(
            "lockfile content hash algorithm is {found}; expected {LOCKFILE_CONTENT_HASH_ALGORITHM}. Review dependency provenance and run `pblang install` to refresh it"
        ));
    }
    Ok(lock)
}

pub fn write_lockfile(lock: &Lockfile) -> Result<String, String> {
    toml::to_string(lock).map_err(|e| format!("lockfile serialize error: {e}"))
}

pub fn manifest_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

pub fn file_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

pub fn find_manifest_dir(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(p) = cur {
        let cand = p.join("pblang.toml");
        if cand.is_file() {
            return Some(p.to_path_buf());
        }
        cur = p.parent();
    }
    None
}

/// Walk up from `start` looking for a `pblang.toml` whose `[workspace]`
/// section is present. Returns the directory containing that manifest, or
/// `None` if no workspace root exists above `start`.
///
/// Used by workspace-aware commands to find the right root even when
/// invoked from inside a member directory.
pub fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut cur = Some(start);
    while let Some(p) = cur {
        let cand = p.join("pblang.toml");
        if cand.is_file() {
            if let Ok(bytes) = std::fs::read(&cand) {
                if let Ok(s) = std::str::from_utf8(&bytes) {
                    if let Ok(m) = parse_manifest(s) {
                        if m.workspace.is_some() {
                            return Some(p.to_path_buf());
                        }
                    }
                }
            }
        }
        cur = p.parent();
    }
    None
}

/// One match returned by `resolve_workspace_members`: the absolute path of a
/// member package directory plus the pattern that selected it (handy for
/// error messages that want to point at the `members` entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMember {
    pub dir: PathBuf,
    pub matched_pattern: String,
}

/// Expand `[workspace] members` / `exclude` patterns against `root` and
/// return the resulting list of member directories.
///
/// Each pattern is one of:
///   - Literal path (`"tools/foo"`): must point at a directory containing
///     `pblang.toml`; missing or non-package paths are an error.
///   - Trailing-`*` wildcard (`"crates/*"`): expands to every immediate
///     subdir of the stem that contains its own `pblang.toml`. The stem
///     itself must exist; non-package subdirs are silently skipped.
///
/// The output is sorted and de-duplicated by absolute path. Excludes are
/// applied last, so a wildcard pull-in can be trimmed by an exclude.
pub fn resolve_workspace_members(
    root: &Path,
    ws: &WorkspaceSection,
) -> Result<Vec<WorkspaceMember>, String> {
    let mut out: Vec<WorkspaceMember> = Vec::new();
    for pat in &ws.members {
        for dir in expand_member_pattern(root, pat, /* literal_must_have_manifest */ true)? {
            out.push(WorkspaceMember {
                dir,
                matched_pattern: pat.clone(),
            });
        }
    }
    // Apply excludes: drop any member whose absolute dir matches an exclude
    // pattern (literal or trailing-`*`).
    if !ws.exclude.is_empty() {
        let mut excluded: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        for pat in &ws.exclude {
            for dir in
                expand_member_pattern(root, pat, /* literal_must_have_manifest */ false)?
            {
                excluded.insert(dir);
            }
        }
        out.retain(|m| !excluded.contains(&m.dir));
    }
    // De-dup by dir (keeps first-seen pattern as the attribution). Sort for
    // determinism.
    out.sort_by(|a, b| a.dir.cmp(&b.dir));
    out.dedup_by(|a, b| a.dir == b.dir);
    Ok(out)
}

/// Expand a single member or exclude pattern relative to `root`. When
/// `literal_must_have_manifest` is true, a literal path that doesn't have
/// `pblang.toml` is an error (the usual `members` contract); excludes can
/// reference paths that aren't packages, so they pass `false`.
fn expand_member_pattern(
    root: &Path,
    pat: &str,
    literal_must_have_manifest: bool,
) -> Result<Vec<PathBuf>, String> {
    // Reject path traversal — workspace members must stay under the root.
    // We check the stripped pattern, then re-check after canonicalize below
    // for symlink shenanigans.
    if pat.split('/').any(|c| c == ".." || c.is_empty()) {
        return Err(format!(
            "workspace pattern '{}' contains empty or '..' segment",
            pat
        ));
    }
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(stem) = pat
        .strip_suffix("/*")
        .or_else(|| if pat == "*" { Some("") } else { None })
    {
        // Wildcard: list immediate children of root/stem and keep those
        // with pblang.toml.
        let base = if stem.is_empty() {
            root.to_path_buf()
        } else {
            root.join(stem)
        };
        if !base.is_dir() {
            return Err(format!(
                "workspace pattern '{}' refers to non-existent directory '{}'",
                pat,
                base.display()
            ));
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&base)
            .map_err(|e| format!("workspace pattern '{}' read_dir failed: {}", pat, e))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.join("pblang.toml").is_file())
            .collect();
        entries.sort();
        out.extend(entries);
    } else {
        // Literal.
        let p = root.join(pat);
        if literal_must_have_manifest && !p.join("pblang.toml").is_file() {
            return Err(format!(
                "workspace member '{}' has no pblang.toml at '{}'",
                pat,
                p.display()
            ));
        }
        if p.is_dir() {
            out.push(p);
        } else if literal_must_have_manifest {
            return Err(format!("workspace member '{}' is not a directory", pat));
        }
    }
    Ok(out)
}

/// Generate lockfile (uses PBLANG_REGISTRY_INDEX env and REQUIRE_REGISTRY_SHA256).
pub fn generate_lockfile(manifest_dir: &Path) -> Result<Lockfile, String> {
    let require_sha = std::env::var("REQUIRE_REGISTRY_SHA256")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .ok()
        .unwrap_or(false);
    generate_lockfile_with_registry(
        manifest_dir,
        std::env::var("PBLANG_REGISTRY_INDEX").ok().as_deref(),
        require_sha,
    )
}

pub fn generate_lockfile_with_registry(
    manifest_dir: &Path,
    registry_index_url: Option<&str>,
    require_registry_sha256: bool,
) -> Result<Lockfile, String> {
    generate_lockfile_with_registry_policy(
        manifest_dir,
        registry_index_url,
        require_registry_sha256,
        unsigned_registry_dev_enabled(),
    )
}

/// Generate a lockfile with an explicit development-only unsigned-registry
/// compatibility choice. Release callers must use `generate_lockfile_with_registry`.
pub fn generate_lockfile_with_registry_policy(
    manifest_dir: &Path,
    registry_index_url: Option<&str>,
    require_registry_sha256: bool,
    allow_unsigned_registry_dev: bool,
) -> Result<Lockfile, String> {
    let man_path = manifest_dir.join("pblang.toml");
    let man_bytes = std::fs::read(&man_path)
        .map_err(|e| format!("failed to read manifest '{}': {e}", man_path.display()))?;
    let man_sha = manifest_sha256(&man_bytes);
    let man = parse_manifest(
        std::str::from_utf8(&man_bytes).map_err(|_| "manifest not utf8".to_string())?,
    )?;

    let mut deps = std::collections::BTreeMap::<String, LockedDep>::new();
    let cache_root_path = cache_root(Some(manifest_dir));

    for (name, spec) in man.dependencies.iter() {
        if name.starts_with('@') || spec.git().is_some() {
            if let Some(git_url) = spec.git() {
                let rev = spec.rev().ok_or_else(|| {
                    format!("git dependency '{name}' requires a pinned rev in pblang.toml")
                })?;
                let (dep_root, content_hash, resolved) =
                    resolve_git_dep(git_url, rev, &cache_root_path)?;
                deps.insert(
                    name.clone(),
                    LockedDep {
                        path: dep_root.to_string_lossy().to_string(),
                        sha256: content_hash,
                        name: Some(name.clone()),
                        version: None,
                        source: Some("git".to_string()),
                        resolved: Some(resolved),
                        fetch_url: None,
                        archive_sha256: None,
                    },
                );
            } else {
                let version = spec.version_str().unwrap_or("");
                if version.is_empty() {
                    return Err(format!("dependency '{}' requires a version string", name));
                }
                let index_url = registry_index_url.ok_or_else(|| {
                    format!(
                        "registry dep '{}' requires PBLANG_REGISTRY_INDEX to be set",
                        name
                    )
                })?;
                let (dep_root, content_hash, archive_hash, fetch_url) =
                    resolve_registry_dep_with_policy(
                        index_url,
                        &cache_root_path,
                        name,
                        version,
                        require_registry_sha256,
                        allow_unsigned_registry_dev,
                    )?;
                deps.insert(
                    name.clone(),
                    LockedDep {
                        path: dep_root.to_string_lossy().to_string(),
                        sha256: content_hash,
                        name: Some(name.clone()),
                        version: Some(version.to_string()),
                        source: Some("registry".to_string()),
                        resolved: Some(fetch_url.clone()),
                        fetch_url: Some(fetch_url),
                        archive_sha256: Some(archive_hash),
                    },
                );
            }
        } else {
            let rel = spec.path().ok_or_else(|| {
                format!(
                    "dependency '{}' has no path (and is not a registry/git dep)",
                    name
                )
            })?;
            let dep_root = manifest_dir.join(rel);
            let dep_root = canonicalize_existing(&dep_root).ok_or_else(|| {
                format!(
                    "dep '{}' path not found: {} ({})",
                    name,
                    rel,
                    dep_root.display()
                )
            })?;
            let dep_hash = hash_dir(&dep_root)?;
            deps.insert(
                name.clone(),
                LockedDep {
                    path: dep_root.to_string_lossy().to_string(),
                    sha256: dep_hash,
                    name: None,
                    version: None,
                    source: None,
                    resolved: None,
                    fetch_url: None,
                    archive_sha256: None,
                },
            );
        }
    }

    Ok(Lockfile {
        manifest_sha256: man_sha,
        content_hash_algorithm: LOCKFILE_CONTENT_HASH_ALGORITHM.to_string(),
        dependencies: deps,
    })
}

struct PackageTreeEntry {
    relative_path: String,
    kind: u8,
    content: Vec<u8>,
}

fn hash_dir(root: &Path) -> Result<String, String> {
    let root_meta = std::fs::symlink_metadata(root)
        .map_err(|e| format!("metadata '{}': {e}", root.display()))?;
    if root_meta.file_type().is_symlink() || !root_meta.is_dir() {
        return Err(format!(
            "package root is not a regular non-symlink directory: {}",
            root.display()
        ));
    }
    let mut entries = vec![];
    collect_files(root, root, &mut entries)?;
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let mut h = Sha256::new();
    h.update(b"pblang-package-tree-v2\0");
    for entry in entries {
        let path = entry.relative_path.as_bytes();
        h.update([entry.kind]);
        h.update((path.len() as u64).to_be_bytes());
        h.update(path);
        h.update((entry.content.len() as u64).to_be_bytes());
        h.update(&entry.content);
    }
    Ok(hex::encode(h.finalize()))
}

fn read_regular_file_nofollow(path: &Path) -> Result<Vec<u8>, String> {
    let link_meta = std::fs::symlink_metadata(path)
        .map_err(|e| format!("metadata '{}': {e}", path.display()))?;
    if link_meta.file_type().is_symlink() || !link_meta.is_file() {
        return Err(format!(
            "package path is not a regular non-symlink file: {}",
            path.display()
        ));
    }

    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("open '{}': {e}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|e| format!("metadata '{}': {e}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "package path is not a regular file: {}",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("read '{}': {e}", path.display()))?;
    Ok(bytes)
}

fn collect_files(root: &Path, cur: &Path, out: &mut Vec<PackageTreeEntry>) -> Result<(), String> {
    let rd = std::fs::read_dir(cur).map_err(|e| format!("read_dir '{}': {e}", cur.display()))?;
    for ent in rd {
        let ent = ent.map_err(|e| format!("read_dir entry: {e}"))?;
        let p = ent.path();
        let meta = std::fs::symlink_metadata(&p)
            .map_err(|e| format!("metadata '{}': {e}", p.display()))?;
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_str()
            .ok_or_else(|| format!("package path is not valid UTF-8: {}", p.display()))?
            .replace('\\', "/");
        if meta.file_type().is_symlink() {
            return Err(format!("package tree contains symlink: {}", p.display()));
        } else if meta.is_dir() {
            // Include directory identities so empty directories and file/dir
            // type changes are part of the deterministic content digest.
            out.push(PackageTreeEntry {
                relative_path: rel,
                kind: b'd',
                content: Vec::new(),
            });
            collect_files(root, &p, out)?;
        } else if meta.is_file() {
            let bytes = read_regular_file_nofollow(&p)?;
            out.push(PackageTreeEntry {
                relative_path: rel,
                kind: b'f',
                content: bytes,
            });
        } else {
            return Err(format!(
                "package tree contains unsupported special file: {}",
                p.display()
            ));
        }
    }
    Ok(())
}

pub fn resolve_import(
    import_str: &str,
    base_dir: Option<&Path>,
    cwd: &Path,
) -> Result<PathBuf, String> {
    // 1) package form: "pkg:subpath"
    if let Some((pkg, sub)) = import_str.split_once(':') {
        let is_windows_drive = cfg!(windows)
            && pkg.len() == 1
            && pkg
                .chars()
                .next()
                .map(|c| c.is_ascii_alphabetic())
                .unwrap_or(false)
            && (sub.starts_with('/') || sub.starts_with('\\'));
        if !is_windows_drive {
            let base = base_dir.unwrap_or(cwd);
            let man_dir =
                find_manifest_dir(base).ok_or_else(|| "could not find pblang.toml".to_string())?;
            let man_path = man_dir.join("pblang.toml");
            let man_bytes = std::fs::read(&man_path)
                .map_err(|e| format!("failed to read manifest '{}': {e}", man_path.display()))?;
            let man_str = std::str::from_utf8(&man_bytes)
                .map_err(|_| "package manifest is not UTF-8".to_string())?;
            let manifest = parse_manifest(man_str)?;
            if !manifest.dependencies.contains_key(pkg) {
                return Err(format!("unknown dependency '{pkg}'"));
            }
            let man_sha = manifest_sha256(&man_bytes);
            let lock_path = man_dir.join("pblang.lock");
            if !lock_path.is_file() {
                return Err(format!(
                    "declared dependency '{pkg}' has no pblang.lock entry; run `pblang install`"
                ));
            }
            let lock_bytes = std::fs::read(&lock_path)
                .map_err(|e| format!("failed to read pblang.lock: {e}"))?;
            let lock_str = std::str::from_utf8(&lock_bytes)
                .map_err(|_| "pblang.lock is not UTF-8".to_string())?;
            let lock = parse_lockfile(lock_str)
                .map_err(|e| format!("failed to parse pblang.lock: {e}"))?;
            if lock.manifest_sha256 != man_sha {
                return Err("pblang.lock is out of date (manifest has changed). Run `pblang install` to regenerate the lockfile.".to_string());
            }
            let dep = lock.dependencies.get(pkg).ok_or_else(|| {
                format!(
                    "declared dependency '{pkg}' is missing from pblang.lock; run `pblang install`"
                )
            })?;
            let dep_root = resolve_dependency_root(&man_dir, pkg, dep)?;
            return resolve_package_subpath(&dep_root, sub);
        }
    }

    // 1b) bare declared package name: import("@scope/pkg") or import("gitdep").
    // A manifest declaration is a trust boundary: once a name is declared as a
    // dependency it may never fall through to a same-named project/CWD file or
    // PBLANG_PATH entry because the lock is missing, stale or malformed.
    {
        let base = base_dir.unwrap_or(cwd);
        if let Some(man_dir) = find_manifest_dir(base) {
            let man_path = man_dir.join("pblang.toml");
            let lock_path = man_dir.join("pblang.lock");
            if man_path.is_file() {
                let explicit_relative = import_str.starts_with("./")
                    || import_str.starts_with("../")
                    || import_str.starts_with(".\\")
                    || import_str.starts_with("..\\");
                let man_bytes = std::fs::read(&man_path)
                    .map_err(|e| format!("failed to read package manifest: {e}"))?;
                let man_str = std::str::from_utf8(&man_bytes)
                    .map_err(|_| "package manifest is not UTF-8".to_string())?;
                let manifest = match parse_manifest(man_str) {
                    Ok(manifest) => Some(manifest),
                    Err(error) if explicit_relative => {
                        // Explicit local source remains debuggable even while a
                        // developer repairs an unrelated malformed manifest.
                        let _ = error;
                        None
                    }
                    Err(error) => {
                        return Err(format!(
                            "cannot resolve non-relative import with invalid pblang.toml: {error}"
                        ));
                    }
                };
                if manifest
                    .as_ref()
                    .map(|manifest| manifest.dependencies.contains_key(import_str))
                    .unwrap_or(false)
                {
                    if !lock_path.is_file() {
                        return Err(format!(
                            "declared dependency '{import_str}' has no pblang.lock entry; run `pblang install`"
                        ));
                    }
                    let lock_bytes = std::fs::read(&lock_path)
                        .map_err(|e| format!("failed to read pblang.lock: {e}"))?;
                    let lock_str = std::str::from_utf8(&lock_bytes)
                        .map_err(|_| "pblang.lock is not UTF-8".to_string())?;
                    let lock = parse_lockfile(lock_str)
                        .map_err(|e| format!("failed to parse pblang.lock: {e}"))?;
                    if lock.manifest_sha256 != manifest_sha256(&man_bytes) {
                        return Err("pblang.lock is out of date (manifest has changed). Run `pblang install` to regenerate the lockfile.".to_string());
                    }
                    let dep = lock.dependencies.get(import_str).ok_or_else(|| {
                        format!(
                            "declared dependency '{import_str}' is missing from pblang.lock; run `pblang install`"
                        )
                    })?;
                    let root = resolve_dependency_root(&man_dir, import_str, dep)?;
                    return resolve_package_entry_in_dir(&root);
                }
            }
        }
    }

    // 2) absolute or relative path
    let p = Path::new(import_str);
    if p.is_absolute() {
        return canonicalize_existing(p)
            .ok_or_else(|| format!("import not found: {}", p.display()));
    }

    if let Some(base) = base_dir {
        let cand = base.join(p);
        if let Some(c) = canonicalize_existing(&cand) {
            return Ok(c);
        }
        if let Some(man_dir) = find_manifest_dir(base) {
            let cand = man_dir.join(p);
            if let Some(c) = canonicalize_existing(&cand) {
                return Ok(c);
            }
        }
    }

    let cand = cwd.join(p);
    if let Some(c) = canonicalize_existing(&cand) {
        return Ok(c);
    }

    if let Ok(sp) = std::env::var("PBLANG_PATH") {
        let sep = if cfg!(windows) { ';' } else { ':' };
        for entry in sp.split(sep).filter(|s| !s.trim().is_empty()) {
            let cand = Path::new(entry).join(p);
            if let Some(c) = canonicalize_existing(&cand) {
                return Ok(c);
            }
        }
    }

    Err(format!("import not found: {import_str}"))
}

fn tarball_mode_for_rel(rel: &str) -> u32 {
    if rel == "bin/pblang" || rel == "bin/pblang.exe" {
        0o755
    } else {
        0o644
    }
}

fn build_tarball_bytes(mut entries: Vec<(String, Vec<u8>)>) -> Result<Vec<u8>, String> {
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut out = vec![];
    let mut enc = flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
    let mut tar_builder = tar::Builder::new(&mut enc);
    for (rel, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header
            .set_path(&rel)
            .map_err(|e| format!("tar path {}: {e}", rel))?;
        header.set_size(bytes.len() as u64);
        header.set_mode(tarball_mode_for_rel(&rel));
        header.set_cksum();
        tar_builder
            .append(&header, bytes.as_slice())
            .map_err(|e| format!("tar append: {e}"))?;
    }
    tar_builder
        .finish()
        .map_err(|e| format!("tar finish: {e}"))?;
    drop(tar_builder);
    enc.finish().map_err(|e| format!("gzip finish: {e}"))?;
    Ok(out)
}

fn package_tree_tar_entries(entries: Vec<PackageTreeEntry>) -> Vec<(String, Vec<u8>)> {
    entries
        .into_iter()
        .filter(|entry| entry.kind == b'f')
        .map(|entry| (entry.relative_path, entry.content))
        .collect()
}

/// Build .tar.gz bytes of the complete regular-file package tree.
pub fn create_package_tarball(package_dir: &Path) -> Result<Vec<u8>, String> {
    let mut entries = vec![];
    collect_files(package_dir, package_dir, &mut entries)?;
    build_tarball_bytes(package_tree_tar_entries(entries))
}

/// Like [`create_package_tarball`], but embeds the pblang runtime at `bin/pblang` (or `bin/pblang.exe`).
/// Used by self-extracting bundles so end users do not need `pblang` on `PATH`.
pub fn create_package_tarball_with_runtime(
    package_dir: &Path,
    runtime_exe: &Path,
) -> Result<Vec<u8>, String> {
    let mut entries = vec![];
    collect_files(package_dir, package_dir, &mut entries)?;
    let rel = if cfg!(windows) {
        "bin/pblang.exe".to_string()
    } else {
        "bin/pblang".to_string()
    };
    let bytes = std::fs::read(runtime_exe)
        .map_err(|e| format!("read runtime executable '{}': {e}", runtime_exe.display()))?;
    let mut tar_entries = package_tree_tar_entries(entries);
    tar_entries.push((rel, bytes));
    build_tarball_bytes(tar_entries)
}

fn validate_publish_package(package_dir: &Path) -> Result<(), String> {
    let man_path = package_dir.join("pblang.toml");
    let man_bytes = std::fs::read(&man_path)
        .map_err(|e| format!("read manifest {}: {e}", man_path.display()))?;
    let man = parse_manifest(std::str::from_utf8(&man_bytes).map_err(|_| "manifest not utf8")?)?;
    if !man.dependencies.is_empty() {
        return Err(
            "publish: registry packages must be self-contained packages (no [dependencies])"
                .to_string(),
        );
    }
    validate_package_tree(package_dir)
}

/// Publish to a local file-based registry directory.
pub fn publish_to_registry_dir(package_dir: &Path, registry_dir: &Path) -> Result<(), String> {
    validate_publish_package(package_dir)?;
    let man_path = package_dir.join("pblang.toml");
    let man_bytes = std::fs::read(&man_path)
        .map_err(|e| format!("read manifest {}: {e}", man_path.display()))?;
    let man = parse_manifest(std::str::from_utf8(&man_bytes).map_err(|_| "manifest not utf8")?)?;
    let name = man
        .package
        .as_ref()
        .and_then(|p| p.name.as_deref())
        .ok_or_else(|| "publish: [package] name required in pblang.toml".to_string())?;
    let version = man
        .package
        .as_ref()
        .and_then(|p| p.version.as_deref())
        .ok_or_else(|| "publish: [package] version required in pblang.toml".to_string())?;

    let tarball_bytes = create_package_tarball(package_dir)?;
    let sha256 = hex::encode(Sha256::digest(&tarball_bytes));
    let tarball_name = format!("{}-{}.tar.gz", name.replace('/', "_"), version);
    std::fs::create_dir_all(registry_dir).map_err(|e| format!("create registry dir: {e}"))?;
    let tarball_path = registry_dir.join(&tarball_name);
    std::fs::write(&tarball_path, &tarball_bytes).map_err(|e| format!("write tarball: {e}"))?;

    let tarball_abs = canonicalize_existing(&tarball_path).unwrap_or(tarball_path);
    let url_path = tarball_abs.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    let url_path = url_path
        .strip_prefix(r"\\?\")
        .unwrap_or(&url_path)
        .to_string();
    let url = format!("file:///{}", url_path);

    let index_path = registry_dir.join("index.json");
    let mut root: serde_json::Value = if index_path.is_file() {
        let bytes = std::fs::read(&index_path).map_err(|e| format!("read index: {e}"))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("index JSON: {e}"))?
    } else {
        serde_json::json!({ "packages": {} })
    };
    let packages = root
        .get_mut("packages")
        .and_then(|p| p.as_object_mut())
        .ok_or_else(|| "index: missing or invalid 'packages'".to_string())?;
    let pkg_entry = packages
        .entry(name.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let versions = pkg_entry
        .as_object_mut()
        .ok_or_else(|| "index: package entry not object".to_string())?;
    if let Some(existing) = versions.get(version) {
        let existing_sha = existing
            .get("sha256")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !existing_sha.is_empty() && !existing_sha.eq_ignore_ascii_case(&sha256) {
            return Err(format!(
                "publish: version {version} already exists in registry with different content"
            ));
        }
    }
    versions.insert(
        version.to_string(),
        serde_json::json!({ "url": url, "sha256": sha256 }),
    );
    let json = serde_json::to_string_pretty(&root).map_err(|e| format!("serialize index: {e}"))?;
    std::fs::write(&index_path, json).map_err(|e| format!("write index: {e}"))?;
    Ok(())
}

/// Publish a package to a hosted registry via HTTP.
/// Sends the tarball as a JSON POST to `{registry_url}/publish`.
///
/// ## Token refresh (M4-gamma)
/// If the server returns **401 Unauthorized** the function checks for the
/// `PBLANG_REGISTRY_REFRESH_TOKEN` environment variable.  When present it
/// calls [`refresh_access_token`] and retries the publish exactly once with
/// the new token.  A successful retry prints `Refreshed token and retried`.
pub fn publish_package(registry_url: &str, package_dir: &Path, token: &str) -> Result<(), String> {
    validate_publish_package(package_dir)?;
    let tarball = create_package_tarball(package_dir)?;
    let b64 = base64_encode(&tarball);
    let man_path = package_dir.join("pblang.toml");
    let man_bytes = std::fs::read(&man_path).map_err(|e| format!("read manifest: {e}"))?;
    let man = parse_manifest(std::str::from_utf8(&man_bytes).map_err(|_| "utf8")?)?;
    let name = man
        .package
        .as_ref()
        .and_then(|p| p.name.as_deref())
        .unwrap_or("");
    let version = man
        .package
        .as_ref()
        .and_then(|p| p.version.as_deref())
        .unwrap_or("");
    let body = format!(r#"{{"name":"{name}","version":"{version}","tarball":"{b64}"}}"#);
    let url = format!("{}/publish", registry_url.trim_end_matches('/'));

    let mut req = ureq::post(&url);
    if !token.is_empty() {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    let result = req
        .set("Content-Type", "application/json")
        .send_string(&body);

    match result {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(401, _)) => {
            // On 401: attempt a token refresh if PBLANG_REGISTRY_REFRESH_TOKEN is set.
            let refresh_tok = std::env::var("PBLANG_REGISTRY_REFRESH_TOKEN").unwrap_or_default();
            if refresh_tok.is_empty() {
                return Err(
                    "publish failed: 401 Unauthorized                      (set PBLANG_REGISTRY_REFRESH_TOKEN to enable auto-refresh)"
                        .to_string(),
                );
            }
            // Obtain a new access token.
            let new_token = refresh_access_token(registry_url, &refresh_tok)?;
            // Retry publish once with the refreshed token.
            let mut retry_req = ureq::post(&url);
            if !new_token.is_empty() {
                retry_req = retry_req.set("Authorization", &format!("Bearer {new_token}"));
            }
            retry_req
                .set("Content-Type", "application/json")
                .send_string(&body)
                .map_err(|e| format!("publish HTTP error (after token refresh): {e}"))?;
            println!("Refreshed token and retried");
            Ok(())
        }
        Err(e) => Err(format!("publish HTTP error: {e}")),
    }
}

/// POST to `{registry_url}/auth/refresh` with the given refresh token.
///
/// Expects the server to respond with `{"token": "<new_access_token>"}`.
/// Returns the new access token string on success.
fn refresh_access_token(registry_url: &str, refresh_token: &str) -> Result<String, String> {
    let refresh_url = format!("{}/auth/refresh", registry_url.trim_end_matches('/'));
    let refresh_body = serde_json::json!({ "refresh_token": refresh_token }).to_string();
    let resp = ureq::post(&refresh_url)
        .set("Content-Type", "application/json")
        .send_string(&refresh_body)
        .map_err(|e| format!("token refresh HTTP error: {e}"))?;
    let resp_str = resp
        .into_string()
        .map_err(|e| format!("token refresh: read response body: {e}"))?;
    let val: serde_json::Value =
        serde_json::from_str(&resp_str).map_err(|e| format!("token refresh: JSON parse: {e}"))?;
    val.get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "token refresh: response missing 'token' field".to_string())
}

/// Search a registry for packages matching `query`.
pub fn search_registry(registry_url: &str, query: &str) -> Result<Vec<SearchResult>, String> {
    if registry_url.starts_with("file://") {
        let path = registry_url
            .strip_prefix("file://")
            .ok_or_else(|| "internal: file registry URL missing file:// prefix".to_string())?;
        let index_path = Path::new(path).join("index.json");
        let bytes = std::fs::read(&index_path)
            .map_err(|e| format!("read index {}: {e}", index_path.display()))?;
        let index = parse_registry_index(&bytes)?;
        let needle = query.to_lowercase();
        let mut results = Vec::new();
        for (name, versions) in index {
            if !needle.is_empty() && !name.to_lowercase().contains(&needle) {
                continue;
            }
            for version in versions.keys() {
                results.push(SearchResult {
                    name: name.clone(),
                    version: version.clone(),
                    description: String::new(),
                });
            }
        }
        results.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
        return Ok(results);
    }

    let url = format!(
        "{}/search?q={}",
        registry_url.trim_end_matches('/'),
        percent_encode(query)
    );
    // Try fetching as a JSON array of {name, version, description}.
    let bytes = fetch_url_with_limit(&url, MAX_REGISTRY_INDEX_BYTES, "search response")?;
    let val: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("search response JSON: {e}"))?;
    let arr = val.as_array().cloned().unwrap_or_else(|| {
        // Some registries wrap in { "results": [...] }
        val.get("results")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default()
    });
    let results = arr
        .iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.to_string();
            let version = item
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let description = item
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            Some(SearchResult {
                name,
                version,
                description,
            })
        })
        .collect();
    Ok(results)
}

/// Validate that [package] has name, version, and that target_platform / min_runtime are well-formed.
pub fn validate_manifest_metadata(manifest_dir: &Path) -> Result<(), String> {
    let man_path = manifest_dir.join("pblang.toml");
    let man_bytes = std::fs::read(&man_path).map_err(|e| format!("read manifest: {e}"))?;
    let man = parse_manifest(std::str::from_utf8(&man_bytes).map_err(|_| "utf8")?)?;
    let pkg = man
        .package
        .as_ref()
        .ok_or("manifest missing [package] section")?;
    if pkg.name.as_deref().unwrap_or("").is_empty() {
        return Err("[package] name is required".to_string());
    }
    if pkg.version.as_deref().unwrap_or("").is_empty() {
        return Err("[package] version is required".to_string());
    }
    // Validate target_platform if set
    const VALID_PLATFORMS: &[&str] = &["linux", "windows", "macos", "any"];
    if let Some(platform) = pkg.target_platform.as_deref() {
        if !VALID_PLATFORMS.contains(&platform) {
            return Err(format!(
                "target_platform '{}' is unknown; valid values: {}",
                platform,
                VALID_PLATFORMS.join(", ")
            ));
        }
    }
    // Validate min_runtime if set (must be MAJOR.MINOR.PATCH)
    if let Some(min_rt) = pkg.min_runtime.as_deref() {
        let parts: Vec<&str> = min_rt.split('.').collect();
        let valid = parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok());
        if !valid {
            return Err(format!(
                "min_runtime '{}' must be a valid MAJOR.MINOR.PATCH version (e.g. 0.3.0)",
                min_rt
            ));
        }
    }
    Ok(())
}

/// Compute an HMAC-SHA256 of the tarball bytes using the given key.
/// Returns the lowercase hex string.
pub fn compute_tarball_hmac(bytes: &[u8], key: &[u8]) -> String {
    // Simple HMAC-SHA256 implementation using SHA-256 directly (ipad/opad construction).
    const BLOCK_SIZE: usize = 64;
    let mut k = if key.len() > BLOCK_SIZE {
        let digest = Sha256::digest(key);
        digest.to_vec()
    } else {
        key.to_vec()
    };
    k.resize(BLOCK_SIZE, 0);

    let ipad: Vec<u8> = k.iter().map(|b| b ^ 0x36).collect();
    let opad: Vec<u8> = k.iter().map(|b| b ^ 0x5c).collect();

    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(bytes);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(inner_hash);
    hex::encode(outer.finalize())
}

/// Verify a pack tarball: checks it contains pblang.toml and at least one .sapp/.sap file.
/// If a .sha256 sidecar exists alongside the tarball and PBLANG_SIGNING_KEY is set, also verifies HMAC.
pub fn verify_tarball(tarball_path: &Path) -> Result<(), String> {
    let file = std::fs::File::open(tarball_path).map_err(|e| format!("read tarball: {e}"))?;
    let size = file
        .metadata()
        .map_err(|e| format!("stat tarball: {e}"))?
        .len();
    if size > MAX_REGISTRY_ARCHIVE_BYTES as u64 {
        return Err(format!(
            "tarball: compressed payload of {size} bytes exceeds cap of {} bytes",
            MAX_REGISTRY_ARCHIVE_BYTES
        ));
    }
    let mut bytes = Vec::with_capacity(size as usize);
    file.take(MAX_REGISTRY_ARCHIVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read tarball: {e}"))?;
    if bytes.len() > MAX_REGISTRY_ARCHIVE_BYTES {
        return Err(format!(
            "tarball: compressed payload exceeds cap of {} bytes",
            MAX_REGISTRY_ARCHIVE_BYTES
        ));
    }
    let dec = flate2::read::GzDecoder::new(&bytes[..]);
    let mut archive = tar::Archive::new(dec);
    let entries = archive
        .entries()
        .map_err(|e| format!("tarball entries: {e}"))?;
    let mut has_manifest = false;
    let mut has_entry = false;
    let mut entry_count = 0usize;
    let mut unpacked_bytes = 0u64;
    for entry in entries {
        let entry = entry.map_err(|e| format!("tarball entry: {e}"))?;
        entry_count = entry_count
            .checked_add(1)
            .ok_or_else(|| "tarball entry count overflow".to_string())?;
        if entry_count > MAX_REGISTRY_ENTRIES {
            return Err(format!(
                "tarball entry count exceeds cap of {MAX_REGISTRY_ENTRIES}"
            ));
        }
        let path = entry.path().map_err(|e| format!("entry path: {e}"))?;
        let entry_type = entry.header().entry_type();
        if !matches!(
            entry_type,
            tar::EntryType::Regular | tar::EntryType::Directory
        ) {
            return Err(format!(
                "tarball entry type is not allowed: {}",
                path.display()
            ));
        }
        let entry_bytes = if entry_type == tar::EntryType::Regular {
            entry
                .header()
                .size()
                .map_err(|e| format!("entry size: {e}"))?
        } else {
            0
        };
        if entry_bytes > MAX_REGISTRY_ENTRY_BYTES {
            return Err(format!(
                "tarball entry '{}' exceeds cap of {} bytes",
                path.display(),
                MAX_REGISTRY_ENTRY_BYTES
            ));
        }
        unpacked_bytes = unpacked_bytes
            .checked_add(entry_bytes)
            .ok_or_else(|| "tarball unpacked size overflow".to_string())?;
        if unpacked_bytes > MAX_REGISTRY_UNPACKED_BYTES {
            return Err(format!(
                "tarball unpacked content exceeds cap of {} bytes",
                MAX_REGISTRY_UNPACKED_BYTES
            ));
        }
        let path_str = path.to_string_lossy();
        if path_str.ends_with("pblang.toml") || path_str == "pblang.toml" {
            has_manifest = true;
        }
        if has_module_extension(&path_str) {
            has_entry = true;
        }
    }
    if !has_manifest {
        return Err("tarball is missing pblang.toml".to_string());
    }
    if !has_entry {
        return Err("tarball has no .sapp/.sap entry file".to_string());
    }
    // HMAC sidecar check
    let sidecar = tarball_path.with_extension("").with_extension(
        tarball_path
            .extension()
            .map(|e| format!("{}.sha256", e.to_string_lossy()))
            .unwrap_or_else(|| "sha256".to_string()),
    );
    if sidecar.is_file() {
        if let Ok(key) = std::env::var("PBLANG_SIGNING_KEY") {
            let expected =
                std::fs::read_to_string(&sidecar).map_err(|e| format!("read sidecar: {e}"))?;
            let expected = expected.trim();
            let got = compute_tarball_hmac(&bytes, key.as_bytes());
            if !got.eq_ignore_ascii_case(expected) {
                return Err(format!("HMAC mismatch: expected {} got {}", expected, got));
            }
        }
    }
    Ok(())
}

/// Read target_platform and min_runtime hints from pblang.toml, if present.
/// Returns None when manifest is missing or unparseable (non-fatal for the caller).
pub fn get_manifest_platform_hints(
    manifest_dir: &Path,
) -> Option<(Option<String>, Option<String>)> {
    let man_path = manifest_dir.join("pblang.toml");
    let man_bytes = std::fs::read(&man_path).ok()?;
    let man_str = std::str::from_utf8(&man_bytes).ok()?;
    let man = parse_manifest(man_str).ok()?;
    let pkg = man.package?;
    Some((pkg.target_platform, pkg.min_runtime))
}

// ──────────────────────────────────────────────────────────────
// Internal helpers
// ──────────────────────────────────────────────────────────────

fn canonicalize_existing(p: &Path) -> Option<PathBuf> {
    std::fs::canonicalize(p).ok()
}

fn base64_encode(data: &[u8]) -> String {
    use std::fmt::Write;
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };
        let _ = write!(
            out,
            "{}{}{}{}",
            CHARS[b0 >> 2] as char,
            CHARS[((b0 & 3) << 4) | (b1 >> 4)] as char,
            if chunk.len() > 1 {
                CHARS[((b1 & 0xf) << 2) | (b2 >> 6)] as char
            } else {
                '='
            },
            if chunk.len() > 2 {
                CHARS[b2 & 0x3f] as char
            } else {
                '='
            },
        );
    }
    out
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{:02X}", b);
            }
        }
    }
    out
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::thread;
    use std::time::Duration;

    #[test]
    fn module_extensions_recognized() {
        for ext in ["sapp", "sap", "bpx", "pbx", "pb", "bp"] {
            assert!(is_module_ext(ext), "ext {ext} should be recognized");
            assert!(has_module_extension(&format!("main.{ext}")));
        }
        assert!(!is_module_ext("txt"));
        assert!(!is_module_ext("rs"));
        assert!(!has_module_extension("main.txt"));
        // bare names and names that merely contain an ext substring must not match
        assert!(!has_module_extension("pb"));
        assert!(!has_module_extension("notapb"));
        assert!(has_module_extension("a/b/c/thing.pb"));
    }

    #[test]
    fn installs_one_registry_dep_to_cache() {
        let temp = std::env::temp_dir().join("pblang_registry_test");
        let _ = std::fs::create_dir_all(&temp);
        let tarball_path = temp.join("pkg-1.0.0.tar.gz");
        {
            let f = std::fs::File::create(&tarball_path).unwrap();
            let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            let mut tar_builder = tar::Builder::new(&mut enc);
            let lib_content = b"export fn id(x) { return x; }";
            let manifest = b"[package]\nname = \"test\"\nversion = \"1.0.0\"\n\n[dependencies]\n";
            for (path, content) in [
                ("lib.sapp", lib_content.as_slice()),
                ("pblang.toml", manifest.as_slice()),
            ] {
                let mut header = tar::Header::new_gnu();
                header.set_path(path).unwrap();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                tar_builder.append(&header, content).unwrap();
            }
            tar_builder.finish().unwrap();
            drop(tar_builder);
            enc.finish().unwrap();
        }
        let tarball_abs = canonicalize_existing(&tarball_path).unwrap_or(tarball_path.clone());
        let archive_sha256 = hex::encode(Sha256::digest(
            std::fs::read(&tarball_path).expect("read registry fixture"),
        ));
        let tarball_url = format!(
            "file:///{}",
            tarball_abs.to_string_lossy().replace('\\', "/")
        );

        let index = serde_json::json!({
            "packages": {
                "@pblang/test": {
                    "1.0.0": { "url": tarball_url, "sha256": archive_sha256 }
                }
            }
        });
        let index_path = temp.join("index.json");
        std::fs::write(&index_path, index.to_string()).unwrap();
        let index_abs = canonicalize_existing(&index_path).unwrap_or(index_path.clone());
        let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));

        let project_dir = std::env::temp_dir().join("pblang_registry_project");
        let _ = std::fs::create_dir_all(&project_dir);
        let manifest = r#"[package]
name = "reg-test"
version = "0.1.0"

[dependencies]
"@pblang/test" = "1.0.0"
"#;
        std::fs::write(project_dir.join("pblang.toml"), manifest).unwrap();

        let lock =
            generate_lockfile_with_registry_policy(&project_dir, Some(&index_url), false, true)
                .expect("generate");
        let dep = lock.dependencies.get("@pblang/test").expect("dep");
        assert_eq!(dep.source.as_deref(), Some("registry"));
        assert_eq!(dep.version.as_deref(), Some("1.0.0"));
        assert!(!dep.path.is_empty());
        assert!(
            std::path::Path::new(&dep.path).exists(),
            "cache path should exist: {}",
            dep.path
        );
        assert!(!dep.sha256.is_empty());

        let _ = std::fs::remove_dir_all(&temp);
        let _ = std::fs::remove_dir_all(&project_dir);
    }

    fn make_tarball_and_sha256(temp: &Path) -> (std::path::PathBuf, String, String) {
        let tarball_path = temp.join("pkg-1.0.0.tar.gz");
        let lib_content = b"export fn id(x) { return x; }";
        let manifest = b"[package]\nname = \"test\"\nversion = \"1.0.0\"\n\n[dependencies]\n";
        {
            let f = std::fs::File::create(&tarball_path).unwrap();
            let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
            let mut tar_builder = tar::Builder::new(&mut enc);
            for (path, content) in [
                ("lib.sapp", lib_content.as_slice()),
                ("pblang.toml", manifest.as_slice()),
            ] {
                let mut header = tar::Header::new_gnu();
                header.set_path(path).unwrap();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                tar_builder.append(&header, content).unwrap();
            }
            tar_builder.finish().unwrap();
            drop(tar_builder);
            enc.finish().unwrap();
        }
        let bytes = std::fs::read(&tarball_path).unwrap();
        let sha256 = hex::encode(Sha256::digest(&bytes));
        let tarball_abs = canonicalize_existing(&tarball_path).unwrap_or(tarball_path.clone());
        let tarball_url = format!(
            "file:///{}",
            tarball_abs.to_string_lossy().replace('\\', "/")
        );
        (tarball_path, tarball_url, sha256)
    }

    #[test]
    fn registry_checksum_mismatch_fails() {
        let temp = std::env::temp_dir().join("pblang_registry_checksum_test");
        let _ = std::fs::create_dir_all(&temp);
        let (_tarball_path, tarball_url, _sha256) = make_tarball_and_sha256(&temp);
        let index = serde_json::json!({
            "packages": {
                "@pblang/pkg": {
                    "1.0.0": { "url": tarball_url, "sha256": "0000000000000000000000000000000000000000000000000000000000000000" }
                }
            }
        });
        let index_path = temp.join("index.json");
        std::fs::write(&index_path, index.to_string()).unwrap();
        let index_abs = canonicalize_existing(&index_path).unwrap_or(index_path.clone());
        let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));
        let cache = temp.join("cache");
        let r = resolve_registry_dep_with_policy(
            &index_url,
            &cache,
            "@pblang/pkg",
            "1.0.0",
            false,
            true,
        );
        assert!(r.is_err(), "expected checksum mismatch error");
        let err = r.unwrap_err();
        assert!(
            err.contains("checksum mismatch"),
            "error should mention checksum mismatch: {}",
            err
        );
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn registry_with_sha256_passes() {
        let temp = std::env::temp_dir().join("pblang_registry_sha256_ok");
        let _ = std::fs::create_dir_all(&temp);
        let (_tarball_path, tarball_url, sha256) = make_tarball_and_sha256(&temp);
        let index = serde_json::json!({
            "packages": {
                "@pblang/ok": {
                    "1.0.0": { "url": tarball_url, "sha256": sha256 }
                }
            }
        });
        let index_path = temp.join("index.json");
        std::fs::write(&index_path, index.to_string()).unwrap();
        let index_abs = canonicalize_existing(&index_path).unwrap_or(index_path.clone());
        let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));
        let cache = temp.join("cache");
        let r = resolve_registry_dep_with_policy(
            &index_url,
            &cache,
            "@pblang/ok",
            "1.0.0",
            false,
            true,
        );
        let (path, _content_sha, archive_sha, _url) =
            r.expect("resolve should succeed with correct sha256");
        assert_eq!(archive_sha, sha256);
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn registry_cache_rejects_preseeded_mismatched_content() {
        let temp = std::env::temp_dir().join(format!(
            "pblang_registry_preseed_rejected_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let (_tarball_path, tarball_url, sha256) = make_tarball_and_sha256(&temp);
        let index = serde_json::json!({
            "packages": {
                "@pblang/preseed": {
                    "1.0.0": { "url": tarball_url, "sha256": sha256 }
                }
            }
        });
        let index_path = temp.join("index.json");
        std::fs::write(&index_path, index.to_string()).unwrap();
        let index_url = format!("file://{}", index_path.to_string_lossy());
        let cache = temp.join("cache");
        std::fs::create_dir(&cache).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&cache, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let seeded = cache.join(&sha256);
        std::fs::create_dir(&seeded).unwrap();
        std::fs::write(
            seeded.join("lib.sapp"),
            "print \"preseeded attacker content\";",
        )
        .unwrap();
        std::fs::write(
            seeded.join("pblang.toml"),
            "[package]\nname=\"preseed\"\nversion=\"1.0.0\"\n",
        )
        .unwrap();

        let error = resolve_registry_dep_with_policy(
            &index_url,
            &cache,
            "@pblang/preseed",
            "1.0.0",
            false,
            true,
        )
        .expect_err("mismatched pre-seeded content must fail closed");
        assert!(
            error.contains("cache collision"),
            "unexpected error: {error}"
        );
        assert_eq!(
            std::fs::read_to_string(seeded.join("lib.sapp")).unwrap(),
            "print \"preseeded attacker content\";",
            "failed install must not overwrite the existing cache entry"
        );
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn registry_sha256_is_required_even_when_legacy_flag_is_false() {
        let temp = std::env::temp_dir().join("pblang_registry_required_sha");
        let _ = std::fs::create_dir_all(&temp);
        let (_tarball_path, tarball_url, _sha256) = make_tarball_and_sha256(&temp);
        let index = serde_json::json!({
            "packages": {
                "@pblang/nosha": {
                    "1.0.0": { "url": tarball_url, "sha256": null }
                }
            }
        });
        let index_path = temp.join("index.json");
        std::fs::write(&index_path, index.to_string()).unwrap();
        let index_abs = canonicalize_existing(&index_path).unwrap_or(index_path.clone());
        let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));
        let cache = temp.join("cache");
        let r = resolve_registry_dep_with_policy(
            &index_url,
            &cache,
            "@pblang/nosha",
            "1.0.0",
            false,
            true,
        );
        assert!(r.is_err(), "expected sha256 required error");
        let err = r.unwrap_err();
        assert!(
            err.contains("sha256 required"),
            "error should mention sha256 required: {}",
            err
        );
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn registry_index_rejects_malformed_sha256() {
        let index = br#"{
            "packages": {
                "@pblang/bad-digest": {
                    "1.0.0": { "url": "pkg.tar.gz", "sha256": "not-a-digest" }
                }
            }
        }"#;
        let err = parse_registry_index(index).expect_err("malformed digest must fail closed");
        assert!(err.contains("malformed sha256"), "unexpected error: {err}");
    }

    #[test]
    fn registry_index_requires_publisher_signature_by_default() {
        let temp = std::env::temp_dir().join(format!(
            "pblang_registry_unsigned_rejected_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let index_path = temp.join("index.json");
        std::fs::write(&index_path, br#"{"packages":{}}"#).unwrap();
        let index_url = format!("file://{}", index_path.to_string_lossy());
        let err = fetch_authenticated_registry_index(&index_url, false)
            .expect_err("unsigned registry must fail closed");
        assert!(
            err.contains("signed registry required"),
            "unexpected error: {err}"
        );
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn unsigned_registry_development_mode_rejects_remote_urls_before_fetch() {
        for url in [
            "https://example.invalid/index.json",
            "http://localhost.example.invalid/index.json",
            "https://127.0.0.1.example.invalid/index.json",
            "http://evil.example\\@localhost/index.json",
            "http://user@localhost/index.json",
            "http://user:password@127.0.0.1/index.json",
        ] {
            let err = fetch_authenticated_registry_index(url, true)
                .expect_err("remote unsigned registry must fail before network access");
            assert!(
                err.contains("restricted to file:// and loopback"),
                "{url}: {err}"
            );
        }
        assert!(unsigned_registry_dev_allowed("file:///tmp/index.json", true).unwrap());
        assert!(unsigned_registry_dev_allowed("http://127.0.0.1:8080/index.json", true).unwrap());
        assert!(unsigned_registry_dev_allowed("https://[::1]/index.json", true).unwrap());
        assert!(local_file_registry_path("file:///tmp/index.json").is_some());
        for url in [
            "file://server/share/index.json",
            "file://\\\\server\\share\\index.json",
            "file:///tmp/index.json?redirect=remote",
        ] {
            assert!(
                local_file_registry_path(url).is_none(),
                "remote or ambiguous file URL must fail: {url}"
            );
            assert!(
                unsigned_registry_dev_allowed(url, true).is_err(),
                "unsigned remote file URL must fail: {url}"
            );
        }
        #[cfg(windows)]
        {
            let unc = "file:////server/share/index.json";
            assert!(local_file_registry_path(unc).is_none());
            assert!(unsigned_registry_dev_allowed(unc, true).is_err());
        }
    }

    #[test]
    fn registry_fetch_does_not_follow_redirects() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind redirect fixture");
        let address = listener.local_addr().expect("fixture address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept redirect fixture");
            let mut request = [0u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: https://example.invalid/remote\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .expect("write redirect response");
        });

        std::env::set_var("PBLANG_ALLOW_INSECURE_HTTP", "1");
        let result = fetch_url_with_limit(
            &format!("http://{address}/index.json"),
            MAX_REGISTRY_INDEX_BYTES,
            "redirect fixture",
        );
        std::env::remove_var("PBLANG_ALLOW_INSECURE_HTTP");
        server.join().expect("redirect fixture server");
        let error = result.expect_err("registry redirects must fail closed");
        assert!(error.contains("302"), "unexpected redirect error: {error}");
    }

    #[test]
    fn package_digest_covers_hidden_and_target_paths() {
        let temp = std::env::temp_dir().join(format!(
            "pblang_package_complete_digest_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(temp.join("lib.sapp"), "export fn ok() { return true; }").unwrap();
        let base = hash_dir(&temp).expect("base digest");

        std::fs::write(
            temp.join(".hidden.sapp"),
            "export fn hidden() { return 1; }",
        )
        .unwrap();
        let hidden = hash_dir(&temp).expect("hidden digest");
        assert_ne!(base, hidden, "hidden package content must affect digest");

        std::fs::create_dir_all(temp.join("target")).unwrap();
        std::fs::write(
            temp.join("target/payload.sapp"),
            "export fn payload() { return 2; }",
        )
        .unwrap();
        let target = hash_dir(&temp).expect("target digest");
        assert_ne!(hidden, target, "target package content must affect digest");
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn package_digest_encoding_is_structurally_unambiguous() {
        let base = std::env::temp_dir().join(format!(
            "pblang_package_digest_domain_separation_{}",
            std::process::id()
        ));
        let first = base.join("first");
        let second = base.join("second");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();

        std::fs::write(
            first.join("a.sapp"),
            b"safe\0lib.sapp\0malicious".as_slice(),
        )
        .unwrap();
        std::fs::write(second.join("a.sapp"), b"safe".as_slice()).unwrap();
        std::fs::write(second.join("lib.sapp"), b"malicious".as_slice()).unwrap();

        assert_ne!(
            hash_dir(&first).expect("first digest"),
            hash_dir(&second).expect("second digest"),
            "distinct package structures must never share a serialized digest input"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn package_validation_rejects_entry_escape() {
        let temp = std::env::temp_dir().join(format!(
            "pblang_package_entry_escape_{}",
            std::process::id()
        ));
        let outside = temp.with_extension("sapp");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(&outside, "export fn outside() { return 1; }").unwrap();
        std::fs::write(temp.join("lib.sapp"), "export fn ok() { return true; }").unwrap();
        std::fs::write(
            temp.join("pblang.toml"),
            "[package]\nname = \"escape\"\nversion = \"0.1.0\"\nentry = \"../outside.sapp\"\n",
        )
        .unwrap();
        let error = validate_package_tree(&temp).expect_err("entry traversal must fail");
        assert!(error.contains("stay within"), "unexpected error: {error}");
        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[cfg(unix)]
    #[test]
    fn package_digest_and_entry_reject_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = std::env::temp_dir().join(format!(
            "pblang_package_symlink_rejected_{}",
            std::process::id()
        ));
        let outside = temp.with_extension("outside.sapp");
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::write(&outside, "export fn outside() { return 1; }").unwrap();
        std::fs::write(
            temp.join("pblang.toml"),
            "[package]\nname = \"links\"\nversion = \"0.1.0\"\nentry = \"main.sapp\"\n",
        )
        .unwrap();
        symlink(&outside, temp.join("main.sapp")).unwrap();

        let digest_error = hash_dir(&temp).expect_err("symlink must not be hashed by target");
        assert!(
            digest_error.contains("symlink"),
            "unexpected error: {digest_error}"
        );
        let validation_error =
            validate_package_tree(&temp).expect_err("symlink manifest entry must fail");
        assert!(
            validation_error.contains("symlink"),
            "unexpected error: {validation_error}"
        );
        let _ = std::fs::remove_file(&outside);
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn registry_index_accepts_trusted_ed25519_publisher_and_rejects_tampering() {
        use ring::signature::KeyPair as _;

        let index = br#"{"packages":{"@pblang/signed":{"1.0.0":{"url":"signed.tar.gz","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}}}"#;
        let seed = [7u8; 32];
        let key_pair = ring::signature::Ed25519KeyPair::from_seed_unchecked(&seed).unwrap();
        let public_key = key_pair.public_key().as_ref();
        let publisher = "pblang-test-publisher";
        let sidecar = serde_json::to_vec(&RegistryIndexSignature {
            schema: "pblang-registry-signature-v1".to_string(),
            publisher: publisher.to_string(),
            key_id: hex::encode(Sha256::digest(public_key)),
            algorithm: "ed25519".to_string(),
            index_sha256: hex::encode(Sha256::digest(index)),
            signature: base64::engine::general_purpose::STANDARD
                .encode(key_pair.sign(index).as_ref()),
        })
        .unwrap();
        let trust = serde_json::to_vec(&serde_json::json!({
            publisher: hex::encode(public_key)
        }))
        .unwrap();

        let verified = verify_registry_index_signature(index, &sidecar, &trust).unwrap();
        assert_eq!(verified, publisher);
        let mut tampered = index.to_vec();
        tampered.push(b' ');
        let err = verify_registry_index_signature(&tampered, &sidecar, &trust)
            .expect_err("tampered index must fail");
        assert!(err.contains("digest"), "unexpected error: {err}");
    }

    #[test]
    fn registry_unpack_entry_cap_is_transactional() {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            for index in 0..=MAX_REGISTRY_ENTRIES {
                let mut header = tar::Header::new_gnu();
                header
                    .set_path(format!("entries/{index}.sapp"))
                    .expect("set bounded fixture path");
                header.set_size(0);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append(&header, std::io::empty())
                    .expect("append bounded fixture entry");
            }
            builder.finish().expect("finish bounded fixture tar");
        }
        let archive = encoder.finish().expect("finish bounded fixture gzip");
        let dest = std::env::temp_dir().join("pblang_registry_entry_cap_transactional");
        let _ = std::fs::remove_dir_all(&dest);

        let err = unpack_tarball_gz(&archive, &dest).expect_err("entry cap must reject archive");
        assert!(
            err.contains("entry count exceeds cap"),
            "unexpected error: {err}"
        );
        assert!(
            !dest.exists(),
            "failed extraction must not leave a partial cache tree"
        );
    }

    #[test]
    fn validate_manifest_metadata_rejects_invalid_platform() {
        let tmp = std::env::temp_dir().join("pblang_validate_meta_test");
        let _ = std::fs::create_dir_all(&tmp);
        std::fs::write(
            tmp.join("pblang.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\ntarget_platform = \"solaris\"\n[dependencies]\n",
        ).unwrap();
        let r = validate_manifest_metadata(&tmp);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("target_platform"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn validate_manifest_metadata_rejects_invalid_min_runtime() {
        let tmp = std::env::temp_dir().join("pblang_validate_min_rt_test");
        let _ = std::fs::create_dir_all(&tmp);
        std::fs::write(
            tmp.join("pblang.toml"),
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\nmin_runtime = \"not-a-version\"\n[dependencies]\n",
        ).unwrap();
        let r = validate_manifest_metadata(&tmp);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("min_runtime"));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn compute_tarball_hmac_produces_hex() {
        let h = compute_tarball_hmac(b"hello world", b"secret");
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn lockfile_generation_is_reproducible_for_path_deps() {
        let tmp = std::env::temp_dir().join("pblang_lockfile_repro");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::create_dir_all(tmp.join("dep")).unwrap();
        std::fs::write(
            tmp.join("pblang.toml"),
            r#"[package]
name = "repro"
version = "0.1.0"

[dependencies]
"dep" = { path = "./dep" }
"#,
        )
        .unwrap();
        std::fs::write(
            tmp.join("dep").join("lib.sapp"),
            "export fn x() { return 1; }\n",
        )
        .unwrap();
        let lock1 = generate_lockfile(&tmp).expect("lock1");
        let lock2 = generate_lockfile(&tmp).expect("lock2");
        let s1 = write_lockfile(&lock1).expect("write1");
        let s2 = write_lockfile(&lock2).expect("write2");
        assert_eq!(s1, s2, "lockfile output should be deterministic");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn legacy_lockfile_hash_algorithm_requires_explicit_refresh() {
        let legacy = r#"manifest_sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[dependencies]
"#;
        let error = parse_lockfile(legacy).expect_err("legacy lock must not be silently rehashed");
        assert!(
            error.contains("missing/legacy") && error.contains("pblang install"),
            "unexpected migration error: {error}"
        );
    }

    #[test]
    fn declared_bare_import_never_falls_through_a_broken_lock() {
        let tmp = std::env::temp_dir().join(format!(
            "pblang_declared_import_fail_closed_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("vendor/dep")).expect("create dependency fixture");
        std::fs::write(
            tmp.join("pblang.toml"),
            "[package]\nname=\"root\"\nversion=\"0.1.0\"\n\n[dependencies]\ndep={path=\"./vendor/dep\"}\n",
        )
        .expect("write manifest");
        std::fs::write(
            tmp.join("vendor/dep/lib.sapp"),
            "export fn trusted() { return 1; }",
        )
        .expect("write declared dependency");
        std::fs::write(tmp.join("dep"), "print \"shadow\";")
            .expect("write attacker-controlled shadow");

        let missing = resolve_import("dep", Some(&tmp), &tmp)
            .expect_err("declared dependency without lock must fail closed");
        assert!(
            missing.contains("no pblang.lock"),
            "unexpected error: {missing}"
        );

        std::fs::write(tmp.join("pblang.lock"), "not valid lock syntax")
            .expect("write malformed lock");
        let malformed = resolve_import("dep", Some(&tmp), &tmp)
            .expect_err("malformed lock must not fall through to shadow");
        assert!(
            malformed.contains("failed to parse pblang.lock"),
            "unexpected error: {malformed}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn declared_subpath_import_requires_current_complete_lock_and_tree() {
        let tmp = std::env::temp_dir().join(format!(
            "pblang_declared_subpath_fail_closed_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("vendor/dep")).expect("create dependency fixture");
        std::fs::write(
            tmp.join("pblang.toml"),
            "[package]\nname=\"root\"\nversion=\"0.1.0\"\n\n[dependencies]\ndep={path=\"./vendor/dep\"}\n",
        )
        .expect("write manifest");
        std::fs::write(
            tmp.join("vendor/dep/lib.sapp"),
            "export fn trusted() { return 1; }",
        )
        .expect("write dependency module");

        let missing = resolve_import("dep:lib.sapp", Some(&tmp), &tmp)
            .expect_err("subpath import without lock must fail closed");
        assert!(
            missing.contains("no pblang.lock"),
            "unexpected error: {missing}"
        );

        std::fs::write(tmp.join("pblang.lock"), [0xff, 0xfe]).expect("write non-UTF8 lock");
        let non_utf8 = resolve_import("dep:lib.sapp", Some(&tmp), &tmp)
            .expect_err("non-UTF8 lock must fail closed");
        assert!(
            non_utf8.contains("not UTF-8"),
            "unexpected error: {non_utf8}"
        );

        let lock = generate_lockfile(&tmp).expect("generate current lock");
        let mut incomplete_lock = lock.clone();
        incomplete_lock.dependencies.remove("dep");
        std::fs::write(
            tmp.join("pblang.lock"),
            write_lockfile(&incomplete_lock).expect("serialize incomplete lock"),
        )
        .expect("write incomplete lock");
        let missing_entry = resolve_import("dep:lib.sapp", Some(&tmp), &tmp)
            .expect_err("declared subpath missing from lock must fail closed");
        assert!(
            missing_entry.contains("missing from pblang.lock"),
            "unexpected error: {missing_entry}"
        );

        std::fs::write(
            tmp.join("pblang.lock"),
            write_lockfile(&lock).expect("serialize current lock"),
        )
        .expect("write current lock");
        let resolved = resolve_import("dep:lib.sapp", Some(&tmp), &tmp)
            .expect("current locked subpath must resolve");
        assert_eq!(
            resolved,
            tmp.join("vendor/dep/lib.sapp").canonicalize().unwrap()
        );

        std::fs::write(
            tmp.join("vendor/dep/lib.sapp"),
            "export fn changed() { return 2; }",
        )
        .expect("mutate dependency after lock");
        let changed = resolve_import("dep:lib.sapp", Some(&tmp), &tmp)
            .expect_err("tree hash mismatch must fail closed");
        assert!(
            changed.contains("hash mismatch"),
            "unexpected error: {changed}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn undeclared_local_import_retains_compatibility() {
        let tmp = std::env::temp_dir().join(format!(
            "pblang_undeclared_local_import_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).expect("create import fixture");
        std::fs::write(
            tmp.join("pblang.toml"),
            "[package]\nname=\"root\"\nversion=\"0.1.0\"\n",
        )
        .expect("write manifest");
        std::fs::write(tmp.join("local.sapp"), "print 1;").expect("write local module");
        let resolved = resolve_import("local.sapp", Some(&tmp), &tmp)
            .expect("undeclared local source remains resolvable");
        assert_eq!(resolved, tmp.join("local.sapp").canonicalize().unwrap());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_registry_release_url_supports_relative_paths() {
        let base = "file:///tmp/registry/index.json";
        assert_eq!(
            resolve_registry_release_url(base, "pkg-1.0.0.tar.gz"),
            "file:///tmp/registry/pkg-1.0.0.tar.gz"
        );
    }

    #[test]
    fn fetch_url_rejects_plain_http_by_default() {
        std::env::remove_var("PBLANG_ALLOW_INSECURE_HTTP");
        let err = fetch_url("http://example.invalid/index.json").unwrap_err();
        assert!(err.contains("insecure registry URL rejected"));
    }

    fn read_http_request(stream: &mut TcpStream) -> (String, String, String, String) {
        let mut buf = Vec::new();
        let mut tmp = [0u8; 1024];
        let mut header_end = None;
        loop {
            let n = stream.read(&mut tmp).expect("read request bytes");
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&tmp[..n]);
            if header_end.is_none() {
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = Some(pos + 4);
                    break;
                }
            }
        }

        let header_end = header_end.expect("request header terminator");
        let headers_raw = String::from_utf8(buf[..header_end].to_vec()).expect("utf8 header");
        let mut lines = headers_raw.split("\r\n").filter(|l| !l.is_empty());
        let request_line = lines.next().expect("request line").to_string();
        let path = request_line
            .split_whitespace()
            .nth(1)
            .expect("request path")
            .to_string();
        let auth = lines
            .find_map(|line| {
                let lower = line.to_ascii_lowercase();
                if lower.starts_with("authorization:") {
                    Some(line["Authorization:".len()..].trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let content_length = headers_raw
            .lines()
            .find_map(|line| {
                let lower = line.to_ascii_lowercase();
                if lower.starts_with("content-length:") {
                    line.split(':')
                        .nth(1)
                        .and_then(|v| v.trim().parse::<usize>().ok())
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let mut body = buf[header_end..].to_vec();
        while body.len() < content_length {
            let n = stream.read(&mut tmp).expect("read request body");
            if n == 0 {
                break;
            }
            body.extend_from_slice(&tmp[..n]);
        }
        let body = String::from_utf8(body).expect("utf8 body");
        (request_line, path, auth, body)
    }

    fn write_http_response(stream: &mut TcpStream, code: u16, body: &str) {
        let reason = match code {
            200 => "OK",
            401 => "Unauthorized",
            _ => "Error",
        };
        let resp = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            code,
            reason,
            body.len(),
            body
        );
        stream.write_all(resp.as_bytes()).expect("write response");
        stream.flush().expect("flush response");
    }

    #[test]
    fn refresh_access_token_json_escapes_refresh_token() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("server addr");

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept connection");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("set read timeout");
            let (_, path, _, body) = read_http_request(&mut stream);
            assert_eq!(path, "/auth/refresh");
            let parsed: serde_json::Value =
                serde_json::from_str(&body).expect("refresh request body should be valid JSON");
            assert_eq!(parsed["refresh_token"], "refresh\"token\\with\\slashes");
            write_http_response(&mut stream, 200, r#"{"token":"escaped-token"}"#);
        });

        let token =
            refresh_access_token(&format!("http://{addr}"), "refresh\"token\\with\\slashes")
                .expect("refresh should succeed");
        assert_eq!(token, "escaped-token");
        server.join().expect("join test server thread");
    }

    #[test]
    fn publish_retries_on_401_with_refresh_token() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let addr = listener.local_addr().expect("server addr");
        let publish_calls = Arc::new(AtomicUsize::new(0));
        let refresh_calls = Arc::new(AtomicUsize::new(0));

        let publish_calls_server = Arc::clone(&publish_calls);
        let refresh_calls_server = Arc::clone(&refresh_calls);
        let server = thread::spawn(move || {
            listener
                .set_nonblocking(false)
                .expect("set blocking mode for test server");
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().expect("accept connection");
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .expect("set read timeout");
                let (_, path, auth, body) = read_http_request(&mut stream);
                if path == "/publish" {
                    let n = publish_calls_server.fetch_add(1, Ordering::SeqCst);
                    if n == 0 {
                        assert_eq!(
                            auth, "Bearer stale-token",
                            "first publish should use original token"
                        );
                        write_http_response(&mut stream, 401, r#"{"error":"unauthorized"}"#);
                    } else {
                        assert_eq!(
                            auth, "Bearer refreshed-token",
                            "retry publish should use refreshed token"
                        );
                        assert!(
                            body.contains("\"name\":\"publish-test\""),
                            "retry publish should contain package metadata"
                        );
                        write_http_response(&mut stream, 200, r#"{"ok":true}"#);
                    }
                } else if path == "/auth/refresh" {
                    refresh_calls_server.fetch_add(1, Ordering::SeqCst);
                    assert!(
                        body.contains("\"refresh_token\":\"refresh-token\""),
                        "refresh endpoint should receive refresh token"
                    );
                    write_http_response(&mut stream, 200, r#"{"token":"refreshed-token"}"#);
                } else {
                    panic!("unexpected test request path: {path}");
                }
            }
        });

        let package_dir = std::env::temp_dir().join("pblang_publish_retry_pkg");
        let _ = std::fs::remove_dir_all(&package_dir);
        std::fs::create_dir_all(&package_dir).expect("create package dir");
        std::fs::write(
            package_dir.join("pblang.toml"),
            r#"[package]
name = "publish-test"
version = "0.1.0"
"#,
        )
        .expect("write manifest");
        std::fs::write(
            package_dir.join("main.sapp"),
            r#"print "publish retry test";"#,
        )
        .expect("write source");

        // SAFETY: this test controls process env and restores it before exit.
        #[allow(unused_unsafe)]
        unsafe {
            std::env::set_var("PBLANG_REGISTRY_REFRESH_TOKEN", "refresh-token");
        }
        let registry_url = format!("http://{}", addr);
        let result = publish_package(&registry_url, &package_dir, "stale-token");
        // SAFETY: restore env mutation from this test.
        #[allow(unused_unsafe)]
        unsafe {
            std::env::remove_var("PBLANG_REGISTRY_REFRESH_TOKEN");
        }

        assert!(
            result.is_ok(),
            "publish should succeed after retry: {result:?}"
        );
        assert_eq!(
            publish_calls.load(Ordering::SeqCst),
            2,
            "expected publish endpoint to be hit twice (initial + retry)"
        );
        assert_eq!(
            refresh_calls.load(Ordering::SeqCst),
            1,
            "expected exactly one token refresh request"
        );

        let _ = std::fs::remove_dir_all(&package_dir);
        server.join().expect("join test server thread");
    }
}
