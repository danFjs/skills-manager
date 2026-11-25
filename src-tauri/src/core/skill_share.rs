//! Local sharing: pack library skills into one `.zip` and unpack such an
//! archive back into importable skill folders.
//!
//! Only skill files travel — one top-level folder per skill. No tags,
//! presets, agent toggles, settings or machine paths are written, and
//! nothing touches the network. The receiving side imports the folders
//! through the normal batch-import path, which skips names already in the
//! library.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::skill_metadata::is_valid_skill_dir;

/// Refuse archives that would unpack to more than this (zip-bomb guard).
pub const MAX_UNPACKED_BYTES: u64 = 1024 * 1024 * 1024;
/// …or that hold more entries than any real skill collection would.
pub const MAX_ENTRIES: usize = 50_000;

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ShareSummary {
    pub skills: usize,
    pub files: usize,
    pub bytes: u64,
}

/// A unique top-level folder name: two skills whose folders share a name
/// (from different sources) must not merge inside the archive.
fn unique_name(base: &str, used: &mut HashSet<String>) -> String {
    let mut name = base.to_string();
    let mut n = 2;
    while !used.insert(name.to_lowercase()) {
        name = format!("{base}-{n}");
        n += 1;
    }
    name
}

/// Write `skills` (folder name, skill directory) into a zip at `dest`.
/// Written to a sibling temp file and renamed, so a failed export never
/// leaves a truncated archive behind.
pub fn write_archive(skills: &[(String, PathBuf)], dest: &Path) -> Result<ShareSummary> {
    if skills.is_empty() {
        bail!("No skills selected to share");
    }
    let parent = dest
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let tmp = tempfile::Builder::new()
        .prefix(".skills-share-")
        .suffix(".zip.part")
        .tempfile_in(parent)
        .with_context(|| format!("Cannot write to {}", parent.display()))?;

    let mut summary = ShareSummary::default();
    {
        let mut zip = zip::ZipWriter::new(tmp.as_file());
        let mut used = HashSet::new();
        for (name, dir) in skills {
            if !dir.is_dir() {
                bail!("Skill folder is missing: {}", dir.display());
            }
            let folder = unique_name(name, &mut used);
            summary.skills += 1;
            for entry in WalkDir::new(dir)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| e.file_name() != ".git")
            {
                let entry = entry?;
                // Links are skipped: following one could pull in files from
                // outside the skill, and zip has no portable link entry.
                if !entry.file_type().is_file() {
                    continue;
                }
                let rel = entry.path().strip_prefix(dir)?;
                let rel = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                let mut options = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = entry.metadata()?.permissions().mode();
                    options = options.unix_permissions(mode & 0o777);
                }
                zip.start_file(format!("{folder}/{rel}"), options)?;
                let mut file = std::fs::File::open(entry.path())?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                zip.write_all(&buf)?;
                summary.files += 1;
                summary.bytes += buf.len() as u64;
            }
        }
        zip.finish()?;
    }
    tmp.persist(dest)
        .with_context(|| format!("Cannot save {}", dest.display()))?;
    Ok(summary)
}

/// An unpacked share archive. The temp directory lives as long as this
/// value, so import from `skill_dirs` before dropping it.
pub struct Unpacked {
    _dir: tempfile::TempDir,
    pub skill_dirs: Vec<PathBuf>,
}

