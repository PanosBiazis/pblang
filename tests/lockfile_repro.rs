//! Lockfile reproducibility (v0.3): regenerating the lockfile from the same manifest
//! must produce the same output.

use pblang_pm::{generate_lockfile, generate_lockfile_with_registry, write_lockfile};

#[test]
fn lockfile_reproducibility_empty_deps() {
    let manifest_content = r#"[package]
name = "repro-test"
version = "0.1.0"

[dependencies]
"#;
    let temp = std::env::temp_dir().join("biapryx-lockfile-repro-test");
    let _ = std::fs::create_dir_all(&temp);
    let manifest_path = temp.join("pblang.toml");
    std::fs::write(&manifest_path, manifest_content).expect("write manifest");

    let lock1 = generate_lockfile(&temp).expect("first generate");
    let lock2 = generate_lockfile(&temp).expect("second generate");

    let s1 = write_lockfile(&lock1).expect("serialize 1");
    let s2 = write_lockfile(&lock2).expect("serialize 2");

    assert_eq!(
        s1, s2,
        "lockfile must be reproducible: two generates must match"
    );
    assert_eq!(lock1.manifest_sha256, lock2.manifest_sha256);

    let _ = std::fs::remove_file(&manifest_path);
    let _ = std::fs::remove_dir(&temp);
}

#[test]
fn lockfile_reproducibility_registry_dependency() {
    let base = std::env::temp_dir().join("biapryx-lockfile-repro-registry");
    let _ = std::fs::create_dir_all(&base);
    let registry_dir = base.join("registry");
    let pkg = base.join("pkg");
    let consumer = base.join("consumer");
    let _ = std::fs::create_dir_all(&registry_dir);
    let _ = std::fs::create_dir_all(&pkg);
    let _ = std::fs::create_dir_all(&consumer);

    std::fs::write(
        pkg.join("pblang.toml"),
        r#"[package]
name = "@sapphire/repro-pkg"
version = "1.0.0"

[dependencies]
"#,
    )
    .unwrap();
    std::fs::write(pkg.join("lib.sapp"), "export fn id(x) { return x; }\n").unwrap();
    pblang_pm::publish_to_registry_dir(&pkg, &registry_dir).expect("publish test package");

    let index_path = registry_dir.join("index.json");
    let index_abs = std::fs::canonicalize(&index_path).unwrap_or(index_path);
    let index_url = format!("file:///{}", index_abs.to_string_lossy().replace('\\', "/"));

    std::fs::write(
        consumer.join("pblang.toml"),
        r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"@sapphire/repro-pkg" = "1.0.0"
"#,
    )
    .unwrap();

    let lock1 = generate_lockfile_with_registry(&consumer, Some(&index_url), false)
        .expect("first generate");
    let lock2 = generate_lockfile_with_registry(&consumer, Some(&index_url), false)
        .expect("second generate");

    let s1 = write_lockfile(&lock1).expect("serialize 1");
    let s2 = write_lockfile(&lock2).expect("serialize 2");

    assert_eq!(s1, s2, "registry lockfile must be reproducible");

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
fn lockfile_reproducibility_git_dependency() {
    let base = std::env::temp_dir().join("biapryx-lockfile-repro-git");
    let _ = std::fs::create_dir_all(&base);
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

    std::fs::write(
        consumer.join("pblang.toml"),
        format!(
            r#"[package]
name = "consumer"
version = "0.1.0"

[dependencies]
"gitdep" = {{ git = "{}", rev = "{}" }}
"#,
            repo_abs.to_string_lossy(),
            rev
        ),
    )
    .unwrap();

    let lock1 = generate_lockfile(&consumer).expect("first generate");
    let lock2 = generate_lockfile(&consumer).expect("second generate");

    let s1 = write_lockfile(&lock1).expect("serialize 1");
    let s2 = write_lockfile(&lock2).expect("serialize 2");

    assert_eq!(s1, s2, "git lockfile must be reproducible");

    let _ = std::fs::remove_dir_all(&base);
}
