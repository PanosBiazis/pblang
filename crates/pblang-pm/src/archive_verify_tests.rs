use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(registry_cache_staging(&std::env::temp_dir(), "archive-verify-test").unwrap())
    }
    fn archive(&self, entries: &[(&[u8], tar::EntryType)]) -> (PathBuf, Vec<u8>) {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for &(name, kind) in entries {
            assert!(name.len() < 100);
            let mut header = tar::Header::new_gnu();
            header.as_mut_bytes()[..name.len()].copy_from_slice(name);
            header.set_entry_type(kind);
            header.set_mode(0o700);
            let data: &[u8] = if kind == tar::EntryType::Regular {
                b"x"
            } else {
                b""
            };
            header.set_size(data.len() as u64);
            header.set_cksum();
            builder.append(&header, data).unwrap();
        }
        let bytes = builder.into_inner().unwrap().finish().unwrap();
        let path = self.0.join("package.tar.gz");
        std::fs::write(&path, &bytes).unwrap();
        (path, bytes)
    }
    fn valid(&self) -> (PathBuf, Vec<u8>) {
        self.archive(&[
            (b"pblang.toml", tar::EntryType::Regular),
            (b"main.sapp", tar::EntryType::Regular),
        ])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove owned archive fixture");
    }
}

#[test]
fn canonical_sidecar_keeps_all_extensions() {
    for name in ["package.tar.gz", "archive", "name.with.dots.tgz"] {
        assert_eq!(
            tarball_hmac_sidecar_path(Path::new(name)),
            PathBuf::from(format!("{name}.sha256"))
        );
    }
}

#[test]
#[cfg(unix)]
fn canonical_sidecar_preserves_non_utf8_filename() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    let path = PathBuf::from(std::ffi::OsString::from_vec(b"package\xff.tar.gz".to_vec()));
    assert_eq!(
        tarball_hmac_sidecar_path(&path).as_os_str().as_bytes(),
        b"package\xff.tar.gz.sha256"
    );
}

#[test]
fn unsigned_and_dot_prefixed_archives_are_structure_only_without_extraction() {
    let fixture = Fixture::new();
    let (path, bytes) = fixture.archive(&[
        (b"./", tar::EntryType::Directory),
        (b"./pblang.toml", tar::EntryType::Regular),
        (b"./src/", tar::EntryType::Directory),
        (b"./src/main.sapp", tar::EntryType::Regular),
    ]);
    assert_eq!(
        verify_tarball_with_key(&path, None).unwrap(),
        TarballVerification::StructureOnly
    );
    assert_eq!(std::fs::read_dir(&fixture.0).unwrap().count(), 1);
    let destination = fixture.0.join("extracted");
    unpack_tarball_gz(&bytes, &destination).unwrap();
    assert_eq!(
        std::fs::read(destination.join("src/main.sapp")).unwrap(),
        b"x"
    );
}

#[test]
fn sidecar_requires_key_and_authenticates_the_multi_extension_archive() {
    let fixture = Fixture::new();
    let (path, bytes) = fixture.valid();
    let sidecar = tarball_hmac_sidecar_path(&path);
    // Synthetic key is deliberately padded: validation must not trim the actual key.
    let key = " fixture-only-key ";
    let signature = compute_tarball_hmac(&bytes, key.as_bytes());
    std::fs::write(&sidecar, &signature).unwrap();
    for key in [None, Some(""), Some(" \t")] {
        assert!(verify_tarball_with_key(&path, key)
            .unwrap_err()
            .contains("nonblank"));
    }
    for ending in ["", "\n", "\r\n"] {
        std::fs::write(&sidecar, format!("{}{ending}", signature.to_uppercase())).unwrap();
        assert_eq!(
            verify_tarball_with_key(&path, Some(key)).unwrap(),
            TarballVerification::HmacVerified
        );
    }
    assert_eq!(
        verify_tarball_with_key(&path, Some("wrong-fixture-key")).unwrap_err(),
        "HMAC signature mismatch"
    );
    assert_eq!(
        verify_archive_sidecar(&path, b"modified", Some(key)).unwrap_err(),
        "HMAC signature mismatch"
    );
}

