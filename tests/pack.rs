use std::io::Read;

fn registry_file_url(path: &std::path::Path) -> String {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    format!("file:///{}", abs.to_string_lossy().replace('\\', "/"))
}

fn make_temp_project() -> std::path::PathBuf {
    let base = std::env::temp_dir().join("pblang_pack_test");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let manifest = r#"[package]
name = "pack-test"
version = "0.1.0"

[dependencies]
"#;
    std::fs::write(base.join("pblang.toml"), manifest).unwrap();
    std::fs::write(base.join("main.sapp"), "print 42;").unwrap();
    base
}

fn make_temp_project_with_manifest_entry(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(base.join("src")).unwrap();
    let manifest = r#"[package]
name = "pack-entry-test"
version = "0.1.0"
entry = "src/app.sapp"

[dependencies]
"#;
    std::fs::write(base.join("pblang.toml"), manifest).unwrap();
    std::fs::write(base.join("src").join("app.sapp"), "print 42;").unwrap();
    base
}

fn make_temp_project_with_external_path_dep(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(name);
    let shared = std::env::temp_dir().join(format!("{name}_shared"));
    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&shared);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::create_dir_all(&shared).unwrap();
    let manifest = r#"[package]
name = "pack-path-dep-test"
version = "0.1.0"

[dependencies]
"shared" = { path = "../"#;
    let manifest = format!(
        "{manifest}{}\" }}\n",
        shared.file_name().unwrap().to_string_lossy()
    );
    std::fs::write(base.join("pblang.toml"), manifest).unwrap();
    std::fs::write(base.join("main.sapp"), "print 42;").unwrap();
    std::fs::write(
        shared.join("pblang.toml"),
        r#"[package]
name = "shared"
version = "0.1.0"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(
        shared.join("lib.sapp"),
        "export fn helper() { return 1; }\n",
    )
    .unwrap();
    base
}

fn make_temp_project_with_missing_path_dep_named(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let manifest = r#"[package]
name = "pack-missing-dep-test"
version = "0.1.0"

[dependencies]
"local" = { path = "deps/missing" }
"#;
    std::fs::write(base.join("pblang.toml"), manifest).unwrap();
    std::fs::write(base.join("main.sapp"), "print 42;").unwrap();
    base
}

fn make_temp_project_with_missing_path_dep() -> std::path::PathBuf {
    make_temp_project_with_missing_path_dep_named("pblang_pack_missing_path_dep")
}

#[test]
fn pack_fails_when_path_dependency_missing() {
    let proj = make_temp_project_with_missing_path_dep();
    let out_path = proj.join("app.tar.gz");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("pack")
        .arg(&proj)
        .arg(&out_path)
        .output()
        .expect("run pblang pack with missing path dep");
    assert!(
        !out.status.success(),
        "pack should fail when path dependency directory does not exist"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("path not found") || stderr.contains("not found"),
        "stderr should mention path/dependency not found: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn bundle_fails_when_path_dependency_missing() {
    let proj = make_temp_project_with_missing_path_dep_named("pblang_bundle_missing_path_dep");
    let bundle_dir = proj.join("bundle-out");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("bundle")
        .arg(".")
        .arg(&bundle_dir)
        .current_dir(&proj)
        .output()
        .expect("run pblang bundle with missing path dep");
    assert!(
        !out.status.success(),
        "bundle should fail when path dependency directory does not exist"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("path not found") || stderr.contains("not found"),
        "stderr should mention path/dependency not found: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&proj);
}

fn make_temp_project_with_manifest(name: &str, manifest_body: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(name);
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(base.join("pblang.toml"), manifest_body).unwrap();
    std::fs::write(base.join("main.sapp"), "print 42;").unwrap();
    base
}

#[test]
fn pack_succeeds_with_valid_target_platform_and_min_runtime() {
    let proj = make_temp_project_with_manifest(
        "pblang_pack_valid_meta",
        r#"[package]
name = "meta-test"
version = "0.1.0"
target_platform = "linux"
min_runtime = "0.3.0"

[dependencies]
"#,
    );
    let out_path = proj.join("out.tar.gz");
    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("pack")
        .arg(&proj)
        .arg(&out_path)
        .output()
        .expect("run pblang pack");
    assert!(
        out.status.success(),
        "pack with valid target_platform and min_runtime should succeed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out_path.is_file());
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn pack_fails_for_invalid_target_platform() {
    let proj = make_temp_project_with_manifest(
        "pblang_pack_invalid_platform",
        r#"[package]
name = "meta-test"
version = "0.1.0"
target_platform = "solaris"

[dependencies]
"#,
    );
    let out_path = proj.join("out.tar.gz");
    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("pack")
        .arg(&proj)
        .arg(&out_path)
        .output()
        .expect("run pblang pack");
    assert!(
        !out.status.success(),
        "pack should fail for invalid target_platform"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("target_platform")
            && (stderr.contains("unknown") || stderr.contains("valid values")),
        "stderr: {}",
        stderr
    );
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn pack_fails_for_invalid_min_runtime() {
    let proj = make_temp_project_with_manifest(
        "pblang_pack_invalid_min_runtime",
        r#"[package]
name = "meta-test"
version = "0.1.0"
min_runtime = "not-a-version"

[dependencies]
"#,
    );
    let out_path = proj.join("out.tar.gz");
    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("pack")
        .arg(&proj)
        .arg(&out_path)
        .output()
        .expect("run pblang pack");
    assert!(
        !out.status.success(),
        "pack should fail for invalid min_runtime"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("min_runtime") && (stderr.contains("valid") || stderr.contains("MAJOR")),
        "stderr: {}",
        stderr
    );
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn pack_then_run_works() {
    let proj = make_temp_project();
    let out_path = proj.join("app.tar.gz");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let status = std::process::Command::new(bin)
        .arg("pack")
        .arg(&proj)
        .arg(&out_path)
        .status()
        .expect("run pblang pack");
    assert!(status.success(), "pblang pack should exit 0");
    assert!(out_path.is_file(), "pack output should exist");

    // Unpack the tar.gz into a fresh directory.
    let unpack_dir = std::env::temp_dir().join("pblang_pack_unpacked");
    let _ = std::fs::remove_dir_all(&unpack_dir);
    std::fs::create_dir_all(&unpack_dir).unwrap();

    let mut file = std::fs::File::open(&out_path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    let dec = flate2::read::GzDecoder::new(&buf[..]);
    let mut archive = tar::Archive::new(dec);
    archive.unpack(&unpack_dir).unwrap();

    // Run the unpacked project with the installed pblang binary.
    let output = std::process::Command::new(bin)
        .arg("run")
        .arg("main.sapp")
        .current_dir(&unpack_dir)
        .output()
        .expect("run packed app");
    assert!(
        output.status.success(),
        "running packed app should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("42"),
        "packed app should print 42, got stdout={}",
        stdout
    );

    let _ = std::fs::remove_dir_all(&proj);
    let _ = std::fs::remove_dir_all(&unpack_dir);
}

#[test]
fn verify_succeeds_on_valid_pack_tarball() {
    let base = std::env::temp_dir().join("pblang_verify_ok");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(
        base.join("pblang.toml"),
        "[package]\nname = \"verify-test\"\nversion = \"0.1.0\"\n[dependencies]\n",
    )
    .unwrap();
    std::fs::write(base.join("main.sapp"), "print 1;").unwrap();
    let out_path = base.join("out.tar.gz");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let status = std::process::Command::new(bin)
        .arg("pack")
        .arg(&base)
        .arg(&out_path)
        .status()
        .expect("run pblang pack");
    assert!(status.success(), "pack should succeed");
    assert!(out_path.is_file(), "tarball should exist");

    let out = std::process::Command::new(bin)
        .arg("verify")
        .arg(&out_path)
        .output()
        .expect("run pblang verify");
    assert!(
        out.status.success(),
        "verify should succeed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn verify_fails_when_tarball_missing_manifest() {
    let bin = env!("CARGO_BIN_EXE_pblang");
    let bad = std::env::temp_dir().join("pblang_verify_bad_nomanifest.tar.gz");
    let _ = std::fs::remove_file(&bad);
    let mut buf = Vec::new();
    {
        let mut enc = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
        let mut tar_builder = tar::Builder::new(&mut enc);
        let mut header = tar::Header::new_gnu();
        header.set_path("main.sapp").unwrap();
        header.set_size(10);
        header.set_cksum();
        tar_builder.append(&header, &b"print 1;"[..]).unwrap();
        tar_builder.finish().unwrap();
    }
    std::fs::write(&bad, &buf).unwrap();

    let out = std::process::Command::new(bin)
        .arg("verify")
        .arg(&bad)
        .output()
        .expect("run pblang verify");
    assert!(
        !out.status.success(),
        "verify should fail when pblang.toml missing"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("pblang.toml") || stderr.contains("missing"),
        "stderr: {}",
        stderr
    );
    let _ = std::fs::remove_file(&bad);
}

#[test]
fn pack_tarball_contains_manifest_and_entry_file() {
    let base = std::env::temp_dir().join("pblang_pack_tarball_content");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(
        base.join("pblang.toml"),
        "[package]\nname = \"tarball-content-test\"\nversion = \"0.1.0\"\n[dependencies]\n",
    )
    .unwrap();
    std::fs::write(base.join("main.sapp"), "print 42;").unwrap();
    let proj = base;
    let out_path = proj.join("app.tar.gz");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let status = std::process::Command::new(bin)
        .arg("pack")
        .arg(&proj)
        .arg(&out_path)
        .status()
        .expect("run pblang pack");
    assert!(status.success(), "pblang pack should exit 0");
    assert!(out_path.is_file(), "pack output should exist");

    let mut file = std::fs::File::open(&out_path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    let dec = flate2::read::GzDecoder::new(&buf[..]);
    let mut archive = tar::Archive::new(dec);
    let entries: Vec<String> = archive
        .entries()
        .expect("tarball entries")
        .filter_map(|e| {
            let entry = e.ok()?;
            let p = entry.path().ok()?;
            Some(p.to_string_lossy().replace('\\', "/"))
        })
        .collect();

    assert!(
        entries
            .iter()
            .any(|s| s.ends_with("pblang.toml") || s == "pblang.toml"),
        "tarball should contain pblang.toml, got: {:?}",
        entries
    );
    assert!(
        entries
            .iter()
            .any(|s| s.ends_with("main.sapp") || s == "main.sapp"),
        "tarball should contain main.sapp, got: {:?}",
        entries
    );

    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn pack_then_run_from_manifest_dir_uses_package_entry() {
    let proj = make_temp_project_with_manifest_entry("pblang_pack_test_manifest_entry");
    let out_path = proj.join("app.tar.gz");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let pack = std::process::Command::new(bin)
        .arg("pack")
        .arg(&proj)
        .arg(&out_path)
        .output()
        .expect("run pblang pack");
    assert!(
        pack.status.success(),
        "pblang pack should exit 0: stderr={}",
        String::from_utf8_lossy(&pack.stderr)
    );
    let pack_stdout = String::from_utf8_lossy(&pack.stdout);
    assert!(
        pack_stdout.contains("entry src/app.sapp"),
        "pack output should mention resolved entry: {}",
        pack_stdout
    );
    assert!(out_path.is_file(), "pack output should exist");

    let unpack_dir = std::env::temp_dir().join("pblang_pack_unpacked_manifest_entry");
    let _ = std::fs::remove_dir_all(&unpack_dir);
    std::fs::create_dir_all(&unpack_dir).unwrap();

    let mut file = std::fs::File::open(&out_path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    let dec = flate2::read::GzDecoder::new(&buf[..]);
    let mut archive = tar::Archive::new(dec);
    archive.unpack(&unpack_dir).unwrap();

    let output = std::process::Command::new(bin)
        .arg("run")
        .current_dir(&unpack_dir)
        .output()
        .expect("run packed app via manifest entry");
    assert!(
        output.status.success(),
        "running packed app should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("42"),
        "packed app should print 42 via manifest entry, got stdout={}",
        stdout
    );

    let _ = std::fs::remove_dir_all(&proj);
    let _ = std::fs::remove_dir_all(&unpack_dir);
}

#[test]
fn bundle_self_extract_flag_accepted() {
    // Verify that --self-extract is a recognized flag for `pblang bundle`
    // (no "unknown flag" style errors), even if no actual self-extracting
    // binary is produced yet.
    let proj = make_temp_project_with_manifest_entry("pblang_bundle_self_extract_flag");
    let bundle_dir = proj.join("bundle-out-self-extract");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("bundle")
        .arg("--self-extract")
        .arg(&proj)
        .arg(&bundle_dir)
        .output()
        .expect("run pblang bundle --self-extract");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("unknown flag") && !stderr.contains("unrecognized"),
        "bundle should recognize --self-extract flag, stderr={}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&proj);
    let _ = std::fs::remove_dir_all(&bundle_dir);
}

#[test]
fn bundle_then_run_from_manifest_dir_uses_package_entry() {
    let proj = make_temp_project_with_manifest_entry("pblang_bundle_test_manifest_entry");
    let bundle_dir = proj.join("bundle-out");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let bundle = std::process::Command::new(bin)
        .arg("bundle")
        .arg(&proj)
        .arg(&bundle_dir)
        .output()
        .expect("run pblang bundle");
    assert!(
        bundle.status.success(),
        "pblang bundle should exit 0: stderr={}",
        String::from_utf8_lossy(&bundle.stderr)
    );
    let bundle_stdout = String::from_utf8_lossy(&bundle.stdout);
    assert!(
        bundle_stdout.contains("entry src/app.sapp"),
        "bundle output should mention resolved entry: {}",
        bundle_stdout
    );

    let exe_name = if cfg!(target_os = "windows") {
        "pblang.exe"
    } else {
        "pblang"
    };
    let bundled_bin = bundle_dir.join(exe_name);
    assert!(bundled_bin.is_file(), "bundled executable should exist");
    assert!(
        bundle_dir.join("src").join("app.sapp").is_file(),
        "bundled entry should preserve relative path"
    );

    let output = std::process::Command::new(&bundled_bin)
        .arg("run")
        .arg(".")
        .current_dir(&bundle_dir)
        .output()
        .expect("run bundled app via manifest entry");
    assert!(
        output.status.success(),
        "running bundled app should succeed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("42"),
        "bundled app should print 42 via manifest entry, got stdout={}",
        stdout
    );

    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn bundle_dir_contains_manifest_and_entry_file() {
    let base = std::env::temp_dir().join("pblang_bundle_dir_content");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    std::fs::write(
        base.join("pblang.toml"),
        "[package]\nname = \"bundle-content-test\"\nversion = \"0.1.0\"\n[dependencies]\n",
    )
    .unwrap();
    std::fs::write(base.join("main.sapp"), "print 42;").unwrap();
    let proj = base.clone();
    let bundle_dir = proj.join("bundle-out");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let status = std::process::Command::new(bin)
        .arg("bundle")
        .arg(&proj)
        .arg(&bundle_dir)
        .status()
        .expect("run pblang bundle");
    assert!(status.success(), "pblang bundle should exit 0");
    assert!(bundle_dir.is_dir(), "bundle output directory should exist");
    assert!(
        bundle_dir.join("pblang.toml").is_file(),
        "bundle dir should contain pblang.toml"
    );
    assert!(
        bundle_dir.join("main.sapp").is_file(),
        "bundle dir should contain entry file main.sapp"
    );

    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn bundle_with_registry_index_generates_lockfile_for_registry_dep() {
    let base = std::env::temp_dir().join("pblang_pack_registry_index");
    let _ = std::fs::remove_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg_a = base.join("pkg_a");
    let consumer = base.join("consumer");
    std::fs::create_dir_all(&registry_dir).unwrap();
    std::fs::create_dir_all(&pkg_a).unwrap();
    std::fs::create_dir_all(&consumer).unwrap();

    let manifest_a = r#"[package]
name = "@pblang/bundle-dep"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg_a.join("pblang.toml"), manifest_a).unwrap();
    std::fs::write(
        pkg_a.join("lib.sapp"),
        "fn answer() { return 42; }\nexport answer;\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let publish = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_a)
        .output()
        .expect("run pblang pack");
    assert!(
        publish.status.success(),
        "pblang publish should exit 0: stderr={}",
        String::from_utf8_lossy(&publish.stderr)
    );

    let manifest_b = r#"[package]
name = "bundle-consumer"
version = "0.1.0"

[dependencies]
"@pblang/bundle-dep" = "1.0.0"
"#;
    std::fs::write(consumer.join("pblang.toml"), manifest_b).unwrap();
    std::fs::write(consumer.join("main.sapp"), "print 42;\n").unwrap();
    let bundle_dir = consumer.join("bundle-out");

    let bundle = std::process::Command::new(bin)
        .arg("bundle")
        .arg("--registry-index")
        .arg(registry_file_url(&registry_dir.join("index.json")))
        .arg(&consumer)
        .arg(&bundle_dir)
        .output()
        .expect("run pblang bundle --registry-index");
    assert!(
        bundle.status.success(),
        "pblang bundle --registry-index should exit 0: stderr={}",
        String::from_utf8_lossy(&bundle.stderr)
    );
    let lock_path = bundle_dir.join("pblang.lock");
    assert!(
        lock_path.is_file(),
        "bundle should copy regenerated pblang.lock"
    );
    let lock_content = std::fs::read_to_string(&lock_path).unwrap();
    assert!(
        lock_content.contains("@pblang/bundle-dep"),
        "bundled lockfile should contain registry dep: {}",
        lock_content
    );
    assert!(
        lock_content.contains("source = \"registry\""),
        "bundled lockfile should record registry source: {}",
        lock_content
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn pack_with_registry_index_generates_lockfile_for_registry_dep() {
    let base = std::env::temp_dir().join("pblang_bundle_registry_index");
    let _ = std::fs::remove_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg_a = base.join("pkg_a");
    let consumer = base.join("consumer");
    std::fs::create_dir_all(&registry_dir).unwrap();
    std::fs::create_dir_all(&pkg_a).unwrap();
    std::fs::create_dir_all(&consumer).unwrap();

    let manifest_a = r#"[package]
name = "@pblang/pack-dep"
version = "1.0.0"

[dependencies]
"#;
    std::fs::write(pkg_a.join("pblang.toml"), manifest_a).unwrap();
    std::fs::write(
        pkg_a.join("lib.sapp"),
        "fn answer() { return 42; }\nexport answer;\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let publish = std::process::Command::new(bin)
        .arg("publish")
        .arg("--registry-dir")
        .arg(&registry_dir)
        .arg(&pkg_a)
        .output()
        .expect("run pblang bundle");
    assert!(
        publish.status.success(),
        "pblang publish should exit 0: stderr={}",
        String::from_utf8_lossy(&publish.stderr)
    );

    let manifest_b = r#"[package]
name = "pack-consumer"
version = "0.1.0"

[dependencies]
"@pblang/pack-dep" = "1.0.0"
"#;
    std::fs::write(consumer.join("pblang.toml"), manifest_b).unwrap();
    std::fs::write(consumer.join("main.sapp"), "print 42;\n").unwrap();
    let index_url = registry_file_url(&registry_dir.join("index.json"));
    let out_path = consumer.join("dist").join("registry-pack.tar.gz");

    let pack = std::process::Command::new(bin)
        .arg("pack")
        .arg("--registry-index")
        .arg(&index_url)
        .arg(&consumer)
        .arg(&out_path)
        .output()
        .expect("run pblang pack --registry-index");
    assert!(
        pack.status.success(),
        "pblang pack --registry-index should exit 0: stderr={}",
        String::from_utf8_lossy(&pack.stderr)
    );
    assert!(out_path.is_file(), "pack output should exist");

    let lock_path = consumer.join("pblang.lock");
    assert!(lock_path.is_file(), "pack should write pblang.lock");
    let lock_content = std::fs::read_to_string(&lock_path).unwrap();
    assert!(
        lock_content.contains("@pblang/pack-dep"),
        "lockfile should contain registry dep after pack: {}",
        lock_content
    );
    assert!(
        lock_content.contains("source = \"registry\""),
        "lockfile should record registry source after pack: {}",
        lock_content
    );

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn pack_require_registry_sha256_flag_rejects_missing_checksum() {
    let base = std::env::temp_dir().join("pblang_pack_require_sha_flag");
    let _ = std::fs::remove_dir_all(&base);
    let registry_dir = base.join("registry");
    let consumer = base.join("consumer");
    std::fs::create_dir_all(&registry_dir).unwrap();
    std::fs::create_dir_all(&consumer).unwrap();

    let tarball_path = registry_dir.join("pkg-1.0.0.tar.gz");
    {
        let f = std::fs::File::create(&tarball_path).unwrap();
        let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut tar_builder = tar::Builder::new(&mut enc);
        let mut header = tar::Header::new_gnu();
        let bytes = b"export fn id() { return 1; }\n";
        header.set_path("lib.sapp").unwrap();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder.append(&header, bytes as &[u8]).unwrap();
        tar_builder.finish().unwrap();
        drop(tar_builder);
        enc.finish().unwrap();
    }

    let index = serde_json::json!({
        "packages": {
            "@pblang/strict-pack": {
                "1.0.0": { "url": "pkg-1.0.0.tar.gz", "sha256": null }
            }
        }
    });
    std::fs::write(registry_dir.join("index.json"), index.to_string()).unwrap();
    std::fs::write(
        consumer.join("pblang.toml"),
        r#"[package]
name = "pack-consumer"
version = "0.1.0"

[dependencies]
"@pblang/strict-pack" = "1.0.0"
"#,
    )
    .unwrap();
    std::fs::write(consumer.join("main.sapp"), "print 42;\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("pack")
        .arg("--registry-index")
        .arg(registry_file_url(&registry_dir.join("index.json")))
        .arg("--require-registry-sha256")
        .arg(&consumer)
        .arg(consumer.join("dist").join("strict.tar.gz"))
        .output()
        .expect("run pblang pack --require-registry-sha256");
    assert!(
        !out.status.success(),
        "pack should fail when strict sha flag is set and index sha256 is missing"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("sha256 required"),
        "stderr should mention strict checksum requirement: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&base);
}

fn make_temp_project_with_metadata(suffix: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("pblang_pack_metadata_{}", suffix));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let manifest = r#"[package]
name = "my-tool"
version = "1.2.3"
description = "A small CLI tool for testing"

[dependencies]
"#;
    std::fs::write(base.join("pblang.toml"), manifest).unwrap();
    std::fs::write(base.join("main.sapp"), "print 42;").unwrap();
    base
}

#[test]
fn pack_output_includes_package_name_version_and_description() {
    let proj = make_temp_project_with_metadata("pack");
    let out_path = std::env::temp_dir().join("pblang_pack_metadata_out.tar.gz");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("pack")
        .arg(&proj)
        .arg(&out_path)
        .output()
        .expect("run pblang pack");
    assert!(
        out.status.success(),
        "pblang pack should exit 0: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("my-tool@1.2.3"),
        "pack stdout should include name@version: {}",
        stdout
    );
    assert!(
        stdout.contains("A small CLI tool for testing"),
        "pack stdout should include description: {}",
        stdout
    );
    assert!(
        stdout.contains("To run:"),
        "pack stdout should include run hint: {}",
        stdout
    );
    assert!(out_path.is_file(), "pack output file should exist");

    let _ = std::fs::remove_dir_all(&proj);
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn bundle_output_includes_package_name_version_and_description() {
    let proj = make_temp_project_with_metadata("bundle");
    let bundle_dir = std::env::temp_dir().join("pblang_bundle_metadata_out");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("bundle")
        .arg(&proj)
        .arg(&bundle_dir)
        .output()
        .expect("run pblang bundle");
    assert!(
        out.status.success(),
        "pblang bundle should exit 0: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("my-tool@1.2.3"),
        "bundle stdout should include name@version: {}",
        stdout
    );
    assert!(
        stdout.contains("A small CLI tool for testing"),
        "bundle stdout should include description: {}",
        stdout
    );
    assert!(
        stdout.contains("To run:"),
        "bundle stdout should include run hint: {}",
        stdout
    );

    let _ = std::fs::remove_dir_all(&proj);
    let _ = std::fs::remove_dir_all(&bundle_dir);
}

#[test]
fn pack_rejects_out_of_package_path_dependency() {
    let proj = make_temp_project_with_external_path_dep("pblang_pack_external_dep");
    let out_path = proj.join("app.tar.gz");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("pack")
        .arg(&proj)
        .arg(&out_path)
        .output()
        .expect("run pblang pack with external path dep");
    assert!(
        !out.status.success(),
        "pack should fail for out-of-package local path dependency"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("must not escape the package root")
            || stderr.contains("must stay inside the package root"),
        "stderr should mention packaging path confinement: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&proj);
    let _ =
        std::fs::remove_dir_all(std::env::temp_dir().join("pblang_bundle_external_dep_shared"));
}

#[test]
fn bundle_rejects_out_of_package_path_dependency() {
    let proj = make_temp_project_with_external_path_dep("pblang_bundle_external_dep");
    let bundle_dir = proj.join("bundle-out");

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("bundle")
        .arg(&proj)
        .arg(&bundle_dir)
        .output()
        .expect("run pblang bundle with external path dep");
    assert!(
        !out.status.success(),
        "bundle should fail for out-of-package local path dependency"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("must not escape the package root")
            || stderr.contains("must stay inside the package root"),
        "stderr should mention packaging path confinement: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&proj);
    let _ =
        std::fs::remove_dir_all(std::env::temp_dir().join("pblang_bundle_external_dep_shared"));
}

#[test]
fn bundle_require_registry_sha256_flag_rejects_missing_checksum() {
    let base = std::env::temp_dir().join("pblang_bundle_require_sha_flag");
    let _ = std::fs::remove_dir_all(&base);
    let registry_dir = base.join("registry");
    let consumer = base.join("consumer");
    std::fs::create_dir_all(&registry_dir).unwrap();
    std::fs::create_dir_all(&consumer).unwrap();

    let tarball_path = registry_dir.join("pkg-1.0.0.tar.gz");
    {
        let f = std::fs::File::create(&tarball_path).unwrap();
        let mut enc = flate2::write::GzEncoder::new(f, flate2::Compression::default());
        let mut tar_builder = tar::Builder::new(&mut enc);
        let mut header = tar::Header::new_gnu();
        let bytes = b"export fn id() { return 1; }\n";
        header.set_path("lib.sapp").unwrap();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder.append(&header, bytes as &[u8]).unwrap();
        tar_builder.finish().unwrap();
        drop(tar_builder);
        enc.finish().unwrap();
    }

    let index = serde_json::json!({
        "packages": {
            "@pblang/strict-bundle": {
                "1.0.0": { "url": "pkg-1.0.0.tar.gz", "sha256": null }
            }
        }
    });
    std::fs::write(registry_dir.join("index.json"), index.to_string()).unwrap();
    std::fs::write(
        consumer.join("pblang.toml"),
        r#"[package]
name = "bundle-consumer"
version = "0.1.0"

[dependencies]
"@pblang/strict-bundle" = "1.0.0"
"#,
    )
    .unwrap();
    std::fs::write(consumer.join("main.sapp"), "print 42;\n").unwrap();

    let bin = env!("CARGO_BIN_EXE_pblang");
    let out = std::process::Command::new(bin)
        .arg("bundle")
        .arg("--registry-index")
        .arg(registry_file_url(&registry_dir.join("index.json")))
        .arg("--require-registry-sha256")
        .arg(&consumer)
        .arg(consumer.join("bundle-out"))
        .output()
        .expect("run pblang bundle --require-registry-sha256");
    assert!(
        !out.status.success(),
        "bundle should fail when strict sha flag is set and index sha256 is missing"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("sha256 required"),
        "stderr should mention strict checksum requirement: {}",
        stderr
    );

    let _ = std::fs::remove_dir_all(&base);
}
