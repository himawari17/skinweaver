use crate::{
    catalog::Target,
    ini::{SkinIni, property_has_path},
    library::{
        Library, Operation, Progress, Skin, check_cancel, copy_tree, files_recursive, relative_path,
    },
};
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

pub const TRANSPARENT: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 0x0d, 0x49, 0x48, 0x44, 0x52, 0, 0, 0,
    1, 0, 0, 0, 1, 8, 4, 0, 0, 0, 0xb5, 0x1c, 0x0c, 2, 0, 0, 0, 0x0b, 0x49, 0x44, 0x41, 0x54, 0x78,
    0xda, 0x63, 0x64, 0x60, 0, 0, 0, 6, 0, 2, 0x30, 0x81, 0xd0, 0x2f, 0, 0, 0, 0, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Source {
    #[default]
    Default,
    Unchanged,
    Blank,
    Skin(PathBuf),
}
#[derive(Clone, Debug)]
pub struct Selection {
    pub target: Target,
    pub source: Source,
}

#[derive(Clone, Debug)]
struct Credit {
    skin: String,
    author: String,
    checksum: String,
}
#[derive(Default)]
struct Credits(BTreeMap<String, Credit>);
impl Credits {
    fn read(path: &Path) -> Result<Self> {
        let path = path.join("credits.ini");
        let mut credits = Self::default();
        if !path.exists() {
            return Ok(credits);
        }
        let text = fs::read_to_string(path).context("Read existing credits")?;
        let mut current = None;
        for line in text.lines().map(str::trim) {
            if let Some(version) = line.strip_prefix("version:") {
                ensure!(version.trim() == "0.1", "Unsupported credits.ini version");
            }
            if let Some(section) = line.strip_prefix("[\"").and_then(|s| s.strip_suffix("\"]")) {
                current = section
                    .split_once("\" by \"")
                    .map(|(skin, author)| (skin.to_owned(), author.to_owned()));
            } else if let Some((skin, author)) = &current
                && let Some((checksum, name)) = line.split_once(" - ")
            {
                credits.0.insert(
                    name.into(),
                    Credit {
                        skin: skin.clone(),
                        author: author.clone(),
                        checksum: checksum.into(),
                    },
                );
            }
        }
        Ok(credits)
    }
    fn add(&mut self, relative: &Path, skin: &Skin, bytes: &[u8]) {
        let name = relative.to_string_lossy().replace('\\', "/");
        self.0.insert(
            name,
            Credit {
                skin: skin.name.clone(),
                author: skin.author.clone(),
                checksum: format!("{:x}", md5::compute(bytes)),
            },
        );
    }
    fn write(&self, path: &Path) -> Result<()> {
        let mut groups: BTreeMap<(&str, &str), Vec<(&str, &str)>> = BTreeMap::new();
        for (name, credit) in &self.0 {
            groups
                .entry((&credit.skin, &credit.author))
                .or_default()
                .push((name, &credit.checksum));
        }
        let mut text = String::from("version: 0.1\ngenerated_by: Skinweaver\n\n");
        for ((skin, author), files) in groups {
            text.push_str(&format!(
                "[\"{}\" by \"{}\"]\n",
                skin.replace(['\n', '\r', '"'], " "),
                author.replace(['\n', '\r', '"'], " ")
            ));
            for (name, checksum) in files {
                text.push_str(&format!("{checksum} - {name}\n"));
            }
            text.push('\n');
        }
        fs::write(path.join("credits.ini"), text)?;
        Ok(())
    }
}

fn refresh_credits(folder: &Path) -> Result<()> {
    let mut credits = Credits::read(folder)?;
    let mut removed = vec![];
    for (name, credit) in &mut credits.0 {
        let file = folder.join(relative_path(name)?);
        if file.is_file() {
            credit.checksum = format!("{:x}", md5::compute(fs::read(file)?));
        } else {
            removed.push(name.clone());
        }
    }
    for name in removed {
        credits.0.remove(&name);
    }
    credits.write(folder)
}

