use super::*;

struct Fixture {
    root: tempfile::TempDir,
    dst: std::path::PathBuf,
    state: std::path::PathBuf,
    archive: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let dst = root.path().join("destination");
        let state = root.path().join("records");
        let archive = root.path().join("archive.gz");
        let file = std::fs::File::create(&archive).unwrap();
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            file,
            flate2::Compression::default(),
        ));
        for (name, body, mode) in [
            ("root/tool", "original", 0o755),
            ("root/other", "other", 0o644),
            ("root/sub/data", "data", 0o644),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(mode);
            header.set_cksum();
            tar.append_data(&mut header, name, body.as_bytes()).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
        Self {
            root,
            dst,
            state,
            archive,
        }
    }
    fn definition(&self, policy: &str, extra: &str) -> crate::model::Archive {
        let operation: crate::model::Operation = toml::from_str(&format!(
            r#"kind = "archive"
dst = {:?}
policy = {:?}
version = "1"
url = "https://example.test/release.tar.gz"
sha256 = {:?}
format = "tar.gz"
root = "root"
{extra}
"#,
            self.dst.to_str().unwrap(),
            policy,
            record::digest(&self.archive).unwrap()
        ))
        .unwrap();
        operation.validate().unwrap();
        let crate::model::Operation::Archive(a) = operation else {
            unreachable!()
        };
        a
    }
    fn install(&self, a: &crate::model::Archive) -> Result<(), crate::error::MoiError> {
        let work = tempfile::tempdir_in(self.root.path()).unwrap();
        install_downloaded(a, &self.dst, &self.archive, work.path(), &self.state)
    }
    fn satisfied(
        &self,
        a: &crate::model::Archive,
    ) -> Result<bool, crate::error::MoiError> {
        satisfied_in(a, &self.dst, &self.state)
    }
}

#[test]
fn pinned_full_tree_converges_and_repairs_content_modes_extra_files_and_missing_target()
{
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let a = f.definition("pinned", "");
    assert!(!f.satisfied(&a).unwrap());
    f.install(&a).unwrap();
    assert!(f.satisfied(&a).unwrap());
    for change in 0..4 {
        match change {
            0 => std::fs::write(f.dst.join("tool"), "changed").unwrap(),
            1 => std::fs::set_permissions(
                f.dst.join("tool"),
                std::fs::Permissions::from_mode(0o644),
            )
            .unwrap(),
            2 => std::fs::write(f.dst.join("extra"), "extra").unwrap(),
            _ => std::fs::remove_dir_all(&f.dst).unwrap(),
        }
        assert!(!f.satisfied(&a).unwrap());
        f.install(&a).unwrap();
        assert!(f.satisfied(&a).unwrap());
        assert_eq!(
            std::fs::read_to_string(f.dst.join("tool")).unwrap(),
            "original"
        );
        assert!(!f.dst.join("extra").exists());
    }
}

#[test]
fn selected_files_share_destination_and_never_remove_unselected_files() {
    let f = Fixture::new();
    let a = f.definition("pinned", "include = [\"tool\", \"sub/data\"]");
    std::fs::create_dir(&f.dst).unwrap();
    std::fs::write(f.dst.join("unrelated"), "keep").unwrap();
    f.install(&a).unwrap();
    assert!(f.satisfied(&a).unwrap());
    assert!(!f.dst.join("other").exists());
    assert_eq!(std::fs::read_dir(&f.state).unwrap().count(), 2);
    std::fs::write(f.dst.join("sub/data"), "changed").unwrap();
    assert!(!f.satisfied(&a).unwrap());
    f.install(&a).unwrap();
    assert_eq!(
        std::fs::read_to_string(f.dst.join("unrelated")).unwrap(),
        "keep"
    );
    let reduced = f.definition("pinned", "include = [\"tool\"]");
    f.install(&reduced).unwrap();
    assert!(f.dst.join("sub/data").exists());
    assert_eq!(std::fs::read_dir(&f.state).unwrap().count(), 2);
}

#[test]
fn preserve_keeps_self_updates_but_repairs_required_paths_and_definition_changes() {
    let f = Fixture::new();
    let mut a = f.definition("preserve", "required_paths = [\"tool\"]");
    f.install(&a).unwrap();
    std::fs::write(f.dst.join("tool"), "self-update").unwrap();
    std::fs::remove_file(f.dst.join("other")).unwrap();
    assert!(f.satisfied(&a).unwrap());
    a.url = "https://mirror.test/tool".into();
    assert!(f.satisfied(&a).unwrap());
    a.version = "0.9".into();
    assert!(!f.satisfied(&a).unwrap());
    f.install(&a).unwrap();
    assert_eq!(
        std::fs::read_to_string(f.dst.join("tool")).unwrap(),
        "original"
    );
    std::fs::remove_file(f.dst.join("tool")).unwrap();
    assert!(!f.satisfied(&a).unwrap());
    f.install(&a).unwrap();
    assert!(f.satisfied(&a).unwrap());
}