#[test]
fn sidecar_rejects_malformed_oversize_and_directory() {
    let fixture = Fixture::new();
    let (path, _) = fixture.valid();
    let sidecar = tarball_hmac_sidecar_path(&path);
    for contents in [
        vec![],
        vec![b'0'; 63],
        vec![b'0'; 65],
        vec![b'g'; 64],
        vec![b'0'; 129],
        vec![0xff; 64],
        format!(" {}", "0".repeat(64)).into_bytes(),
        format!("{}\n\n", "0".repeat(64)).into_bytes(),
    ] {
        std::fs::write(&sidecar, contents).unwrap();
        assert!(verify_tarball_with_key(&path, Some("fixture-key")).is_err());
    }
    std::fs::remove_file(&sidecar).unwrap();
    std::fs::create_dir(&sidecar).unwrap();
    assert!(verify_tarball_with_key(&path, Some("fixture-key"))
        .unwrap_err()
        .contains("regular"));
}

#[test]
#[cfg(unix)]
fn sidecar_rejects_existing_and_dangling_symlinks() {
    let fixture = Fixture::new();
    let (path, bytes) = fixture.valid();
    let target = fixture.0.join("signature");
    std::fs::write(&target, compute_tarball_hmac(&bytes, b"fixture-key")).unwrap();
    let sidecar = tarball_hmac_sidecar_path(&path);
    std::os::unix::fs::symlink(&target, &sidecar).unwrap();
    assert!(verify_tarball_with_key(&path, Some("fixture-key"))
        .unwrap_err()
        .contains("non-symlink"));
    std::fs::remove_file(&target).unwrap();
    assert!(verify_tarball_with_key(&path, Some("fixture-key")).is_err());
}

#[test]
fn archive_escape_paths_are_rejected_by_verify_and_unpack() {
    let fixture = Fixture::new();
    for name in [
        b"../escape".as_slice(),
        b"/absolute",
        b"C:/drive",
        b"file:stream",
        b"dir\\file",
        b"line\nbreak",
        b"bad\xff",
        b"dir/../escape",
    ] {
        let (path, bytes) = fixture.archive(&[
            (b"pblang.toml", tar::EntryType::Regular),
            (b"main.sapp", tar::EntryType::Regular),
            (name, tar::EntryType::Regular),
        ]);
        assert!(
            verify_tarball_with_key(&path, None).is_err(),
            "verify accepted {name:?}"
        );
        let destination = fixture.0.join("rejected");
        assert!(
            unpack_tarball_gz(&bytes, &destination).is_err(),
            "unpack accepted {name:?}"
        );
        assert!(
            !destination.exists(),
            "failed extraction retained its new destination"
        );
    }
}

#[test]
fn archive_requires_exact_regular_root_manifest_and_regular_module() {
    let fixture = Fixture::new();
    for entries in [
        vec![
            (b"nested/pblang.toml".as_slice(), tar::EntryType::Regular),
            (b"main.sapp", tar::EntryType::Regular),
        ],
        vec![
            (b"evilpblang.toml", tar::EntryType::Regular),
            (b"main.sapp", tar::EntryType::Regular),
        ],
        vec![
            (b"pblang.toml", tar::EntryType::Directory),
            (b"main.sapp", tar::EntryType::Regular),
        ],
        vec![
            (b"pblang.toml", tar::EntryType::Regular),
            (b"main.sapp", tar::EntryType::Directory),
        ],
    ] {
        let (path, _) = fixture.archive(&entries);
        assert!(verify_tarball_with_key(&path, None).is_err());
    }
}

