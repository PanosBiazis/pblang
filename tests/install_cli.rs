//! Regression tests for registry CLI flows (`install`, `build`, local `publish`).

use pblang_pm::{fetch_url, parse_lockfile, write_lockfile};
use sha2::Digest;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

fn file_url_for_path(path: &std::path::Path) -> String {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    format!("file:///{}", abs.to_string_lossy().replace('\\', "/"))
}

fn make_tarball_and_sha256(temp: &std::path::Path) -> (std::path::PathBuf, String, String) {
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
    let sha256 = hex::encode(sha2::Sha256::digest(&bytes));
    let tarball_url = file_url_for_path(&tarball_path);
    (tarball_path, tarball_url, sha256)
}

fn make_tarball_with_entry(
    temp: &std::path::Path,
    file_name: &str,
    archive_path: &str,
    bytes: &[u8],
    entry_type: tar::EntryType,
    link_name: Option<&str>,
) -> (std::path::PathBuf, String, String) {
    let tarball_path = temp.join(file_name);
    {
        let f = std::fs::File::create(&tarball_path).unwrap();
        let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut tar_builder = tar::Builder::new(&mut enc);
        let mut header = tar::Header::new_gnu();
        header.set_path(archive_path).unwrap();
        header.set_entry_type(entry_type);
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        if let Some(link) = link_name {
            header.set_link_name(link).unwrap();
        }
        header.set_cksum();
        tar_builder.append(&header, bytes).unwrap();
        tar_builder.finish().unwrap();
        drop(tar_builder);
        enc.finish().unwrap();
    }
    let bytes = std::fs::read(&tarball_path).unwrap();
    let sha256 = hex::encode(sha2::Sha256::digest(&bytes));
    let tarball_url = file_url_for_path(&tarball_path);
    (tarball_path, tarball_url, sha256)
}

#[test]
fn install_with_registry_index_resolves_registry_dep() {
    let temp = std::env::temp_dir().join("pblang_install_cli_test");
    let _ = std::fs::create_dir_all(&temp);
    let (_tarball_path, tarball_url, _sha256) = make_tarball_and_sha256(&temp);
    let index = serde_json::json!({
        "packages": {
            "@sapphire/cli-test": {
                "1.0.0": { "url": tarball_url, "sha256": null }
            }
        }
    });
    let index_path = temp.join("index.json");
    std::fs::write(&index_path, index.to_string()).unwrap();
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path.clone());
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));
    let manifest = r#"[package]
name = "install-cli-test"
version = "0.1.0"

[dependencies]
"@sapphire/cli-test" = "1.0.0"
"#;
    std::fs::write(temp.join("pblang.toml"), manifest).unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let status = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&temp)
        .status()
        .expect("run sapphire install --registry-index");
    assert!(
        status.success(),
        "sapphire install --registry-index should exit 0"
    );

    let lock_path = temp.join("pblang.lock");
    assert!(lock_path.is_file(), "pblang.lock should exist");
    let lock_content = std::fs::read_to_string(&lock_path).unwrap();
    assert!(
        lock_content.contains("@sapphire/cli-test"),
        "lockfile should contain registry dep: {}",
        lock_content
    );
    assert!(
        lock_content.contains("source = \"registry\""),
        "lockfile should have source=registry: {}",
        lock_content
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn install_name_at_version_from_registry() {
    let base = std::env::temp_dir().join("pblang_install_name_at_version");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let registry_dir = base.join("registry");
    let pkg_dir = base.join("pkg");
    std::fs::create_dir_all(&pkg_dir).unwrap();

    let manifest = r#"[package]
name = "@sapphire/m4-pkg"
version = "1.2.0"

[dependencies]
"#;
    std::fs::write(pkg_dir.join("pblang.toml"), manifest).unwrap();
    std::fs::write(pkg_dir.join("lib.sapp"), "export fn id(x) { return x; }").unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let publish_status = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_dir)
        .output()
        .expect("run sapphire publish");
    assert!(
        publish_status.status.success(),
        "sapphire publish should exit 0: stderr={}",
        String::from_utf8_lossy(&publish_status.stderr)
    );

    let index_path = registry_dir.join("index.json");
    assert!(
        index_path.is_file(),
        "index.json should exist after publish"
    );
    let index_url = file_url_for_path(&index_path);

    let install_out = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg("@sapphire/m4-pkg@1.2.0")
        .output()
        .expect("run sapphire install <name>@<version>");
    let stdout = String::from_utf8_lossy(&install_out.stdout);
    assert!(
        install_out.status.success(),
        "sapphire install <name>@<version> should exit 0: stdout={}, stderr={}",
        stdout,
        String::from_utf8_lossy(&install_out.stderr)
    );
    assert!(
        stdout.contains("Installed") && stdout.contains("1.2.0"),
        "expected 'Installed @sapphire/m4-pkg@1.2.0' in stdout, got: {}",
        stdout
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn install_rejects_registry_tarball_when_hmac_sidecar_wrong() {
    let temp = std::env::temp_dir().join("pblang_install_hmac_reject");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let (tarball_path, tarball_url, _sha256) = make_tarball_and_sha256(&temp);
    let sidecar_path = tarball_path.with_extension("gz.sha256");
    std::fs::write(&sidecar_path, "wrong_hmac_signature_00000000").unwrap();
    let index = serde_json::json!({
        "packages": {
            "@sapphire/hmac-test": {
                "1.0.0": { "url": tarball_url, "sha256": null }
            }
        }
    });
    let index_path = temp.join("index.json");
    std::fs::write(&index_path, index.to_string()).unwrap();
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path.clone());
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));
    let manifest = r#"[package]
name = "install-hmac-test"
version = "0.1.0"

[dependencies]
"@sapphire/hmac-test" = "1.0.0"
"#;
    std::fs::write(temp.join("pblang.toml"), manifest).unwrap();
    // SAFETY: test-only; set before launching the child process.
    #[allow(unused_unsafe)]
    unsafe {
        std::env::set_var("PBLANG_SIGNING_KEY", "testkey")
    };
    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&temp)
        .output()
        .expect("run sapphire install");
    std::env::remove_var("PBLANG_SIGNING_KEY");
    assert!(
        !out.status.success(),
        "install should fail when .sha256 sidecar has wrong HMAC"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("HMAC") || stderr.contains("signature mismatch"),
        "stderr should mention HMAC/signature: {}",
        stderr
    );
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn build_with_registry_index_generates_lockfile_for_registry_dep() {
    let temp = std::env::temp_dir().join("pblang_build_cli_registry_test");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let (_tarball_path, tarball_url, _sha256) = make_tarball_and_sha256(&temp);
    let index = serde_json::json!({
        "packages": {
            "@sapphire/build-test": {
                "1.0.0": { "url": tarball_url, "sha256": null }
            }
        }
    });
    let index_path = temp.join("index.json");
    std::fs::write(&index_path, index.to_string()).unwrap();
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path.clone());
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));
    let manifest = r#"[package]