/// Unpack `src` and find the skill folders in it: every top-level folder
/// that is a skill, or the root itself when the archive is a single skill
/// (also looking through one wrapping folder, as zipping a folder makes).
pub fn extract_archive(src: &Path) -> Result<Unpacked> {
    let file = std::fs::File::open(src).with_context(|| format!("Cannot open {}", src.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("Not a zip archive")?;
    if archive.len() > MAX_ENTRIES {
        bail!("Archive has too many files to be a skill collection");
    }
    let mut total = 0u64;
    for i in 0..archive.len() {
        total = total.saturating_add(archive.by_index(i)?.size());
        if total > MAX_UNPACKED_BYTES {
            bail!("Archive is too large to import");
        }
    }

    let dir = tempfile::tempdir()?;
    super::installer::safe_extract(&mut archive, dir.path())?;

    let mut root = dir.path().to_path_buf();
    let children = |p: &Path| -> Result<Vec<PathBuf>> {
        let mut out: Vec<PathBuf> = std::fs::read_dir(p)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n != "__MACOSX"))
            .collect();
        out.sort();
        Ok(out)
    };
    let mut skill_dirs = Vec::new();
    for _ in 0..2 {
        if is_valid_skill_dir(&root) {
            skill_dirs.push(root.clone());
            break;
        }
        let kids = children(&root)?;
        skill_dirs = kids.iter().filter(|p| is_valid_skill_dir(p)).cloned().collect();
        // One wrapping folder and nothing else: look inside it once.
        if skill_dirs.is_empty() && kids.len() == 1 {
            root = kids[0].clone();
            continue;
        }
        break;
    }
    if skill_dirs.is_empty() {
        bail!("No skills found in this archive");
    }
    Ok(Unpacked {
        _dir: dir,
        skill_dirs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn skill(root: &Path, name: &str, body: &str) -> PathBuf {
        let dir = root.join(name);
        fs::create_dir_all(dir.join("scripts")).unwrap();
        fs::write(dir.join("SKILL.md"), format!("---\nname: {name}\n---\n{body}")).unwrap();
        fs::write(dir.join("scripts/run.sh"), "echo hi").unwrap();
        dir
    }

    fn names(u: &Unpacked) -> Vec<String> {
        u.skill_dirs
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn round_trips_skills_and_skips_git_internals() {
        let src = tempdir().unwrap();
        let a = skill(src.path(), "alpha", "a");
        let b = skill(src.path(), "beta", "b");
        fs::create_dir_all(a.join(".git")).unwrap();
        fs::write(a.join(".git/config"), "secret remote").unwrap();

        let out = tempdir().unwrap();
        let dest = out.path().join("share.zip");
        let summary =
            write_archive(&[("alpha".into(), a), ("beta".into(), b)], &dest).unwrap();
        assert_eq!(summary.skills, 2);
        assert_eq!(summary.files, 4, "SKILL.md + script per skill, no .git");

        let unpacked = extract_archive(&dest).unwrap();
        assert_eq!(names(&unpacked), vec!["alpha", "beta"]);
        let alpha = &unpacked.skill_dirs[0];
        assert!(fs::read_to_string(alpha.join("SKILL.md")).unwrap().contains("name: alpha"));
        assert!(alpha.join("scripts/run.sh").is_file());
        assert!(!alpha.join(".git").exists());
    }

    #[cfg(unix)]
    #[test]
    fn keeps_executable_bits() {
        use std::os::unix::fs::PermissionsExt;
        let src = tempdir().unwrap();
        let a = skill(src.path(), "alpha", "a");
        fs::set_permissions(a.join("scripts/run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
        let dest = src.path().join("s.zip");
        write_archive(&[("alpha".into(), a)], &dest).unwrap();
        let unpacked = extract_archive(&dest).unwrap();
        let mode = fs::metadata(unpacked.skill_dirs[0].join("scripts/run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o111, 0o111);
    }

    #[test]
    fn same_folder_names_stay_separate() {
        let one = tempdir().unwrap();
        let two = tempdir().unwrap();
        let a = skill(one.path(), "pdf", "first");
        let b = skill(two.path(), "pdf", "second");
        let dest = one.path().join("s.zip");
        write_archive(&[("pdf".into(), a), ("pdf".into(), b)], &dest).unwrap();
        let unpacked = extract_archive(&dest).unwrap();
        assert_eq!(names(&unpacked), vec!["pdf", "pdf-2"]);
    }

    #[test]
    fn accepts_a_single_skill_or_one_wrapping_folder() {
        let src = tempdir().unwrap();
        let a = skill(src.path(), "alpha", "a");

        // a zip whose root is the skill itself
        let root_zip = src.path().join("root.zip");
        {
            let f = fs::File::create(&root_zip).unwrap();
            let mut z = zip::ZipWriter::new(f);
            z.start_file("SKILL.md", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"---\nname: solo\n---\n").unwrap();
            z.finish().unwrap();
        }
        assert_eq!(extract_archive(&root_zip).unwrap().skill_dirs.len(), 1);

        // skills nested inside one wrapping folder ("Compress folder")
        let wrapped = src.path().join("wrapped.zip");
        {
            let f = fs::File::create(&wrapped).unwrap();
            let mut z = zip::ZipWriter::new(f);
            for sk in ["x", "y"] {
                z.start_file(format!("my-skills/{sk}/SKILL.md"), zip::write::SimpleFileOptions::default())
                    .unwrap();
                z.write_all(b"---\nname: n\n---\n").unwrap();
            }
            z.finish().unwrap();
        }
        let unpacked = extract_archive(&wrapped).unwrap();
        assert_eq!(names(&unpacked), vec!["x", "y"]);
        drop(a);
    }

    #[test]
    fn rejects_empty_selections_non_zips_and_skill_less_archives() {
        let dir = tempdir().unwrap();
        assert!(write_archive(&[], &dir.path().join("x.zip")).is_err());

        let not_zip = dir.path().join("x.zip");
        fs::write(&not_zip, "plain text").unwrap();
        assert!(extract_archive(&not_zip).is_err());

        let empty = dir.path().join("empty.zip");
        {
            let f = fs::File::create(&empty).unwrap();
            let mut z = zip::ZipWriter::new(f);
            z.start_file("notes.txt", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"hi").unwrap();
            z.finish().unwrap();
        }
        assert!(extract_archive(&empty)
            .err()
            .unwrap()
            .to_string()
            .contains("No skills"));
    }

    #[test]
    fn a_failed_export_leaves_no_file_behind() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("s.zip");
        let missing = dir.path().join("gone");
        assert!(write_archive(&[("gone".into(), missing)], &dest).is_err());
        assert!(!dest.exists());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0, "temp file cleaned up");
    }
}