#[test]
fn preserve_optional_selection_can_disappear_without_reinstall() {
    let f = Fixture::new();
    let a = f.definition(
        "preserve",
        "include = [\"tool\", \"other\"]\nrequired_paths = [\"tool\"]",
    );
    f.install(&a).unwrap();
    std::fs::remove_file(f.dst.join("other")).unwrap();
    assert!(f.satisfied(&a).unwrap());
    std::fs::remove_file(f.dst.join("tool")).unwrap();
    assert!(!f.satisfied(&a).unwrap());
}

#[test]
fn changing_required_paths_establishes_new_metadata_and_checks_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let a = f.definition("preserve", "required_paths = [\"other\"]");
    f.install(&a).unwrap();
    let a = f.definition("preserve", "required_paths = [\"tool\"]");
    assert!(!f.satisfied(&a).unwrap());
    f.install(&a).unwrap();
    assert!(f.satisfied(&a).unwrap());
    std::fs::set_permissions(
        f.dst.join("tool"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(!f.satisfied(&a).unwrap());
}

#[test]
fn unowned_targets_corrupt_records_and_wrong_types_are_errors() {
    let f = Fixture::new();
    let a = f.definition("pinned", "include = [\"tool\"]");
    std::fs::create_dir(&f.dst).unwrap();
    std::fs::write(f.dst.join("tool"), "existing").unwrap();
    assert!(f.satisfied(&a).is_err());
    assert!(f.install(&a).is_err());
    assert_eq!(
        std::fs::read_to_string(f.dst.join("tool")).unwrap(),
        "existing"
    );
    std::fs::remove_file(f.dst.join("tool")).unwrap();
    f.install(&a).unwrap();
    let record = record::path_for(&f.state, &f.dst.join("tool"));
    std::fs::write(&record, "{}").unwrap();
    assert!(f.satisfied(&a).is_err());
    assert!(f.install(&a).is_err());
    std::fs::remove_file(&record).unwrap();
    std::fs::remove_file(f.dst.join("tool")).unwrap();
    std::fs::create_dir(f.dst.join("tool")).unwrap();
    assert!(f.satisfied(&a).is_err());
}

#[test]
fn all_selected_records_are_checked_even_after_an_unsatisfied_target() {
    let f = Fixture::new();
    let a = f.definition("pinned", "include = [\"tool\", \"other\"]");
    f.install(&a).unwrap();
    std::fs::remove_file(f.dst.join("other")).unwrap();
    std::fs::write(record::path_for(&f.state, &f.dst.join("tool")), "broken").unwrap();
    assert!(f.satisfied(&a).is_err());
}

#[test]
fn checksum_or_missing_include_fail_before_existing_placement_changes() {
    let f = Fixture::new();
    let mut a = f.definition("pinned", "include = [\"tool\"]");
    f.install(&a).unwrap();
    a.sha256 = "0".repeat(64);
    assert!(f.install(&a).is_err());
    assert_eq!(
        std::fs::read_to_string(f.dst.join("tool")).unwrap(),
        "original"
    );
    a.sha256 = record::digest(&f.archive).unwrap();
    a.include = Some(vec!["tool".into(), "missing".into()]);
    assert!(f.install(&a).is_err());
    assert_eq!(
        std::fs::read_to_string(f.dst.join("tool")).unwrap(),
        "original"
    );
}

#[test]
fn include_order_digest_case_and_url_do_not_trigger_reinstallation() {
    let f = Fixture::new();
    let mut a = f.definition("pinned", "include = [\"tool\", \"other\"]");
    f.install(&a).unwrap();
    a.include.as_mut().unwrap().reverse();
    a.sha256 = a.sha256.to_uppercase();
    a.url = "https://mirror.test/file".into();
    assert!(f.satisfied(&a).unwrap());
}

#[test]
fn safe_internal_links_are_copied_and_pinned() {
    let f = Fixture::new();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(&f.archive)
        .unwrap();
    let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
        file,
        flate2::Compression::default(),
    ));
    let mut h = tar::Header::new_gnu();
    h.set_mode(0o755);
    h.set_size(4);
    h.set_cksum();
    tar.append_data(&mut h, "root/bin/tool", b"data".as_slice())
        .unwrap();
    let mut h = tar::Header::new_gnu();
    h.set_mode(0o777);
    h.set_entry_type(tar::EntryType::Symlink);
    h.set_size(0);
    h.set_link_name("bin/tool").unwrap();
    h.set_cksum();
    tar.append_data(&mut h, "root/tool", std::io::empty())
        .unwrap();
    tar.into_inner().unwrap().finish().unwrap();
    let a = f.definition("pinned", "");
    f.install(&a).unwrap();
    assert_eq!(
        std::fs::read_link(f.dst.join("tool")).unwrap(),
        std::path::Path::new("bin/tool")
    );
    assert!(f.satisfied(&a).unwrap());
    let a = f.definition("preserve", "required_paths = [\"tool\"]");
    f.install(&a).unwrap();
    assert!(f.satisfied(&a).unwrap());
}