#[test]
fn archive_rejects_normalized_duplicates_and_links() {
    let fixture = Fixture::new();
    for (name, kind) in [
        (b"./main.sapp".as_slice(), tar::EntryType::Regular),
        (b"link", tar::EntryType::Symlink),
        (b"hardlink", tar::EntryType::Link),
    ] {
        let (path, bytes) = fixture.archive(&[
            (b"pblang.toml", tar::EntryType::Regular),
            (b"main.sapp", tar::EntryType::Regular),
            (name, kind),
        ]);
        assert!(verify_tarball_with_key(&path, None).is_err());
        let destination = fixture.0.join("rejected");
        assert!(unpack_tarball_gz(&bytes, &destination).is_err());
        assert!(!destination.exists());
    }
}

#[test]
fn path_guard_bounds_names_and_rejects_controls_without_echo() {
    for bytes in [
        vec![b'a'; 4097],
        b"embedded\0name".to_vec(),
        b"name\x7f".to_vec(),
        vec![],
    ] {
        let error = admit_archive_path(&bytes, tar::EntryType::Regular, &mut Default::default())
            .unwrap_err();
        assert!(!error.contains("embedded"));
    }
}

#[test]
fn local_registry_decodes_file_url_before_sidecar_admission() {
    let fixture = Fixture::new();
    let (_, bytes) = fixture.valid();
    let path = fixture.0.join("encoded package.tar.gz");
    std::fs::write(&path, &bytes).unwrap();
    let url = url::Url::from_file_path(&path).unwrap();
    assert!(url.as_str().contains("%20"));
    std::fs::write(
        tarball_hmac_sidecar_path(&path),
        compute_tarball_hmac(&bytes, b"fixture-key"),
    )
    .unwrap();
    assert!(verify_local_archive_sidecar(url.as_str(), &bytes, None).is_err());
    verify_local_archive_sidecar(url.as_str(), &bytes, Some("fixture-key")).unwrap();
    assert!(verify_local_archive_sidecar(url.as_str(), b"tampered", Some("fixture-key")).is_err());
    for bad in [
        "file://server/share/archive.tar.gz",
        "file:///archive.tar.gz?query=1",
    ] {
        assert!(verify_local_archive_sidecar(bad, &bytes, Some("fixture-key")).is_err());
    }
}

#[test]
fn directory_payload_is_rejected_before_body_skip() {
    let fixture = Fixture::new();
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_path("directory").unwrap();
    header.set_entry_type(tar::EntryType::Directory);
    header.set_mode(0o700);
    header.set_size(1);
    header.set_cksum();
    builder.append(&header, b"x".as_slice()).unwrap();
    let bytes = builder.into_inner().unwrap().finish().unwrap();
    let path = fixture.0.join("directory.tar.gz");
    std::fs::write(&path, &bytes).unwrap();
    assert!(verify_tarball_with_key(&path, None)
        .unwrap_err()
        .contains("zero payload"));
    let destination = fixture.0.join("rejected");
    assert!(unpack_tarball_gz(&bytes, &destination)
        .unwrap_err()
        .contains("zero payload"));
    assert!(!destination.exists());
}

#[test]
fn generated_package_round_trips_with_existing_module_extensions() {
    let fixture = Fixture::new();
    for extension in MODULE_EXTENSIONS {
        let bytes = build_tarball_bytes(vec![
            (
                "pblang.toml".to_string(),
                b"[package]\nname = \"fixture\"\nversion = \"0.1.0\"\n".to_vec(),
            ),
            (format!("src/main.{extension}"), b"fixture".to_vec()),
        ])
        .unwrap();
        let path = fixture.0.join("generated.tar.gz");
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            verify_tarball_with_key(&path, None).unwrap(),
            TarballVerification::StructureOnly
        );
        let destination = fixture.0.join(extension);
        unpack_tarball_gz(&bytes, &destination).unwrap();
        assert_eq!(
            std::fs::read(destination.join(format!("src/main.{extension}"))).unwrap(),
            b"fixture"
        );
    }
}