fn copy_asset(
    source: &Path,
    relative: &Path,
    destination: &Path,
    skin: &Skin,
    credits: &mut Credits,
    cancel: &AtomicBool,
) -> Result<()> {
    check_cancel(cancel)?;
    let relative = relative_path(&relative.to_string_lossy())?;
    let bytes = fs::read(source).with_context(|| format!("Read {}", source.display()))?;
    let target = destination.join(&relative);
    fs::create_dir_all(target.parent().context("Asset directory")?)?;
    fs::write(target, &bytes)?;
    credits.add(&relative, skin, &bytes);
    Ok(())
}

/// Top-level files of a folder, listed once per operation.
fn top_files(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut files = vec![];
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    Ok(files)
}

/// Directory listings reused for the whole operation: sources never change while a
/// mix is written (the destination is always a separate staging folder).
#[derive(Default)]
struct Listings {
    top: HashMap<PathBuf, Vec<PathBuf>>,
    recursive: HashMap<PathBuf, Vec<PathBuf>>,
}
impl Listings {
    fn top(&mut self, folder: &Path) -> Result<&[PathBuf]> {
        if !self.top.contains_key(folder) {
            let files = top_files(folder)?;
            self.top.insert(folder.to_path_buf(), files);
        }
        Ok(&self.top[folder])
    }
    fn recursive(&mut self, folder: &Path) -> Result<&[PathBuf]> {
        if !self.recursive.contains_key(folder) {
            let files = files_recursive(folder)?;
            self.recursive.insert(folder.to_path_buf(), files);
        }
        Ok(&self.recursive[folder])
    }
}

fn copy_property_files(
    skin: &Skin,
    key: &str,
    value: &str,
    destination: &Path,
    credits: &mut Credits,
    listings: &mut Listings,
    cancel: &AtomicBool,
) -> Result<()> {
    if !property_has_path(key) || value.is_empty() {
        return Ok(());
    }
    let prefix = relative_path(value)?;
    let folder = prefix.parent().unwrap_or(Path::new(""));
    let search = skin.path.join(folder);
    if !search.exists() {
        return Ok(());
    }
    let pattern = prefix
        .file_name()
        .context("Asset prefix")?
        .to_string_lossy()
        .to_ascii_lowercase();
    for file in listings.recursive(&skin.path)? {
        if file.parent() == Some(search.as_path())
            && file.file_name().is_some_and(|n| {
                n.to_string_lossy()
                    .to_ascii_lowercase()
                    .starts_with(&pattern)
            })
        {
            copy_asset(
                file,
                file.strip_prefix(&skin.path)?,
                destination,
                skin,
                credits,
                cancel,
            )?;
        }
    }
    Ok(())
}