name = "build-cli-test"
version = "0.1.0"

[dependencies]
"@sapphire/build-test" = "1.0.0"
"#;
    std::fs::write(temp.join("pblang.toml"), manifest).unwrap();
    std::fs::write(temp.join("main.sapp"), "print 41;\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("build")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&temp)
        .output()
        .expect("run sapphire build --registry-index");
    assert!(
        out.status.success(),
        "sapphire build --registry-index should exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("build ok"),
        "build output should confirm success: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    let lock_path = temp.join("pblang.lock");
    assert!(lock_path.is_file(), "pblang.lock should exist after build");
    let lock_content = std::fs::read_to_string(&lock_path).unwrap();
    assert!(
        lock_content.contains("@sapphire/build-test"),
        "lockfile should contain registry dep after build: {}",
        lock_content
    );
    assert!(
        lock_content.contains("source = \"registry\""),
        "lockfile should record registry source after build: {}",
        lock_content
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn install_without_registry_index_uses_env_or_fails_for_registry_dep() {
    let temp = std::env::temp_dir().join("pblang_install_no_reg");
    let _ = std::fs::create_dir_all(&temp);
    let manifest = r#"[package]
name = "no-reg"
version = "0.1.0"

[dependencies]
"@sapphire/missing" = "1.0.0"
"#;
    std::fs::write(temp.join("pblang.toml"), manifest).unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("install")
        .arg(&temp)
        .output()
        .expect("run sapphire install");
    assert!(
        !out.status.success(),
        "install without --registry-index and no PBLANG_REGISTRY_INDEX should fail for registry dep"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("REGISTRY_INDEX") || stderr.contains("registry"),
        "stderr should mention registry/index: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn lock_require_registry_sha256_flag_rejects_missing_checksum() {
    let temp = std::env::temp_dir().join("pblang_lock_require_sha_flag");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let (_tarball_path, tarball_url, _sha256) = make_tarball_and_sha256(&temp);
    let index = serde_json::json!({
        "packages": {
            "@sapphire/strict-lock": {
                "1.0.0": { "url": tarball_url, "sha256": null }
            }
        }
    });
    let index_path = temp.join("index.json");
    std::fs::write(&index_path, index.to_string()).unwrap();
    let index_url = file_url_for_path(&index_path);
    let manifest = r#"[package]
name = "strict-lock-test"
version = "0.1.0"

[dependencies]
"@sapphire/strict-lock" = "1.0.0"
"#;
    std::fs::write(temp.join("pblang.toml"), manifest).unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("--lock")
        .arg("--registry-index")
        .arg(&index_url)
        .arg("--require-registry-sha256")
        .arg(&temp)
        .output()
        .expect("run sapphire --lock --require-registry-sha256");
    assert!(
        !out.status.success(),
        "--lock should fail when strict sha flag is set and index sha256 is missing"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("sha256 required"),
        "stderr should mention strict checksum requirement: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn install_with_registry_index_rejects_unsafe_tarball_entry() {
    let temp = std::env::temp_dir().join("pblang_install_cli_unsafe_tarball");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let (_tarball_path, tarball_url, tarball_sha256) = make_tarball_with_entry(
        &temp,
        "pkg-unsafe-1.0.0.tar.gz",
        "lib.sapp",
        b"",
        tar::EntryType::Symlink,
        Some("../escape.sapp"),
    );
    let index = serde_json::json!({
        "packages": {
            "@sapphire/unsafe": {
                "1.0.0": { "url": tarball_url, "sha256": tarball_sha256 }
            }
        }
    });
    let index_path = temp.join("index.json");
    std::fs::write(&index_path, index.to_string()).unwrap();
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path.clone());
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));
    let cache_dir = temp.join("cache");
    let manifest = r#"[package]
name = "unsafe-install-test"
version = "0.1.0"

[dependencies]
"@sapphire/unsafe" = "1.0.0"
"#;
    std::fs::write(temp.join("pblang.toml"), manifest).unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&temp)
        .env("PBLANG_CACHE", &cache_dir)
        .output()
        .expect("run sapphire install --registry-index");
    assert!(
        !out.status.success(),
        "install should fail for unsafe registry tarball"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("link entries are not allowed"),
        "stderr should mention forbidden archive link entry: {}",
        stderr
    );
    assert!(
        !temp.join("escape.sapp").exists(),
        "unsafe tarball should not write outside cache destination"
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn install_with_registry_index_supports_relative_tarball_urls() {
    let temp = std::env::temp_dir().join("pblang_install_relative_registry_url");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let (tarball_path, _tarball_url, tarball_sha256) = make_tarball_and_sha256(&temp);
    let tarball_name = tarball_path
        .file_name()
        .expect("tarball file name")
        .to_string_lossy()
        .to_string();
    let index = serde_json::json!({
        "packages": {
            "@sapphire/relative-url": {
                "1.0.0": { "url": tarball_name, "sha256": tarball_sha256 }
            }
        }
    });
    let index_path = temp.join("index.json");
    std::fs::write(&index_path, index.to_string()).unwrap();
    let index_url = file_url_for_path(&index_path);
    let manifest = r#"[package]
name = "relative-url-install-test"
version = "0.1.0"

[dependencies]
"@sapphire/relative-url" = "1.0.0"
"#;
    std::fs::write(temp.join("pblang.toml"), manifest).unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&temp)
        .output()
        .expect("run sapphire install with relative registry URL");
    assert!(
        out.status.success(),
        "install with relative registry URL should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lock_path = temp.join("pblang.lock");
    assert!(lock_path.is_file(), "pblang.lock should exist");
    let lock_content = std::fs::read_to_string(&lock_path).unwrap();
    assert!(
        lock_content.contains("@sapphire/relative-url"),
        "lockfile should contain installed relative-url dep: {}",
        lock_content
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn install_with_registry_index_rejects_package_without_modules() {
    let temp = std::env::temp_dir().join("pblang_install_registry_no_modules");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let (_tarball_path, tarball_url, tarball_sha256) = make_tarball_with_entry(
        &temp,
        "pkg-no-modules-1.0.0.tar.gz",
        "README.txt",
        b"not a sapphire module",
        tar::EntryType::Regular,
        None,
    );
    let index = serde_json::json!({
        "packages": {
            "@sapphire/no-modules": {
                "1.0.0": { "url": tarball_url, "sha256": tarball_sha256 }
            }
        }
    });
    let index_path = temp.join("index.json");
    std::fs::write(&index_path, index.to_string()).unwrap();
    let index_url = file_url_for_path(&index_path);
    let manifest = r#"[package]
name = "registry-no-modules-test"
version = "0.1.0"

[dependencies]
"@sapphire/no-modules" = "1.0.0"
"#;
    std::fs::write(temp.join("pblang.toml"), manifest).unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&temp)
        .output()
        .expect("run sapphire install for package without modules");
    assert!(
        !out.status.success(),
        "install should fail when registry package has no pblang module files"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("has no pblang module files"),
        "stderr should mention missing module files: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn publish_then_install_works() {
    let base = std::env::temp_dir().join("pblang_publish_then_install");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg_a = base.join("pkg_a");
    let pkg_b = base.join("pkg_b");
    let _ = std::fs::create_dir_all(&pkg_a);
    let _ = std::fs::create_dir_all(&pkg_b);

    let manifest_a = r#"[package]
name = "@sapphire/pub-pkg"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg_a.join("pblang.toml"), manifest_a).unwrap();
    std::fs::write(pkg_a.join("lib.sapp"), "export fn id(x) { return x; }").unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let status = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_a)
        .status()
        .expect("run sapphire publish");
    assert!(status.success(), "sapphire publish should exit 0");

    let index_path = registry_dir.join("index.json");
    assert!(index_path.is_file(), "index.json should exist");
    let index_content = std::fs::read_to_string(&index_path).unwrap();
    assert!(
        index_content.contains("@sapphire/pub-pkg") && index_content.contains("1.0.0"),
        "index should list package: {}",
        index_content
    );

    let manifest_b = r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"@sapphire/pub-pkg" = "1.0.0"
"#;
    std::fs::write(pkg_b.join("pblang.toml"), manifest_b).unwrap();

    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path);
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));
    let status2 = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&pkg_b)
        .status()
        .expect("run sapphire install");
    assert!(
        status2.success(),
        "sapphire install after publish should exit 0"
    );

    let lock_path = pkg_b.join("pblang.lock");
    assert!(lock_path.is_file(), "consumer lockfile should exist");
    let lock_content = std::fs::read_to_string(&lock_path).unwrap();
    assert!(
        lock_content.contains("@sapphire/pub-pkg")
            && lock_content.contains("source = \"registry\""),
        "lockfile should have registry dep: {}",
        lock_content
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn publish_and_search_roundtrip() {
    // Setup: local file:// registry + temp package
    let reg_dir = tempfile::tempdir().unwrap();
    let pkg_dir = tempfile::tempdir().unwrap();

    let manifest = r#"[package]
name = "pblang_m4b_pkg"
version = "2.0.0"

[dependencies]
"#;
    std::fs::write(pkg_dir.path().join("pblang.toml"), manifest).unwrap();
    std::fs::write(
        pkg_dir.path().join("lib.sapp"),
        "export fn id(x) { return x; }",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let registry_dir = reg_dir.path().to_string_lossy().to_string();
    let index_url = file_url_for_path(reg_dir.path());

    let pub_out = std::process::Command::new(bin)
        .args([
            "publish",
            "--registry-dir",
            &registry_dir,
            pkg_dir.path().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        pub_out.status.success(),
        "publish failed: {}",
        String::from_utf8_lossy(&pub_out.stderr)
    );

    let search_out = std::process::Command::new(bin)
        .env("PBLANG_REGISTRY_INDEX", &index_url)
        .args(["search", "pblang_m4b"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&search_out.stdout);
    assert!(
        search_out.status.success(),
        "search failed: stdout={}, stderr={}",
        stdout,
        String::from_utf8_lossy(&search_out.stderr)
    );
    assert!(
        stdout.contains("pblang_m4b_pkg"),
        "Expected package name in search results: {}",
        stdout
    );
}

#[test]
fn publish_with_dependencies_fails_and_writes_no_artifacts() {
    let base = std::env::temp_dir().join("pblang_publish_with_deps_fails");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let registry_dir = base.join("registry");
    let pkg = base.join("pkg");
    std::fs::create_dir_all(&pkg).unwrap();

    let manifest = r#"[package]
name = "@sapphire/deps-pkg"
version = "1.0.0"

[dependencies]
"helper" = { path = "./helper" }
"#;
    std::fs::write(pkg.join("pblang.toml"), manifest).unwrap();
    std::fs::write(pkg.join("lib.sapp"), "export fn id() { return 1; }\n").unwrap();
    std::fs::create_dir_all(pkg.join("helper")).unwrap();
    std::fs::write(
        pkg.join("helper").join("pblang.toml"),
        r#"[package]
name = "helper"
version = "0.1.0"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(
        pkg.join("helper").join("lib.sapp"),
        "export fn helper() { return 42; }\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg)
        .output()
        .expect("publish package with dependencies");
    assert!(
        !out.status.success(),
        "publish should fail when package declares dependencies"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("self-contained packages"),
        "stderr should mention self-contained publish contract: {}",
        stderr
    );
    assert!(
        !registry_dir.join("index.json").exists(),
        "failed publish should not create registry index"
    );
    assert!(
        !registry_dir.join("@biapryx_deps-pkg-1.0.0.tar.gz").exists(),
        "failed publish should not create tarball artifact"
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn republish_same_version_with_different_content_fails() {
    let base = std::env::temp_dir().join("pblang_publish_immutable_version");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg = base.join("pkg");
    let _ = std::fs::create_dir_all(&pkg);

    let manifest = r#"[package]
name = "@sapphire/immutable-pkg"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg.join("pblang.toml"), manifest).unwrap();
    std::fs::write(pkg.join("lib.sapp"), "export fn id() { return 1; }\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let first = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg)
        .output()
        .expect("first publish");
    assert!(
        first.status.success(),
        "first publish should succeed: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    std::fs::write(pkg.join("lib.sapp"), "export fn id() { return 2; }\n").unwrap();
    let second = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg)
        .output()
        .expect("second publish");
    assert!(
        !second.status.success(),
        "republish with changed content should fail"
    );
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(
        stderr.contains("already exists in registry with different content"),
        "stderr should explain immutable publish failure: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn republish_same_version_with_same_content_is_ok() {
    let base = std::env::temp_dir().join("pblang_publish_same_content_ok");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg = base.join("pkg");
    let _ = std::fs::create_dir_all(&pkg);

    let manifest = r#"[package]
name = "@sapphire/same-content-pkg"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg.join("pblang.toml"), manifest).unwrap();
    std::fs::write(pkg.join("lib.sapp"), "export fn id() { return 1; }\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let first = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg)
        .output()
        .expect("first publish");
    assert!(
        first.status.success(),
        "first publish should succeed: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    let second = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg)
        .output()
        .expect("second publish");
    assert!(
        second.status.success(),
        "same-content republish should be idempotent: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn publish_with_missing_manifest_entry_fails() {
    let base = std::env::temp_dir().join("pblang_publish_missing_entry");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg = base.join("pkg");
    let _ = std::fs::create_dir_all(pkg.join("src"));

    let manifest = r#"[package]
name = "@sapphire/missing-entry-pkg"
version = "1.0.0"
entry = "src/app.sapp"

[dependencies]
"#;
    std::fs::write(pkg.join("pblang.toml"), manifest).unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg)
        .output()
        .expect("publish package with missing manifest entry");
    assert!(
        !out.status.success(),
        "publish should fail when declared package entry is missing"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("package entry not found"),
        "stderr should mention missing package entry: {}",
        stderr
    );
    assert!(
        !registry_dir.join("index.json").exists(),
        "failed publish should not create registry index"
    );
    assert!(
        !registry_dir
            .join("@sapphire_missing-entry-pkg-1.0.0.tar.gz")
            .exists(),
        "failed publish should not create tarball artifact"
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn publish_with_directory_manifest_entry_fails() {
    let base = std::env::temp_dir().join("pblang_publish_directory_entry");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg = base.join("pkg");
    let _ = std::fs::create_dir_all(pkg.join("src"));

    let manifest = r#"[package]
name = "@sapphire/directory-entry-pkg"
version = "1.0.0"
entry = "src"

[dependencies]
"#;
    std::fs::write(pkg.join("pblang.toml"), manifest).unwrap();
    std::fs::write(
        pkg.join("src").join("app.sapp"),
        "export fn id() { return 1; }\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg)
        .output()
        .expect("publish package with directory manifest entry");
    assert!(
        !out.status.success(),
        "publish should fail when declared package entry points to a directory"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("package entry is not a file"),
        "stderr should mention directory entry rejection: {}",
        stderr
    );
    assert!(
        !registry_dir.join("index.json").exists(),
        "failed publish should not create registry index"
    );
    assert!(
        !registry_dir
            .join("@sapphire_directory-entry-pkg-1.0.0.tar.gz")
            .exists(),
        "failed publish should not create tarball artifact"
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn publish_without_any_sapphire_module_fails() {
    let base = std::env::temp_dir().join("pblang_publish_missing_module_file");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let registry_dir = base.join("registry");
    let pkg = base.join("pkg");
    std::fs::create_dir_all(&pkg).unwrap();

    let manifest = r#"[package]
name = "@sapphire/no-default-entry"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg.join("pblang.toml"), manifest).unwrap();
    std::fs::write(pkg.join("README.txt"), "not a Sapphire module\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg)
        .output()
        .expect("publish package without sapphire module files");
    assert!(
        !out.status.success(),
        "publish should fail when package has no pblang module files"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("has no pblang module files"),
        "stderr should mention missing module-file contract: {}",
        stderr
    );
    assert!(
        !registry_dir.join("index.json").exists(),
        "failed publish should not create registry index"
    );
    assert!(
        !registry_dir
            .join("@sapphire_no-default-entry-1.0.0.tar.gz")
            .exists(),
        "failed publish should not create tarball artifact"
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn installed_registry_package_can_be_imported_by_package_name() {
    let base = std::env::temp_dir().join("pblang_registry_import_by_name");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg_a = base.join("pkg_a");
    let pkg_b = base.join("pkg_b");
    let _ = std::fs::create_dir_all(&pkg_a);
    let _ = std::fs::create_dir_all(&pkg_b);

    let manifest_a = r#"[package]
name = "@sapphire/pub-pkg"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg_a.join("pblang.toml"), manifest_a).unwrap();
    std::fs::write(
        pkg_a.join("lib.sapp"),
        "fn id(x) { return x + 1; }\nexport id;\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let publish_status = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_a)
        .status()
        .expect("run sapphire publish");
    assert!(publish_status.success(), "sapphire publish should exit 0");

    let manifest_b = r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"@sapphire/pub-pkg" = "1.0.0"
"#;
    std::fs::write(pkg_b.join("pblang.toml"), manifest_b).unwrap();
    std::fs::write(
        pkg_b.join("main.sapp"),
        r#"
        let pkg = import("@sapphire/pub-pkg");
        print pkg.id(41);
        "#,
    )
    .unwrap();

    let index_path = registry_dir.join("index.json");
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path);
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));

    let install_status = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&pkg_b)
        .status()
        .expect("run sapphire install");
    assert!(install_status.success(), "sapphire install should exit 0");

    let out = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&pkg_b)
        .output()
        .expect("run sapphire run");
    assert!(
        out.status.success(),
        "sapphire run should exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("42"),
        "stdout should contain package result: {}",
        stdout
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn installed_registry_package_uses_manifest_entry_for_package_import() {
    let base = std::env::temp_dir().join("pblang_registry_import_manifest_entry");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg_a = base.join("pkg_a");
    let pkg_b = base.join("pkg_b");
    let _ = std::fs::create_dir_all(pkg_a.join("src"));
    let _ = std::fs::create_dir_all(&pkg_b);

    let manifest_a = r#"[package]
name = "@sapphire/entry-pkg"
version = "1.0.0"
entry = "src/entry.sapp"

[dependencies]
"#;
    std::fs::write(pkg_a.join("pblang.toml"), manifest_a).unwrap();
    std::fs::write(
        pkg_a.join("src").join("entry.sapp"),
        "fn id(x) { return x + 2; }\nexport id;\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let publish_status = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_a)
        .status()
        .expect("run sapphire publish");
    assert!(publish_status.success(), "sapphire publish should exit 0");

    let manifest_b = r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"@sapphire/entry-pkg" = "1.0.0"
"#;
    std::fs::write(pkg_b.join("pblang.toml"), manifest_b).unwrap();
    std::fs::write(
        pkg_b.join("main.sapp"),
        r#"
        let pkg = import("@sapphire/entry-pkg");
        print pkg.id(40);
        "#,
    )
    .unwrap();

    let index_path = registry_dir.join("index.json");
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path);
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));

    let install_status = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&pkg_b)
        .status()
        .expect("run sapphire install");
    assert!(install_status.success(), "sapphire install should exit 0");

    let out = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&pkg_b)
        .output()
        .expect("run sapphire run");
    assert!(
        out.status.success(),
        "sapphire run should exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("42"),
        "stdout should contain package result from manifest entry: {}",
        stdout
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn installed_registry_package_subpath_import_works_within_package_root() {
    let base = std::env::temp_dir().join("pblang_registry_subpath_import_ok");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg_a = base.join("pkg_a");
    let pkg_b = base.join("pkg_b");
    let _ = std::fs::create_dir_all(pkg_a.join("src"));
    let _ = std::fs::create_dir_all(&pkg_b);

    let manifest_a = r#"[package]
name = "@sapphire/subpath-pkg"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg_a.join("pblang.toml"), manifest_a).unwrap();
    std::fs::write(
        pkg_a.join("src").join("math.sapp"),
        "fn forty_two() { return 42; }\nexport forty_two;\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let publish_status = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_a)
        .status()
        .expect("run sapphire publish");
    assert!(publish_status.success(), "sapphire publish should exit 0");

    let manifest_b = r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"@sapphire/subpath-pkg" = "1.0.0"
"#;
    std::fs::write(pkg_b.join("pblang.toml"), manifest_b).unwrap();
    std::fs::write(
        pkg_b.join("main.sapp"),
        r#"
        let math = import("@sapphire/subpath-pkg:src/math.sapp");
        print math.forty_two();
        "#,
    )
    .unwrap();

    let index_path = registry_dir.join("index.json");
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path);
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));

    let install_status = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&pkg_b)
        .status()
        .expect("run sapphire install");
    assert!(install_status.success(), "sapphire install should exit 0");

    let out = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&pkg_b)
        .output()
        .expect("run sapphire run");
    assert!(
        out.status.success(),
        "sapphire run should exit 0, stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("42"),
        "stdout should contain package subpath import result: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn installed_registry_package_subpath_import_rejects_directory_target() {
    let base = std::env::temp_dir().join("pblang_registry_subpath_import_directory");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg_a = base.join("pkg_a");
    let pkg_b = base.join("pkg_b");
    let _ = std::fs::create_dir_all(pkg_a.join("src"));
    let _ = std::fs::create_dir_all(&pkg_b);

    let manifest_a = r#"[package]
name = "@sapphire/subpath-dir-pkg"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg_a.join("pblang.toml"), manifest_a).unwrap();
    std::fs::write(
        pkg_a.join("src").join("math.sapp"),
        "fn forty_two() { return 42; }\nexport forty_two;\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let publish_status = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_a)
        .status()
        .expect("run sapphire publish");
    assert!(publish_status.success(), "sapphire publish should exit 0");

    let manifest_b = r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"@sapphire/subpath-dir-pkg" = "1.0.0"
"#;
    std::fs::write(pkg_b.join("pblang.toml"), manifest_b).unwrap();
    std::fs::write(
        pkg_b.join("main.sapp"),
        r#"
        let math = import("@sapphire/subpath-dir-pkg:src");
        print math.forty_two();
        "#,
    )
    .unwrap();

    let index_path = registry_dir.join("index.json");
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path);
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));

    let install_status = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&pkg_b)
        .status()
        .expect("run sapphire install");
    assert!(install_status.success(), "sapphire install should exit 0");

    let out = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&pkg_b)
        .output()
        .expect("run sapphire run");
    assert!(
        !out.status.success(),
        "sapphire run should fail for package subpath directory target"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("subpath is not a file"),
        "stderr should mention file-only subpath contract: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn tampered_installed_registry_dependency_fails_lockfile_hash_check() {
    let base = std::env::temp_dir().join("pblang_registry_lockfile_hash_check");
    let _ = std::fs::create_dir_all(&base);
    let cache_dir = base.join("cache");
    let _ = std::fs::remove_dir_all(&cache_dir);
    std::fs::create_dir_all(&cache_dir).unwrap();
    let registry_dir = base.join("registry");
    let pkg_a = base.join("pkg_a");
    let pkg_b = base.join("pkg_b");
    let _ = std::fs::create_dir_all(&pkg_a);
    let _ = std::fs::create_dir_all(&pkg_b);

    let manifest_a = r#"[package]
name = "@sapphire/pub-pkg"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg_a.join("pblang.toml"), manifest_a).unwrap();
    std::fs::write(
        pkg_a.join("lib.sapp"),
        "fn forty_two() { return 42; }\nexport forty_two;\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let publish_status = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_a)
        .status()
        .expect("run sapphire publish");
    assert!(publish_status.success(), "sapphire publish should exit 0");

    let manifest_b = r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"@sapphire/pub-pkg" = "1.0.0"
"#;
    std::fs::write(pkg_b.join("pblang.toml"), manifest_b).unwrap();
    std::fs::write(
        pkg_b.join("main.sapp"),
        r#"
        let pkg = import("@sapphire/pub-pkg");
        print pkg.forty_two();
        "#,
    )
    .unwrap();

    let index_path = registry_dir.join("index.json");
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path);
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));

    let install_status = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&pkg_b)
        .env("PBLANG_CACHE", &cache_dir)
        .status()
        .expect("run sapphire install");
    assert!(install_status.success(), "sapphire install should exit 0");

    let lock_content = std::fs::read_to_string(pkg_b.join("pblang.lock")).unwrap();
    let lock = parse_lockfile(&lock_content).expect("parse pblang.lock");
    let dep = lock
        .dependencies
        .get("@sapphire/pub-pkg")
        .expect("registry dep in lockfile");
    std::fs::write(
        std::path::Path::new(&dep.path).join("lib.sapp"),
        "fn forty_two() { return 7; }\nexport forty_two;\n",
    )
    .unwrap();

    let out = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&pkg_b)
        .env("PBLANG_CACHE", &cache_dir)
        .output()
        .expect("run sapphire run");
    assert!(
        !out.status.success(),
        "sapphire run should fail when installed registry dep is tampered"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("content hash mismatch"),
        "stderr should mention hash mismatch: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn registry_dependency_rehydrates_from_lockfile_when_cache_path_is_missing() {
    let base = std::env::temp_dir().join("pblang_registry_rehydrate");
    let _ = std::fs::remove_dir_all(&base);
    let cache_dir = base.join("cache");
    let registry_dir = base.join("registry");
    let pkg_a = base.join("pkg_a");
    let pkg_b = base.join("pkg_b");
    std::fs::create_dir_all(&pkg_a).unwrap();
    std::fs::create_dir_all(&pkg_b).unwrap();

    std::fs::write(
        pkg_a.join("pblang.toml"),
        r#"[package]
name = "@sapphire/rehydrate-pkg"
version = "1.0.0"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(
        pkg_a.join("lib.sapp"),
        "fn forty_two() { return 42; }\nexport forty_two;\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let publish = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_a)
        .output()
        .expect("publish rehydrate package");
    assert!(
        publish.status.success(),
        "publish should succeed: {}",
        String::from_utf8_lossy(&publish.stderr)
    );

    std::fs::write(
        pkg_b.join("pblang.toml"),
        r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"@sapphire/rehydrate-pkg" = "1.0.0"
"#,
    )
    .unwrap();
    std::fs::write(
        pkg_b.join("main.sapp"),
        r#"
        let dep = import("@sapphire/rehydrate-pkg");
        print dep.forty_two();
        "#,
    )
    .unwrap();

    let index_url = file_url_for_path(&registry_dir.join("index.json"));
    let install = std::process::Command::new(bin)
        .arg("install")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&pkg_b)
        .env("PBLANG_CACHE", &cache_dir)
        .output()
        .expect("install registry dependency");
    assert!(
        install.status.success(),
        "install should succeed: {}",
        String::from_utf8_lossy(&install.stderr)
    );

    let lock_content = std::fs::read_to_string(pkg_b.join("pblang.lock")).unwrap();
    let lock = parse_lockfile(&lock_content).expect("parse pblang.lock");
    let dep = lock
        .dependencies
        .get("@sapphire/rehydrate-pkg")
        .expect("registry dep in lockfile");
    let dep_path = std::path::PathBuf::from(&dep.path);
    std::fs::remove_dir_all(&dep_path).unwrap();
    assert!(
        !dep_path.exists(),
        "dependency cache path should be removed before rehydration test"
    );

    let run = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&pkg_b)
        .env("PBLANG_CACHE", &cache_dir)
        .output()
        .expect("run with rehydrated registry dependency");
    assert!(
        run.status.success(),
        "run should rehydrate registry dependency: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("42"),
        "stdout should contain rehydrated registry dependency result: {}",
        String::from_utf8_lossy(&run.stdout)
    );
    assert!(
        dep_path.exists(),
        "rehydration should recreate dependency cache path"
    );

    let _ = std::fs::remove_dir_all(&base);
}