#[test]
fn decoded_budget_never_turns_exhaustion_into_clean_eof() {
    let mut reader = ArchiveReadBudget::new(b"12345".as_slice(), 4);
    let mut buffer = [0; 4];
    reader.read_exact(&mut buffer).unwrap();
    assert_eq!(&buffer, b"1234");
    assert_eq!(reader.read(&mut []).unwrap(), 0);
    assert_eq!(
        reader.read(&mut buffer).unwrap_err().kind(),
        std::io::ErrorKind::InvalidData
    );
    let mut shorter = ArchiveReadBudget::new(b"123".as_slice(), 4);
    let mut contents = Vec::new();
    shorter.read_to_end(&mut contents).unwrap();
    assert_eq!(contents, b"123");
}

#[test]
fn decoded_budget_rejects_gnu_extension_before_logical_entry() {
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_path("././@LongLink").unwrap();
    header.set_entry_type(tar::EntryType::GNULongName);
    let data = vec![b'a'; 2048];
    header.set_size(data.len() as u64);
    header.set_cksum();
    builder.append(&header, data.as_slice()).unwrap();
    let bytes = builder.into_inner().unwrap().finish().unwrap();
    let decoder = flate2::read::GzDecoder::new(bytes.as_slice());
    let mut archive = tar::Archive::new(ArchiveReadBudget::new(decoder, 1024));
    let error = match archive.entries().unwrap().next().unwrap() {
        Ok(_) => panic!("oversized extension unexpectedly yielded an entry"),
        Err(error) => error,
    };
    assert!(error
        .to_string()
        .contains("decompressed input exceeds admission budget"));
}

#[test]
fn gnu_long_names_remain_compatible() {
    let fixture = Fixture::new();
    let name = format!("{}/main.sapp", "a".repeat(150));
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for path in ["pblang.toml", name.as_str()] {
        let mut header = tar::Header::new_gnu();
        header.set_size(7);
        header.set_mode(0o644);
        builder
            .append_data(&mut header, path, b"fixture".as_slice())
            .unwrap();
    }
    let bytes = builder.into_inner().unwrap().finish().unwrap();
    let path = fixture.0.join("long.tar.gz");
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        verify_tarball_with_key(&path, None).unwrap(),
        TarballVerification::StructureOnly
    );
    let destination = fixture.0.join("long");
    unpack_tarball_gz(&bytes, &destination).unwrap();
    assert_eq!(std::fs::read(destination.join(name)).unwrap(), b"fixture");
}

#[test]
fn pax_effective_size_cannot_bypass_file_or_directory_budget() {
    let fixture = Fixture::new();
    for (kind, size, expected) in [
        (
            tar::EntryType::Regular,
            MAX_REGISTRY_ENTRY_BYTES + 1,
            "exceeds cap",
        ),
        (tar::EntryType::Directory, 1, "zero payload"),
    ] {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        let size = size.to_string();
        builder
            .append_pax_extensions([("size", size.as_bytes())])
            .unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_path("effective-size").unwrap();
        header.set_entry_type(kind);
        header.set_size(0);
        header.set_mode(0o700);
        header.set_cksum();
        builder.append(&header, std::io::empty()).unwrap();
        let bytes = builder.into_inner().unwrap().finish().unwrap();
        let path = fixture.0.join("pax.tar.gz");
        std::fs::write(&path, &bytes).unwrap();
        assert!(verify_tarball_with_key(&path, None)
            .unwrap_err()
            .contains(expected));
        let destination = fixture.0.join("rejected");
        let error = unpack_tarball_gz(&bytes, &destination).unwrap_err();
        assert!(
            error.contains("cap is") || error.contains("zero payload"),
            "{error}"
        );
        assert!(!destination.exists());
    }
}