pub fn apply_selections(
    library: &Library,
    destination: &Path,
    ini: &mut SkinIni,
    selections: &[Selection],
    cancel: &AtomicBool,
    progress: &Progress<'_>,
) -> Result<()> {
    let mut credits = Credits::read(destination)?;
    let mut skins = BTreeMap::new();
    for selection in selections {
        if let Source::Skin(path) = &selection.source
            && !skins.contains_key(path)
        {
            library.validate_skin(path)?;
            skins.insert(path.clone(), Skin::read(path.clone(), false)?);
        }
    }
    let mut references = vec![];
    let mut listings = Listings::default();
    // Top-level files of the destination, kept in step with what this loop writes and
    // removes, so the folder is listed once instead of once per element.
    let mut written = top_files(destination)?;
    for (index, selection) in selections.iter().enumerate() {
        check_cancel(cancel)?;
        if selection.source == Source::Unchanged {
            continue;
        }
        progress(
            index as f32 / selections.len().max(1) as f32 * 0.8,
            "Подготовка элементов / Preparing elements",
        );
        let source = if let Source::Skin(path) = &selection.source {
            skins.get(path)
        } else {
            None
        };
        match &selection.target {
            Target::File {
                pattern,
                audio,
                suffixes,
                ..
            } => {
                let mut kept = Vec::with_capacity(written.len());
                for file in std::mem::take(&mut written) {
                    if !selection.target.matches(&file) {
                        kept.push(file);
                        continue;
                    }
                    // A name listed twice in different case is one file on Windows.
                    match fs::remove_file(&file) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => return Err(e.into()),
                    }
                    credits.0.remove(
                        &file
                            .file_name()
                            .context("Filename")?
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
                written = kept;
                if let Some(skin) = source {
                    for file in listings.top(&skin.path)? {
                        if selection.target.matches(file) {
                            copy_asset(
                                file,
                                file.strip_prefix(&skin.path)?,
                                destination,
                                skin,
                                &mut credits,
                                cancel,
                            )?;
                            let target = destination.join(file.strip_prefix(&skin.path)?);
                            if !written.iter().any(|w| w.as_os_str() == target.as_os_str()) {
                                written.push(target);
                            }
                        }
                    }
                } else if selection.source == Source::Blank {
                    let prefix = pattern.trim_end_matches('*');
                    let suffixes = if pattern.ends_with('*') {
                        suffixes.clone().unwrap_or_else(|| vec![String::new()])
                    } else {
                        vec![String::new()]
                    };
                    for suffix in suffixes {
                        let name = format!("{prefix}{suffix}");
                        relative_path(&name)?;
                        if *audio {
                            // A valid silent WAV avoids decoding errors from a zero-byte file.
                            let silent: &[u8] = b"RIFF$\0\0\0WAVEfmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data\0\0\0\0";
                            let file = destination.join(format!("{name}.wav"));
                            fs::write(&file, silent)?;
                            if file.parent() == Some(destination) {
                                written.push(file);
                            }
                        } else {
                            for scale in ["", "@2x"] {
                                let file = destination.join(format!("{name}{scale}.png"));
                                fs::write(&file, TRANSPARENT)?;
                                if file.parent() == Some(destination) {
                                    written.push(file);
                                }
                            }
                        }
                    }
                }
            }
            Target::Property { section, key } => {
                ini.section_mut(section).remove(key);
                if let Some(skin) = source
                    && let Some(value) = skin.ini.get(section, key)
                {
                    if property_has_path(key) && !value.is_empty() {
                        relative_path(value)?;
                    }
                    ini.set(section, key, value);
                    references.push((skin, key.clone(), value.to_owned()));
                }
            }
            Target::Section {
                section,
                key,
                value,
            } => {
                ini.sections.retain(|s| {
                    !(s.name.eq_ignore_ascii_case(section) && s.get(key) == Some(value))
                });
                if let Some(skin) = source
                    && let Some(s) =
                        skin.ini.sections.iter().find(|s| {
                            s.name.eq_ignore_ascii_case(section) && s.get(key) == Some(value)
                        })
                {
                    ini.sections.push(s.clone());
                    for (key, value) in &s.values {
                        references.push((skin, key.clone(), value.clone()));
                    }
                }
            }
        }
    }
    for (skin, key, value) in references {
        copy_property_files(
            skin,
            &key,
            &value,
            destination,
            &mut credits,
            &mut listings,
            cancel,
        )?;
    }
    credits.write(destination)?;
    Ok(())
}

pub fn create_mix(
    library: &Library,
    name: &str,
    selections: &[Selection],
    cancel: &AtomicBool,
    progress: &Progress<'_>,
) -> Result<Operation> {
    library.ensure_new_name(name)?;
    let target = library.destination(name, false)?;
    let stage = library.staging()?;
    let staged = stage.path().join("skin");
    fs::create_dir(&staged)?;
    let mut ini = SkinIni::new(name);
    apply_selections(library, &staged, &mut ini, selections, cancel, progress)?;
    fs::write(staged.join("skin.ini"), ini.to_string())?;
    progress(0.9, "Сохранение / Saving");
    library.commit(vec![(target, Some(staged))], "mix", name, cancel)
}

/// A new skin that starts as a full copy of `base`. `selections` (with `Unchanged` for
/// whatever was left alone) are applied on top, so files outside the catalog survive.
pub fn create_from_skin(
    library: &Library,
    base: &Path,
    name: &str,
    selections: &[Selection],
    cancel: &AtomicBool,
    progress: &Progress<'_>,
) -> Result<Operation> {
    library.ensure_new_name(name)?;
    library.validate_skin(base)?;
    let skin = Skin::read(base.to_path_buf(), false)?;
    let target = library.destination(name, false)?;
    let stage = library.staging()?;
    let staged = stage.path().join("skin");
    copy_tree(base, &staged, cancel)?;
    let mut ini = skin.ini;
    ini.set("General", "Name", name);
    apply_selections(library, &staged, &mut ini, selections, cancel, progress)?;
    refresh_credits(&staged)?;
    fs::write(staged.join("skin.ini"), ini.to_string())?;
    progress(0.9, "Сохранение / Saving");
    library.commit(vec![(target, Some(staged))], "mix", name, cancel)
}

pub fn modify_skins(
    library: &Library,
    paths: &[PathBuf],
    selections: &[Selection],
    flags: &crate::modify::Modifications,
    cancel: &AtomicBool,
    progress: &Progress<'_>,
) -> Result<Operation> {
    ensure!(!paths.is_empty(), "No skins selected");
    let stage = library.staging()?;
    let mut replacements = vec![];
    for (i, path) in paths.iter().enumerate() {
        library.validate_skin(path)?;
        let skin = Skin::read(path.clone(), false)?;
        let staged = stage.path().join(i.to_string());
        copy_tree(path, &staged, cancel)?;
        let mut ini = skin.ini;
        apply_selections(
            library,
            &staged,
            &mut ini,
            selections,
            cancel,
            &|p, status| progress((i as f32 + p) / paths.len() as f32, status),
        )?;
        flags.apply(&staged, &mut ini, cancel)?;
        refresh_credits(&staged)?;
        fs::write(staged.join("skin.ini"), ini.to_string())?;
        replacements.push((path.clone(), Some(staged)));
    }
    progress(0.95, "Сохранение / Saving");
    library.commit(replacements, "modify", &paths.len().to_string(), cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modify::Modifications;

    #[test]
    fn copy_of_a_skin_keeps_its_files_and_takes_the_changes() {
        let root = tempfile::tempdir().unwrap();
        for name in ["Base", "Other"] {
            fs::create_dir_all(root.path().join("Skins").join(name)).unwrap();
        }
        let library = Library::new(root.path()).unwrap();
        let base = library.root.join("Skins/Base");
        let other = library.root.join("Skins/Other");
        fs::write(
            base.join("skin.ini"),
            "[General]\nName: Base\n[Colours]\nCombo1: 1,2,3\n",
        )
        .unwrap();
        fs::write(base.join("custom-thing.png"), TRANSPARENT).unwrap();
        fs::write(base.join("cursor.png"), TRANSPARENT).unwrap();
        fs::write(other.join("hitcircle.png"), TRANSPARENT).unwrap();
        let selections = [
            Selection {
                target: Target::File {
                    pattern: "hitcircle".into(),
                    audio: false,
                    animated: false,
                    suffixes: None,
                },
                source: Source::Skin(other.clone()),
            },
            Selection {
                target: Target::File {
                    pattern: "cursor".into(),
                    audio: false,
                    animated: false,
                    suffixes: None,
                },
                source: Source::Unchanged,
            },
        ];
        create_from_skin(
            &library,
            &base,
            "Copy",
            &selections,
            &AtomicBool::new(false),
            &|_, _| {},
        )
        .unwrap();
        let copy = library.root.join("Skins/Copy");
        for file in ["custom-thing.png", "cursor.png", "hitcircle.png"] {
            assert!(copy.join(file).is_file(), "{file}");
        }
        let ini = SkinIni::parse(&fs::read_to_string(copy.join("skin.ini")).unwrap());
        assert_eq!(ini.get("General", "Name"), Some("Copy"));
        assert_eq!(ini.get("Colours", "Combo1"), Some("1,2,3"));
        assert!(base.join("cursor.png").is_file());
        assert!(!base.join("hitcircle.png").exists());
    }
    #[test]
    fn cancellation_and_failed_batch_leave_all_originals_untouched() {
        let root = tempfile::tempdir().unwrap();
        for name in ["One", "Two"] {
            fs::create_dir_all(root.path().join("Skins").join(name)).unwrap();
        }
        let library = Library::new(root.path()).unwrap();
        let one = library.root.join("Skins/One");
        let two = library.root.join("Skins/Two");
        fs::write(one.join("cursor.png"), TRANSPARENT).unwrap();
        fs::write(two.join("cursor.png"), b"corrupt image").unwrap();
        let before_one = crate::library::fingerprint(&one).unwrap();
        let before_two = crate::library::fingerprint(&two).unwrap();
        let flags = Modifications {
            cursor_color: Some([255, 0, 0]),
            ..Modifications::default()
        };
        assert!(
            modify_skins(
                &library,
                &[one.clone(), two.clone()],
                &[],
                &flags,
                &AtomicBool::new(false),
                &|_, _| {}
            )
            .is_err()
        );
        assert_eq!(crate::library::fingerprint(&one).unwrap(), before_one);
        assert_eq!(crate::library::fingerprint(&two).unwrap(), before_two);
        assert!(
            create_mix(
                &library,
                "Cancelled",
                &[],
                &AtomicBool::new(true),
                &|_, _| {}
            )
            .is_err()
        );
        assert!(!library.root.join("Skins/Cancelled").exists());
        assert!(library.history().unwrap().is_empty());
    }
    #[test]
    fn mixed_files_references_and_credits_are_ported() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Skins/Source");
        fs::create_dir_all(path.join("digits")).unwrap();
        fs::write(path.join("skin.ini"), "[General]\nAuthor: Someone\n[Fonts]\nHitCirclePrefix: digits/custom\n[Mania]\nKeys: 4\nNoteImage0: digits/custom-1").unwrap();
        fs::write(path.join("digits/custom-1.png"), TRANSPARENT).unwrap();
        fs::write(path.join("cursor@2x.png"), TRANSPARENT).unwrap();
        let library = Library::new(root.path()).unwrap();
        let selections = vec![
            Selection {
                target: Target::File {
                    pattern: "cursor".into(),
                    audio: false,
                    animated: false,
                    suffixes: None,
                },
                source: Source::Skin(path.clone()),
            },
            Selection {
                target: Target::Property {
                    section: "Fonts".into(),
                    key: "HitCirclePrefix".into(),
                },
                source: Source::Skin(path.clone()),
            },
            Selection {
                target: Target::Section {
                    section: "Mania".into(),
                    key: "Keys".into(),
                    value: "4".into(),
                },
                source: Source::Skin(path.clone()),
            },
        ];
        let op = create_mix(
            &library,
            "Mix",
            &selections,
            &AtomicBool::new(false),
            &|_, _| {},
        )
        .unwrap();
        let mix = library.root.join("Skins/Mix");
        assert!(mix.join("cursor@2x.png").exists());
        assert!(mix.join("digits/custom-1.png").exists());
        assert!(
            fs::read_to_string(mix.join("credits.ini"))
                .unwrap()
                .contains("Someone")
        );
        assert!(
            create_mix(
                &library,
                "Mix",
                &selections,
                &AtomicBool::new(false),
                &|_, _| {}
            )
            .is_err()
        );
        library.undo(&op.id).unwrap();
        assert!(!mix.exists());
    }
    #[test]
    fn malicious_ini_cannot_read_outside_skin() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Skins/Bad");
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("skin.ini"),
            "[Fonts]\nHitCirclePrefix: ../../secret",
        )
        .unwrap();
        let library = Library::new(dir.path()).unwrap();
        let options = [Selection {
            target: Target::Property {
                section: "Fonts".into(),
                key: "HitCirclePrefix".into(),
            },
            source: Source::Skin(path),
        }];
        assert!(
            create_mix(
                &library,
                "Safe",
                &options,
                &AtomicBool::new(false),
                &|_, _| {}
            )
            .is_err()
        );
        assert!(!dir.path().join("Skins/Safe").exists());
    }
}