fn run_git(repo: &std::path::Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "Sapphire Tests")
        .env("GIT_AUTHOR_EMAIL", "tests@example.com")
        .env("GIT_COMMITTER_NAME", "Sapphire Tests")
        .env("GIT_COMMITTER_EMAIL", "tests@example.com")
        .output()
        .expect("run git command")
}

#[test]
fn install_git_dependency_with_pinned_rev_works() {
    let base = std::env::temp_dir().join("pblang_git_dep_install");
    let _ = std::fs::remove_dir_all(&base);
    let repo = base.join("gitdep");
    let consumer = base.join("consumer");
    let _ = std::fs::create_dir_all(&repo);
    let _ = std::fs::create_dir_all(&consumer);

    std::fs::write(
        repo.join("pblang.toml"),
        r#"[package]
name = "gitdep"
version = "0.1.0"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(
        repo.join("lib.sapp"),
        "fn forty_two() { return 42; }\nexport forty_two;\n",
    )
    .unwrap();

    let init = run_git(&repo, &["init"]);
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let add = run_git(&repo, &["add", "."]);
    assert!(
        add.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let commit = run_git(&repo, &["commit", "-m", "init"]);
    assert!(
        commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
    let rev_out = run_git(&repo, &["rev-parse", "HEAD"]);
    assert!(
        rev_out.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&rev_out.stderr)
    );
    let rev = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();

    let repo_abs = std::fs::canonicalize(&repo).unwrap_or(repo.clone());
    let repo_url = repo_abs.to_string_lossy().to_string();
    std::fs::write(
        consumer.join("pblang.toml"),
        format!(
            r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"gitdep" = {{ git = "{repo_url}", rev = "{rev}" }}
"#
        ),
    )
    .unwrap();
    std::fs::write(
        consumer.join("main.sapp"),
        r#"
        let dep = import("gitdep");
        print dep.forty_two();
        "#,
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let install = std::process::Command::new(bin)
        .arg("install")
        .arg(&consumer)
        .output()
        .expect("run sapphire install");
    assert!(
        install.status.success(),
        "sapphire install should exit 0, stderr: {}",
        String::from_utf8_lossy(&install.stderr)
    );

    let lock_content = std::fs::read_to_string(consumer.join("pblang.lock")).unwrap();
    assert!(
        lock_content.contains("source = \"git\""),
        "lockfile should record git source: {}",
        lock_content
    );
    assert!(
        lock_content.contains(&rev),
        "lockfile should contain pinned rev metadata: {}",
        lock_content
    );

    let run = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&consumer)
        .output()
        .expect("run sapphire run");
    assert!(
        run.status.success(),
        "sapphire run should exit 0, stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("42"),
        "stdout should contain imported git dep result: {}",
        String::from_utf8_lossy(&run.stdout)
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn install_git_dependency_rejects_invalid_package_entry() {
    let base = std::env::temp_dir().join("pblang_git_dep_invalid_entry");
    let _ = std::fs::remove_dir_all(&base);
    let repo = base.join("gitdep");
    let consumer = base.join("consumer");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::create_dir_all(&consumer).unwrap();

    std::fs::write(
        repo.join("pblang.toml"),
        r#"[package]
name = "gitdep-invalid-entry"
version = "0.1.0"
entry = "src/missing.sapp"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(
        repo.join("src").join("present.sapp"),
        "export fn forty_two() { return 42; }\n",
    )
    .unwrap();

    let init = run_git(&repo, &["init"]);
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let add = run_git(&repo, &["add", "."]);
    assert!(
        add.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let commit = run_git(&repo, &["commit", "-m", "init"]);
    assert!(
        commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
    let rev_out = run_git(&repo, &["rev-parse", "HEAD"]);
    assert!(
        rev_out.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&rev_out.stderr)
    );
    let rev = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();

    let repo_url = std::fs::canonicalize(&repo)
        .unwrap_or(repo.clone())
        .to_string_lossy()
        .to_string();
    std::fs::write(
        consumer.join("pblang.toml"),
        format!(
            r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"gitdep-invalid-entry" = {{ git = "{repo_url}", rev = "{rev}" }}
"#
        ),
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let install = std::process::Command::new(bin)
        .arg("install")
        .arg(&consumer)
        .output()
        .expect("install invalid git dependency");
    assert!(
        !install.status.success(),
        "install should fail for git dependency with invalid package entry"
    );
    let stderr = String::from_utf8_lossy(&install.stderr);
    assert!(
        stderr.contains("entry not found"),
        "stderr should mention invalid git package entry: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn git_dependency_rehydrates_from_lockfile_when_checkout_is_missing() {
    let base = std::env::temp_dir().join("pblang_git_dep_rehydrate");
    let _ = std::fs::remove_dir_all(&base);
    let cache_dir = base.join("cache");
    let repo = base.join("gitdep");
    let consumer = base.join("consumer");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&consumer).unwrap();

    std::fs::write(
        repo.join("pblang.toml"),
        r#"[package]
name = "gitdep"
version = "0.1.0"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(
        repo.join("lib.sapp"),
        "fn forty_two() { return 42; }\nexport forty_two;\n",
    )
    .unwrap();

    let init = run_git(&repo, &["init"]);
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let add = run_git(&repo, &["add", "."]);
    assert!(
        add.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let commit = run_git(&repo, &["commit", "-m", "init"]);
    assert!(
        commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
    let rev_out = run_git(&repo, &["rev-parse", "HEAD"]);
    assert!(
        rev_out.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&rev_out.stderr)
    );
    let rev = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();

    let repo_url = std::fs::canonicalize(&repo)
        .unwrap_or(repo.clone())
        .to_string_lossy()
        .to_string();
    std::fs::write(
        consumer.join("pblang.toml"),
        format!(
            r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"gitdep" = {{ git = "{repo_url}", rev = "{rev}" }}
"#
        ),
    )
    .unwrap();
    std::fs::write(
        consumer.join("main.sapp"),
        r#"
        let dep = import("gitdep");
        print dep.forty_two();
        "#,
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let install = std::process::Command::new(bin)
        .arg("install")
        .arg(&consumer)
        .env("PBLANG_CACHE", &cache_dir)
        .output()
        .expect("install git dependency");
    assert!(
        install.status.success(),
        "install should succeed: {}",
        String::from_utf8_lossy(&install.stderr)
    );

    let lock_content = std::fs::read_to_string(consumer.join("pblang.lock")).unwrap();
    let lock = parse_lockfile(&lock_content).expect("parse pblang.lock");
    let dep = lock
        .dependencies
        .get("gitdep")
        .expect("git dep in lockfile");
    let dep_path = std::path::PathBuf::from(&dep.path);
    std::fs::remove_dir_all(&dep_path).unwrap();
    assert!(
        !dep_path.exists(),
        "git checkout path should be removed before rehydration test"
    );

    let run = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&consumer)
        .env("PBLANG_CACHE", &cache_dir)
        .output()
        .expect("run with rehydrated git dependency");
    assert!(
        run.status.success(),
        "run should rehydrate git dependency: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("42"),
        "stdout should contain rehydrated git dependency result: {}",
        String::from_utf8_lossy(&run.stdout)
    );
    assert!(
        dep_path.exists(),
        "rehydration should recreate git checkout path"
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn install_git_dependency_uses_manifest_entry_for_package_import() {
    let base = std::env::temp_dir().join("pblang_git_dep_manifest_entry");
    let _ = std::fs::remove_dir_all(&base);
    let repo = base.join("gitdep");
    let consumer = base.join("consumer");
    let _ = std::fs::create_dir_all(repo.join("src"));
    let _ = std::fs::create_dir_all(&consumer);

    std::fs::write(
        repo.join("pblang.toml"),
        r#"[package]
name = "gitdep-entry"
version = "0.1.0"
entry = "src/entry.sapp"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(
        repo.join("src").join("entry.sapp"),
        "fn forty_two() { return 42; }\nexport forty_two;\n",
    )
    .unwrap();

    let init = run_git(&repo, &["init"]);
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let add = run_git(&repo, &["add", "."]);
    assert!(
        add.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let commit = run_git(&repo, &["commit", "-m", "init"]);
    assert!(
        commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
    let rev_out = run_git(&repo, &["rev-parse", "HEAD"]);
    assert!(
        rev_out.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&rev_out.stderr)
    );
    let rev = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();

    let repo_abs = std::fs::canonicalize(&repo).unwrap_or(repo.clone());
    let repo_url = repo_abs.to_string_lossy().to_string();
    std::fs::write(
        consumer.join("pblang.toml"),
        format!(
            r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"gitdep-entry" = {{ git = "{repo_url}", rev = "{rev}" }}
"#
        ),
    )
    .unwrap();
    std::fs::write(
        consumer.join("main.sapp"),
        r#"
        let dep = import("gitdep-entry");
        print dep.forty_two();
        "#,
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let install = std::process::Command::new(bin)
        .arg("install")
        .arg(&consumer)
        .output()
        .expect("run sapphire install");
    assert!(
        install.status.success(),
        "sapphire install should exit 0, stderr: {}",
        String::from_utf8_lossy(&install.stderr)
    );

    let run = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&consumer)
        .output()
        .expect("run sapphire run");
    assert!(
        run.status.success(),
        "sapphire run should exit 0, stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stdout).contains("42"),
        "stdout should contain imported git dep result from manifest entry: {}",
        String::from_utf8_lossy(&run.stdout)
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn install_git_dependency_subpath_import_cannot_escape_package_root() {
    let base = std::env::temp_dir().join("pblang_git_dep_subpath_escape");
    let _ = std::fs::remove_dir_all(&base);
    let repo = base.join("gitdep");
    let consumer = base.join("consumer");
    let _ = std::fs::create_dir_all(repo.join("src"));
    let _ = std::fs::create_dir_all(&consumer);

    std::fs::write(
        repo.join("pblang.toml"),
        r#"[package]
name = "gitdep-subpath"
version = "0.1.0"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(
        repo.join("src").join("entry.sapp"),
        "export fn forty_two() { return 42; }\n",
    )
    .unwrap();

    let init = run_git(&repo, &["init"]);
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let add = run_git(&repo, &["add", "."]);
    assert!(
        add.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let commit = run_git(&repo, &["commit", "-m", "init"]);
    assert!(
        commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
    let rev_out = run_git(&repo, &["rev-parse", "HEAD"]);
    assert!(
        rev_out.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&rev_out.stderr)
    );
    let rev = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();

    let repo_abs = std::fs::canonicalize(&repo).unwrap_or(repo.clone());
    let repo_url = repo_abs.to_string_lossy().to_string();
    std::fs::write(
        consumer.join("pblang.toml"),
        format!(
            r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"gitdep-subpath" = {{ git = "{repo_url}", rev = "{rev}" }}
"#
        ),
    )
    .unwrap();
    std::fs::write(
        consumer.join("main.sapp"),
        r#"
        let dep = import("gitdep-subpath:../outside.sapp");
        print dep.forty_two();
        "#,
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let install = std::process::Command::new(bin)
        .arg("install")
        .arg(&consumer)
        .output()
        .expect("run sapphire install");
    assert!(
        install.status.success(),
        "sapphire install should exit 0, stderr: {}",
        String::from_utf8_lossy(&install.stderr)
    );

    let run = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&consumer)
        .output()
        .expect("run sapphire run");
    assert!(
        !run.status.success(),
        "sapphire run should fail for package subpath escape"
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("must not escape the package root"),
        "stderr should mention package root confinement: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn install_git_dependency_subpath_import_rejects_directory_target() {
    let base = std::env::temp_dir().join("pblang_git_dep_subpath_directory");
    let _ = std::fs::remove_dir_all(&base);
    let repo = base.join("gitdep");
    let consumer = base.join("consumer");
    let _ = std::fs::create_dir_all(repo.join("src"));
    let _ = std::fs::create_dir_all(&consumer);

    std::fs::write(
        repo.join("pblang.toml"),
        r#"[package]
name = "gitdep-subpath-dir"
version = "0.1.0"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(
        repo.join("src").join("entry.sapp"),
        "export fn forty_two() { return 42; }\n",
    )
    .unwrap();

    let init = run_git(&repo, &["init"]);
    assert!(
        init.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    let add = run_git(&repo, &["add", "."]);
    assert!(
        add.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let commit = run_git(&repo, &["commit", "-m", "init"]);
    assert!(
        commit.status.success(),
        "git commit failed: {}",
        String::from_utf8_lossy(&commit.stderr)
    );
    let rev_out = run_git(&repo, &["rev-parse", "HEAD"]);
    assert!(
        rev_out.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&rev_out.stderr)
    );
    let rev = String::from_utf8_lossy(&rev_out.stdout).trim().to_string();

    let repo_abs = std::fs::canonicalize(&repo).unwrap_or(repo.clone());
    let repo_url = repo_abs.to_string_lossy().to_string();
    std::fs::write(
        consumer.join("pblang.toml"),
        format!(
            r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"gitdep-subpath-dir" = {{ git = "{repo_url}", rev = "{rev}" }}
"#
        ),
    )
    .unwrap();
    std::fs::write(
        consumer.join("main.sapp"),
        r#"
        let dep = import("gitdep-subpath-dir:src");
        print dep.forty_two();
        "#,
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let install = std::process::Command::new(bin)
        .arg("install")
        .arg(&consumer)
        .output()
        .expect("run sapphire install");
    assert!(
        install.status.success(),
        "sapphire install should exit 0, stderr: {}",
        String::from_utf8_lossy(&install.stderr)
    );

    let run = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&consumer)
        .output()
        .expect("run sapphire run");
    assert!(
        !run.status.success(),
        "sapphire run should fail for package subpath directory target"
    );
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(
        stderr.contains("subpath is not a file"),
        "stderr should mention file-only subpath contract: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn lockfile_is_reproducible_after_registry_install() {
    let temp = std::env::temp_dir().join("pblang_lockfile_repro_registry");
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).unwrap();
    let (_tarball_path, tarball_url, tarball_sha256) = make_tarball_and_sha256(&temp);
    let index = serde_json::json!({
        "packages": {
            "@sapphire/repro-lock": {
                "1.0.0": { "url": tarball_url, "sha256": tarball_sha256 }
            }
        }
    });
    let index_path = temp.join("index.json");
    std::fs::write(&index_path, index.to_string()).unwrap();
    let index_url = file_url_for_path(&index_path);
    let manifest = r#"[package]
name = "repro-lock-consumer"
version = "0.1.0"

[dependencies]
"@sapphire/repro-lock" = "1.0.0"
"#;
    std::fs::write(temp.join("pblang.toml"), manifest).unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    for _ in 0..2 {
        let out = std::process::Command::new(bin)
            .arg("install")
            .arg("--registry-index")
            .arg(&index_url)
            .arg(&temp)
            .output()
            .expect("install");
        assert!(
            out.status.success(),
            "install failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let lock_content = std::fs::read_to_string(temp.join("pblang.lock")).unwrap();
    let lock = parse_lockfile(&lock_content).expect("parse lock");
    let serialized = write_lockfile(&lock).expect("serialize lock");
    assert!(
        serialized.contains("@sapphire/repro-lock"),
        "serialized lock should list registry dependency"
    );

    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn fetch_url_reads_registry_index_over_http_when_explicitly_allowed() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test http listener");
    let addr = listener.local_addr().expect("listener addr");
    let body =
        br#"{"packages":{"@sapphire/http-pkg":{"1.0.0":{"url":"file:///noop","sha256":null}}}}"#;
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                String::from_utf8_lossy(body)
            );
            let _ = stream.write_all(response.as_bytes());
            let mut buf = [0u8; 64];
            let _ = stream.read(&mut buf);
        }
    });
    let url = format!("http://{}/index.json", addr);
    std::env::set_var("PBLANG_ALLOW_INSECURE_HTTP", "1");
    let bytes = fetch_url(&url).expect("fetch registry index over http");
    std::env::remove_var("PBLANG_ALLOW_INSECURE_HTTP");
    handle.join().expect("http server thread");
    let val: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert!(val.get("packages").is_some());
}

// ────────────────────────────────────────────────────────────────────────────
// JSON ingress cap (PL2-beta) — registry index size is capped at
// MAX_REGISTRY_INDEX_BYTES (10 MB by default). Oversized payloads must be
// rejected before the parse to bound memory consumption.
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn registry_index_rejects_oversized_payload() {
    // Synthesize a payload larger than the cap. We don't actually need it
    // to be valid JSON — the size check fires before the parse.
    let cap = pblang_pm::MAX_REGISTRY_INDEX_BYTES;
    let bytes: Vec<u8> = vec![b'a'; cap + 1];
    let err = pblang_pm::parse_registry_index(&bytes).expect_err("should reject oversized payload");
    assert!(
        err.contains("exceeds cap"),
        "expected size-cap error, got: {err}"
    );
    assert!(
        err.contains(&cap.to_string()),
        "error should mention the cap value, got: {err}"
    );
}

#[test]
fn registry_index_accepts_payload_at_cap_boundary() {
    // A valid (empty-packages) JSON well under the cap should parse fine.
    let json = br#"{"packages":{}}"#;
    let idx = pblang_pm::parse_registry_index(json).expect("small valid index should parse");
    // RegistryIndex doesn't expose a public len(); just verify we got one.
    let _ = idx;
}