#[test]
fn zip_and_xz_extract_real_archives() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let zip_path = root.path().join("a.zip");
    let mut zip = zip::ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
    zip.start_file(
        "bin/tool",
        zip::write::SimpleFileOptions::default().unix_permissions(0o755),
    )
    .unwrap();
    zip.write_all(b"zip").unwrap();
    zip.finish().unwrap();
    let dst = root.path().join("zip");
    extract::unpack(&zip_path, crate::model::Format::Zip, &dst).unwrap();
    assert_eq!(std::fs::read(dst.join("bin/tool")).unwrap(), b"zip");
    let xz_path = root.path().join("a.xz");
    let mut tar = tar::Builder::new(xz2::write::XzEncoder::new(
        std::fs::File::create(&xz_path).unwrap(),
        1,
    ));
    let mut h = tar::Header::new_gnu();
    h.set_mode(0o755);
    h.set_size(2);
    h.set_cksum();
    tar.append_data(&mut h, "tool", b"xz".as_slice()).unwrap();
    tar.into_inner().unwrap().finish().unwrap();
    let dst = root.path().join("xz");
    extract::unpack(&xz_path, crate::model::Format::TarXz, &dst).unwrap();
    assert_eq!(std::fs::read(dst.join("tool")).unwrap(), b"xz");
}

#[test]
fn extraction_rejects_traversal_outside_links_and_special_files() {
    for (name, kind, target) in [
        ("../escape", tar::EntryType::Regular, None),
        ("/absolute", tar::EntryType::Regular, None),
        ("link", tar::EntryType::Symlink, Some("../outside")),
        ("fifo", tar::EntryType::Fifo, None),
    ] {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a.gz");
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            std::fs::File::create(&path).unwrap(),
            flate2::Compression::default(),
        ));
        let mut h = tar::Header::new_gnu();
        h.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        h.set_mode(0o755);
        h.set_size(0);
        h.set_entry_type(kind);
        if let Some(target) = target {
            h.set_link_name(target).unwrap();
        }
        h.set_cksum();
        tar.append(&h, std::io::empty()).unwrap();
        tar.into_inner().unwrap().finish().unwrap();
        assert!(
            extract::unpack(
                &path,
                crate::model::Format::TarGz,
                &root.path().join("out")
            )
            .is_err(),
            "{name}"
        );
    }
}

#[test]
fn symlink_chains_are_resolved_before_parent_components() {
    for target in ["d/link/../outside", "d/link/../../outside"] {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        std::fs::create_dir_all(source.join("d")).unwrap();
        std::os::unix::fs::symlink("..", source.join("d/link")).unwrap();
        std::os::unix::fs::symlink(target, source.join("escape")).unwrap();
        assert!(extract::copy_tree(&source, &root.path().join("copy")).is_err());
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    std::fs::create_dir_all(source.join("d")).unwrap();
    std::fs::write(source.join("data"), "data").unwrap();
    std::os::unix::fs::symlink("../data", source.join("d/link")).unwrap();
    extract::copy_tree(&source, &root.path().join("copy")).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.path().join("copy/d/link")).unwrap(),
        "data"
    );
}

#[test]
fn dangling_preserve_required_symlink_needs_repair() {
    let f = Fixture::new();
    let a = f.definition("preserve", "required_paths = [\"tool\"]");
    f.install(&a).unwrap();
    std::fs::remove_file(f.dst.join("tool")).unwrap();
    std::os::unix::fs::symlink("missing", f.dst.join("tool")).unwrap();
    assert!(!f.satisfied(&a).unwrap());
}

#[test]
fn record_write_failure_keeps_the_new_placement_and_old_backup_for_manual_recovery() {
    let f = Fixture::new();
    let a = f.definition("pinned", "include = [\"tool\"]");
    std::fs::create_dir(&f.dst).unwrap();
    let target = f.dst.join("tool");
    std::fs::write(&target, "old").unwrap();
    let staging = tempfile::Builder::new()
        .prefix(".moi-archive-")
        .tempdir_in(&f.dst)
        .unwrap();
    let staged = staging.path().to_path_buf();
    let candidate = staged.join("new");
    std::fs::write(&candidate, "new").unwrap();
    let rec = record::create(
        &a,
        &f.dst,
        &target,
        Some(std::path::Path::new("tool")),
        &candidate,
    )
    .unwrap();
    // A real filesystem failure at record persistence, after the successful rename.
    std::fs::write(&f.state, "not a directory").unwrap();
    let err = finish_replacements(
        &a,
        &f.dst,
        vec![Replacement {
            target: target.clone(),
            staging,
            record: rec,
            moved: false,
        }],
        &f.state,
    )
    .unwrap_err();
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
    assert_eq!(std::fs::read_to_string(staged.join("old")).unwrap(), "old");
    let message = err.to_string();
    assert!(
        message.contains("backup=")
            && message.contains("record=")
            && message.contains("target=")
    );
}
