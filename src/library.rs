use crate::{ini::SkinIni, settings::atomic_write};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub type Cancel = Arc<AtomicBool>;
pub type Progress<'a> = dyn Fn(f32, &str) + 'a;
pub fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Relaxed), "Отменено / Cancelled");
    Ok(())
}

pub fn validate_name(name: &str) -> Result<()> {
    ensure!(
        !name.trim().is_empty() && name == name.trim() && name.chars().count() <= 120,
        "Некорректное имя / Invalid name"
    );
    ensure!(
        !name.ends_with(['.', ' '])
            && !name
                .chars()
                .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)),
        "Недопустимые символы / Invalid filename characters"
    );
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    ensure!(
        !matches!(stem.as_str(), "." | ".." | "CON" | "PRN" | "AUX" | "NUL")
            && !(stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.ends_with(['1', '2', '3', '4', '5', '6', '7', '8', '9'])),
        "Зарезервированное имя / Reserved filename"
    );
    Ok(())
}

pub fn relative_path(value: &str) -> Result<PathBuf> {
    let value = value.replace('\\', "/");
    ensure!(
        !value.is_empty() && !value.contains(':') && !value.contains('\0'),
        "Invalid relative asset path"
    );
    let path = PathBuf::from(value);
    ensure!(
        path.components().all(|c| matches!(c, Component::Normal(_))),
        "Путь за пределами скина / Path escapes skin"
    );
    Ok(path)
}

pub fn reject_link(path: &Path) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("Read {}", path.display()))?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "Symlinks are not supported: {}",
        path.display()
    );
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Reparse points are not supported: {}",
            path.display()
        );
    }
    Ok(())
}

/// Checks a directory entry without extra system calls where the platform allows it.
/// On Windows `DirEntry::metadata` comes straight from the directory listing.
fn checked_entry(entry: &fs::DirEntry) -> Result<(fs::FileType, fs::Metadata)> {
    let file_type = entry.file_type()?;
    ensure!(
        !file_type.is_symlink(),
        "Symlinks are not supported: {}",
        entry.path().display()
    );
    let metadata = entry.metadata()?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Reparse points are not supported: {}",
            entry.path().display()
        );
    }
    Ok((file_type, metadata))
}

/// Visits every regular file below `root` together with its metadata.
pub fn walk(root: &Path, visit: &mut dyn FnMut(PathBuf, &fs::Metadata)) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let (file_type, metadata) = checked_entry(&entry)?;
        if file_type.is_dir() {
            walk(&entry.path(), visit)?;
        } else if file_type.is_file() {
            visit(entry.path(), &metadata);
        }
    }
    Ok(())
}

pub fn files_recursive(root: &Path) -> Result<Vec<PathBuf>> {
    reject_link(root)?;
    let mut files = vec![];
    walk(root, &mut |path, _| files.push(path))?;
    files.sort();
    Ok(files)
}

pub fn copy_tree(source: &Path, destination: &Path, cancel: &AtomicBool) -> Result<()> {
    fs::create_dir_all(destination)?;
    for path in files_recursive(source)? {
        check_cancel(cancel)?;
        let target = destination.join(path.strip_prefix(source)?);
        fs::create_dir_all(target.parent().context("File parent")?)?;
        fs::copy(&path, &target).with_context(|| format!("Copy {}", path.display()))?;
    }
    Ok(())
}

pub fn fingerprint(root: &Path) -> Result<String> {
    let mut digest = md5::Context::new();
    let mut buffer = [0; 64 * 1024];
    for path in files_recursive(root)? {
        digest.consume(path.strip_prefix(root)?.to_string_lossy().as_bytes());
        digest.consume([0]);
        let mut file = fs::File::open(path)?;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            digest.consume(&buffer[..count]);
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[derive(Clone, Debug)]
pub struct Skin {
    pub path: PathBuf,
    pub name: String,
    pub author: String,
    pub hidden: bool,
    pub ini: SkinIni,
    pub elements: usize,
    pub bytes: u64,
    pub modified: u64,
}
impl Skin {
    pub fn read(path: PathBuf, hidden: bool) -> Result<Self> {
        reject_link(&path)?;
        let name = path
            .file_name()
            .context("Skin name")?
            .to_string_lossy()
            .into_owned();
        let mut ini_path = None;
        let mut bytes = 0;
        let mut modified = 0;
        let mut elements = 0;
        walk(&path, &mut |file, metadata| {
            bytes += metadata.len();
            modified = modified.max(
                metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .unwrap_or_default()
                    .as_secs(),
            );
            if file.parent() == Some(path.as_path())
                && file
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("skin.ini"))
            {
                ini_path = Some(file.clone());
            }
            if file.extension().is_some_and(|e| {
                ["png", "jpg", "jpeg", "wav", "ogg", "mp3"]
                    .iter()
                    .any(|ext| e.eq_ignore_ascii_case(ext))
            }) {
                elements += 1;
            }
        })?;
        let ini = match ini_path {
            Some(file) => SkinIni::read(&file)?,
            None => SkinIni::new(&name),
        };
        let author = ini.get("General", "Author").unwrap_or("").to_owned();
        Ok(Self {
            path,
            name,
            author,
            hidden,
            ini,
            elements,
            bytes,
            modified,
        })
    }
}

/// Cheap change marker for a skin folder: folder mtime plus skin.ini mtime and size.
/// Adding, removing or replacing files changes the folder time; editing skin.ini changes its own.
fn skin_stamp(path: &Path, folder: &fs::Metadata) -> u128 {
    let time = |m: &fs::Metadata| {
        m.modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos())
    };
    let ini = fs::metadata(path.join("skin.ini"))
        .map(|m| time(&m) ^ (u128::from(m.len()) << 64))
        .unwrap_or(0);
    time(folder).wrapping_mul(31) ^ ini
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CachedSkin {
    path: PathBuf,
    hidden: bool,
    stamp: u128,
    ini: String,
    elements: usize,
    bytes: u64,
    modified: u64,
}

/// Remembers the result of the last library scan so the app can show skins instantly
/// on start and only re-read folders that changed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ScanCache {
    version: u32,
    root: PathBuf,
    skins: Vec<CachedSkin>,
}
const CACHE_VERSION: u32 = 1;
impl ScanCache {
    pub fn load(file: &Path, root: &Path) -> Option<Self> {
        let cache: Self = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
        (cache.version == CACHE_VERSION && cache.root == root).then_some(cache)
    }
    pub fn save(&self, file: &Path) -> Result<()> {
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        atomic_write(file, &serde_json::to_vec(self)?)
    }
    pub fn skins(&self) -> Vec<Skin> {
        let mut skins: Vec<_> = self.skins.iter().map(CachedSkin::to_skin).collect();
        skins.sort_by_key(|s| s.name.to_lowercase());
        skins
    }
}
impl CachedSkin {
    fn to_skin(&self) -> Skin {
        let ini = SkinIni::parse(&self.ini);
        Skin {
            name: self
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            author: ini.get("General", "Author").unwrap_or("").to_owned(),
            path: self.path.clone(),
            hidden: self.hidden,
            ini,
            elements: self.elements,
            bytes: self.bytes,
            modified: self.modified,
        }
    }
}

/// Folder timestamps of Skins and HiddenSkins; changes when skins are added, removed or renamed.
pub fn library_stamp(root: &Path) -> u128 {
    ["Skins", "HiddenSkins"]
        .iter()
        .map(|folder| {
            fs::metadata(root.join(folder))
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos())
        })
        .fold(0u128, |acc, t| acc.wrapping_mul(1_000_003) ^ t)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub backup: Option<String>,
    pub after: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub time: u64,
    pub changes: Vec<Change>,
    #[serde(default)]
    pub undone: bool,
}

/// For every operation: whether a newer operation that is still in effect changed one of
/// its files, so it cannot be undone first. One pass from newest to oldest instead of
/// comparing every pair of operations.
pub fn undo_blocked(history: &[Operation]) -> Vec<bool> {
    let mut touched: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut blocked = vec![false; history.len()];
    for (i, operation) in history.iter().enumerate().rev() {
        blocked[i] = operation
            .changes
            .iter()
            .any(|c| touched.contains(&c.path.to_lowercase()));
        if !operation.undone {
            touched.extend(operation.changes.iter().map(|c| c.path.to_lowercase()));
        }
    }
    blocked
}

#[derive(Clone, Debug)]
pub struct Library {
    pub root: PathBuf,
}
impl Library {
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        reject_link(root)?;
        let root = fs::canonicalize(root)?;
        ensure!(
            root.join("Skins").is_dir(),
            "Выберите папку osu!, содержащую Skins / Choose an osu! folder containing Skins"
        );
        reject_link(&root.join("Skins"))?;
        if root.join("HiddenSkins").exists() {
            reject_link(&root.join("HiddenSkins"))?;
        }
        Ok(Self { root })
    }
    pub fn scan(&self, cancel: &AtomicBool) -> Result<(Vec<Skin>, Vec<String>)> {
        let (skins, warnings, _) = self.scan_cached(None, cancel)?;
        Ok((skins, warnings))
    }

    /// Scans the library on several threads. Skin folders whose stamp matches `cache`
    /// are taken from the cache without walking their files.
    pub fn scan_cached(
        &self,
        cache: Option<&ScanCache>,
        cancel: &AtomicBool,
    ) -> Result<(Vec<Skin>, Vec<String>, ScanCache)> {
        let mut folders = vec![];
        for (folder, hidden) in [("Skins", false), ("HiddenSkins", true)] {
            let root = self.root.join(folder);
            if !root.exists() {
                continue;
            }
            reject_link(&root)?;
            for entry in fs::read_dir(root)? {
                check_cancel(cancel)?;
                let entry = entry?;
                let Ok(file_type) = entry.file_type() else {
                    continue;
                };
                if !file_type.is_dir() {
                    continue;
                }
                let metadata = entry.metadata()?;
                let path = entry.path();
                let stamp = skin_stamp(&path, &metadata);
                folders.push((path, hidden, stamp));
            }
        }
        let cached: std::collections::HashMap<&Path, &CachedSkin> = cache
            .map(|c| c.skins.iter().map(|s| (s.path.as_path(), s)).collect())
            .unwrap_or_default();
        let next = std::sync::atomic::AtomicUsize::new(0);
        let results = std::sync::Mutex::new(Vec::with_capacity(folders.len()));
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .clamp(2, 8);
        std::thread::scope(|scope| {
            for _ in 0..threads {
                scope.spawn(|| {
                    loop {
                        if cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some((path, hidden, stamp)) = folders.get(index) else {
                            break;
                        };
                        let result = match cached.get(path.as_path()) {
                            Some(entry) if entry.stamp == *stamp && entry.hidden == *hidden => {
                                Ok((*entry).clone())
                            }
                            _ => Skin::read(path.clone(), *hidden).map(|skin| CachedSkin {
                                path: skin.path,
                                hidden: *hidden,
                                stamp: *stamp,
                                ini: skin.ini.to_string(),
                                elements: skin.elements,
                                bytes: skin.bytes,
                                modified: skin.modified,
                            }),
                        };
                        results
                            .lock()
                            .unwrap()
                            .push(result.map_err(|e| format!("{}: {e:#}", path.display())));
                    }
                });
            }
        });
        check_cancel(cancel)?;
        let mut entries = vec![];
        let mut warnings = vec![];
        for result in results.into_inner().unwrap() {
            match result {
                Ok(entry) => entries.push(entry),
                Err(warning) => warnings.push(warning),
            }
        }
        warnings.sort();
        let cache = ScanCache {
            version: CACHE_VERSION,
            root: self.root.clone(),
            skins: entries,
        };
        Ok((cache.skins(), warnings, cache))
    }
    pub fn validate_skin(&self, path: &Path) -> Result<()> {
        let parent = path.parent().context("Missing skin parent")?;
        reject_link(parent)?;
        let parent = fs::canonicalize(parent)?;
        ensure!(
            parent == self.root.join("Skins") || parent == self.root.join("HiddenSkins"),
            "Skin is outside the library"
        );
        validate_name(
            path.file_name()
                .context("Skin name")?
                .to_str()
                .context("Non-Unicode skin name")?,
        )?;
        if path.exists() {
            reject_link(path)?;
            ensure!(path.is_dir(), "Expected a skin directory");
        }
        Ok(())
    }
    pub fn destination(&self, name: &str, hidden: bool) -> Result<PathBuf> {
        validate_name(name)?;
        let parent = self.root.join(if hidden { "HiddenSkins" } else { "Skins" });
        if !parent.exists() {
            fs::create_dir(&parent)?;
        }
        reject_link(&parent)?;
        let target = parent.join(name);
        self.validate_skin(&target)?;
        Ok(target)
    }
    pub fn ensure_new_name(&self, name: &str) -> Result<()> {
        validate_name(name)?;
        ensure!(
            !self.root.join("Skins").join(name).exists()
                && !self.root.join("HiddenSkins").join(name).exists(),
            "Имя уже занято / A skin with this name already exists"
        );
        Ok(())
    }
    pub fn staging(&self) -> Result<tempfile::TempDir> {
        let root = self.backup_root()?;
        Ok(tempfile::Builder::new().prefix("stage-").tempdir_in(root)?)
    }
    fn backup_root(&self) -> Result<PathBuf> {
        let root = self.root.join(".osu-skin-mixer-backups");
        if !root.exists() {
            fs::create_dir(&root)?;
        }
        reject_link(&root)?;
        Ok(root)
    }
    pub fn history(&self) -> Result<Vec<Operation>> {
        let root = self.backup_root()?;
        let file = root.join("history.json");
        let history: Vec<Operation> = if file.exists() {
            reject_link(&file)?;
            serde_json::from_slice(&fs::read(file)?)
                .context("Cannot read recovery history; original files were kept")?
        } else {
            vec![]
        };
        // shortcut: interrupted commits need manual recovery; add guided recovery before automatic repair.
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("op-")
                && entry.path().join("pending.json").exists()
            {
                ensure!(
                    history
                        .iter()
                        .any(|op| op.id == entry.file_name().to_string_lossy()),
                    "Незавершённая операция. Оригиналы сохранены в {}. Восстановите файлы по pending.json перед новыми изменениями / Interrupted operation; recover originals using pending.json before making changes",
                    entry.path().display()
                );
            }
        }
        Ok(history)
    }
    fn write_history(&self, history: &[Operation]) -> Result<()> {
        atomic_write(
            &self.backup_root()?.join("history.json"),
            &serde_json::to_vec_pretty(history)?,
        )
    }

    pub fn commit(
        &self,
        replacements: Vec<(PathBuf, Option<PathBuf>)>,
        kind: &str,
        label: &str,
        cancel: &AtomicBool,
    ) -> Result<Operation> {
        ensure!(!replacements.is_empty(), "No skins selected");
        let replacements: Vec<_> = replacements
            .into_iter()
            .map(|(path, stage)| -> Result<_> {
                self.validate_skin(&path)?;
                Ok((
                    fs::canonicalize(path.parent().context("Skin parent")?)?
                        .join(path.file_name().context("Skin name")?),
                    stage,
                ))
            })
            .collect::<Result<_>>()?;
        let mut history = self.history()?;
        let transaction = tempfile::Builder::new()
            .prefix("op-")
            .tempdir_in(self.backup_root()?)?;
        let id = transaction
            .path()
            .file_name()
            .context("Transaction id")?
            .to_string_lossy()
            .into_owned();
        let mut changes = vec![];
        for (i, (target, staged)) in replacements.iter().enumerate() {
            self.validate_skin(target)?;
            if staged.is_some() && kind != "modify" && kind != "undo" {
                ensure!(!target.exists(), "Имя уже занято / Target already exists");
            }
            ensure!(
                !replacements[..i].iter().any(|(p, _)| p == target),
                "Duplicate operation target"
            );
            if let Some(stage) = staged {
                ensure!(
                    stage.is_dir() && fs::canonicalize(stage)?.starts_with(self.backup_root()?),
                    "Invalid staging directory"
                );
                reject_link(stage)?;
            }
            changes.push(Change {
                path: target
                    .strip_prefix(&self.root)?
                    .to_string_lossy()
                    .into_owned(),
                backup: target.exists().then(|| format!("{id}/{i}")),
                after: staged.as_ref().map(|p| fingerprint(p)).transpose()?,
            });
        }
        check_cancel(cancel)?;
        // The journal is durable before any original directory is moved.
        atomic_write(
            &transaction.path().join("pending.json"),
            &serde_json::to_vec_pretty(&changes)?,
        )?;
        let transaction = transaction.keep();
        let mut moved = 0;
        let result = (|| -> Result<()> {
            for ((target, staged), change) in replacements.iter().zip(&changes) {
                if let Some(backup) = &change.backup {
                    fs::rename(target, self.backup_root()?.join(backup))?;
                }
                moved += 1;
                if let Some(staged) = staged {
                    fs::rename(staged, target)?;
                }
            }
            let operation = Operation {
                id: id.clone(),
                kind: kind.into(),
                label: label.into(),
                time: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                changes: changes.clone(),
                undone: false,
            };
            history.push(operation);
            self.write_history(&history)
        })();
        if let Err(error) = result {
            let rollback = (|| -> Result<()> {
                for (i, ((target, staged), change)) in replacements
                    .iter()
                    .zip(&changes)
                    .take(moved)
                    .enumerate()
                    .rev()
                {
                    if target.exists()
                        && let Some(staged) = staged
                    {
                        fs::rename(target, staged)
                            .with_context(|| format!("Rollback staged skin {i}"))?;
                    }
                    if let Some(backup) = &change.backup {
                        fs::rename(self.backup_root()?.join(backup), target)?;
                    }
                }
                Ok(())
            })();
            if let Err(rollback) = rollback {
                bail!(
                    "{error:#}; rollback: {rollback:#}. Recovery: {}",
                    transaction.display()
                );
            }
            fs::remove_file(transaction.join("pending.json"))?;
            return Err(error.context("Operation was rolled back"));
        }
        fs::remove_file(transaction.join("pending.json")).ok();
        history.pop().context("Committed operation")
    }

    pub fn manage(
        &self,
        paths: &[PathBuf],
        kind: &str,
        new_name: &str,
        cancel: &AtomicBool,
        progress: &Progress<'_>,
    ) -> Result<Operation> {
        ensure!(!paths.is_empty(), "No skins selected");
        let stage = self.staging()?;
        let mut replacements = vec![];
        for (i, path) in paths.iter().enumerate() {
            check_cancel(cancel)?;
            self.validate_skin(path)?;
            ensure!(path.is_dir(), "Skin no longer exists");
            let name = path.file_name().context("Skin name")?.to_string_lossy();
            progress(i as f32 / paths.len() as f32, &name);
            if kind == "delete" {
                replacements.push((path.clone(), None));
                continue;
            }
            let hidden = fs::canonicalize(path.parent().context("Skin parent")?)?
                == self.root.join("HiddenSkins");
            let target = match kind {
                "hide" => self.destination(&name, !hidden)?,
                "rename" | "duplicate" => {
                    ensure!(paths.len() == 1, "Select a single skin");
                    self.ensure_new_name(new_name)?;
                    self.destination(new_name, hidden)?
                }
                _ => bail!("Unknown library operation"),
            };
            ensure!(!target.exists(), "Имя уже занято / Target already exists");
            let staged = stage.path().join(i.to_string());
            copy_tree(path, &staged, cancel)?;
            if kind == "rename" {
                let mut ini = Skin::read(path.clone(), hidden)?.ini;
                ini.set("General", "Name", new_name);
                fs::write(staged.join("skin.ini"), ini.to_string())?;
            }
            if kind != "duplicate" {
                replacements.push((path.clone(), None));
            }
            replacements.push((target, Some(staged)));
        }
        let label = if paths.len() == 1 {
            paths[0]
                .file_name()
                .context("Skin name")?
                .to_string_lossy()
                .into_owned()
        } else {
            paths.len().to_string()
        };
        self.commit(replacements, kind, &label, cancel)
    }

    pub fn undo(&self, id: &str) -> Result<()> {
        let history = self.history()?;
        let index = history
            .iter()
            .position(|op| op.id == id && !op.undone)
            .context("Operation cannot be undone")?;
        let operation = history[index].clone();
        ensure!(
            !history[index + 1..].iter().any(|op| !op.undone
                && op.changes.iter().any(|c| operation
                    .changes
                    .iter()
                    .any(|old| old.path.eq_ignore_ascii_case(&c.path)))),
            "Сначала отмените более поздние изменения / Undo newer changes first"
        );
        let backup_root = self.backup_root()?;
        for change in &operation.changes {
            let target = self.root.join(relative_path(&change.path)?);
            self.validate_skin(&target)?;
            match &change.after {
                Some(expected) => ensure!(
                    target.is_dir() && fingerprint(&target)? == *expected,
                    "Скин изменён извне. Отмена остановлена / Skin changed externally; undo stopped"
                ),
                None => ensure!(!target.exists(), "Target now exists; undo stopped"),
            }
            if let Some(backup) = &change.backup {
                let backup = backup_root.join(relative_path(backup)?);
                ensure!(
                    backup.is_dir()
                        && backup
                            .parent()
                            .and_then(Path::file_name)
                            .is_some_and(|n| n == operation.id.as_str()),
                    "Missing recovery backup"
                );
                files_recursive(&backup)?;
            }
        }
        let stage = self.staging()?;
        let mut replacements = vec![];
        for (i, change) in operation.changes.iter().enumerate() {
            let original = if let Some(backup) = &change.backup {
                let original = stage.path().join(i.to_string());
                copy_tree(
                    &backup_root.join(relative_path(backup)?),
                    &original,
                    &AtomicBool::new(false),
                )?;
                Some(original)
            } else {
                None
            };
            replacements.push((self.root.join(relative_path(&change.path)?), original));
        }
        // Undo is another journaled transaction, so a failed restoration is recoverable too.
        let undo = self.commit(
            replacements,
            "undo",
            &operation.label,
            &AtomicBool::new(false),
        )?;
        let mut updated = self.history()?;
        updated
            .iter_mut()
            .filter(|op| op.id == id || op.id == undo.id)
            .for_each(|op| op.undone = true);
        self.write_history(&updated)?;
        Ok(())
    }

    pub fn export(&self, source: &Path, destination: &Path, cancel: &AtomicBool) -> Result<()> {
        self.validate_skin(source)?;
        let parent = destination.parent().context("Archive folder")?;
        ensure!(
            !fs::canonicalize(parent)?.starts_with(&self.root),
            "Экспортируйте за пределы папки osu! / Export outside the osu! folder"
        );
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        {
            let mut archive = zip::ZipWriter::new(temp.as_file_mut());
            for path in files_recursive(source)? {
                check_cancel(cancel)?;
                let name = path
                    .strip_prefix(source)?
                    .to_string_lossy()
                    .replace('\\', "/");
                archive.start_file(
                    name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )?;
                std::io::copy(&mut fs::File::open(path)?, &mut archive)?;
            }
            archive.finish()?;
        }
        temp.as_file_mut().flush()?;
        temp.as_file().sync_all()?;
        check_cancel(cancel)?;
        temp.persist(destination).map_err(|e| e.error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn undo_is_blocked_only_by_newer_live_changes() {
        let op = |paths: &[&str], undone: bool| Operation {
            id: String::new(),
            kind: "modify".into(),
            label: String::new(),
            time: 0,
            changes: paths
                .iter()
                .map(|p| Change {
                    path: (*p).into(),
                    backup: None,
                    after: None,
                })
                .collect(),
            undone,
        };
        let history = [
            op(&["Skins/A"], false),
            op(&["Skins/B"], false),
            op(&["skins/a"], true),
            op(&["Skins/B", "Skins/C"], false),
        ];
        // A: the newer change to it was undone. B: changed again later. Newest: free.
        assert_eq!(undo_blocked(&history), [false, true, false, false]);
    }

    use super::*;
    #[cfg(windows)]
    #[test]
    fn history_write_failure_rolls_back_installed_skin() {
        let (_dir, library, path) = fixture();
        let cancel = AtomicBool::new(false);
        library
            .manage(
                std::slice::from_ref(&path),
                "duplicate",
                "First",
                &cancel,
                &|_, _| {},
            )
            .unwrap();
        let history_file = library.root.join(".osu-skin-mixer-backups/history.json");
        let original_permissions = history_file.metadata().unwrap().permissions();
        let mut readonly = original_permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&history_file, readonly).unwrap();
        let result = library.manage(
            std::slice::from_ref(&path),
            "duplicate",
            "Second",
            &cancel,
            &|_, _| {},
        );
        fs::set_permissions(&history_file, original_permissions).unwrap();
        assert!(result.is_err());
        assert!(!library.root.join("Skins/Second").exists());
        assert_eq!(fs::read(path.join("cursor.png")).unwrap(), b"original");
        assert_eq!(library.history().unwrap().len(), 1);
    }
    #[test]
    fn cached_scan_matches_full_scan_and_sees_new_skins() {
        let (_dir, library, path) = fixture();
        let cancel = AtomicBool::new(false);
        let (skins, warnings, cache) = library.scan_cached(None, &cancel).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(skins.len(), 1);
        let (again, _, _) = library.scan_cached(Some(&cache), &cancel).unwrap();
        assert_eq!(again[0].name, skins[0].name);
        assert_eq!(again[0].elements, skins[0].elements);
        assert_eq!(again[0].author, "test");
        let copy = library.root.join("Skins/Second");
        copy_tree(&path, &copy, &cancel).unwrap();
        let file = library.root.join("cache.json");
        cache.save(&file).unwrap();
        let loaded = ScanCache::load(&file, &library.root).unwrap();
        let (updated, _, _) = library.scan_cached(Some(&loaded), &cancel).unwrap();
        assert_eq!(updated.len(), 2);
        assert!(ScanCache::load(&file, Path::new("/elsewhere")).is_none());
    }
    pub fn fixture() -> (tempfile::TempDir, Library, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Skins/Original");
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("skin.ini"),
            "[General]\nName: Original\nAuthor: test",
        )
        .unwrap();
        fs::write(path.join("cursor.png"), b"original").unwrap();
        let library = Library::new(dir.path()).unwrap();
        (dir, library, path)
    }
    #[test]
    fn names_and_paths_cannot_escape_library() {
        for name in [
            "", "../foo", "foo/bar", "NUL", "CON.txt", "bad.", "a:b", " ", "LPT1",
        ] {
            assert!(validate_name(name).is_err(), "{name}");
        }
        for path in ["../a", "C:/a", "/a", "x/../../a", "\\\\host\\a"] {
            assert!(relative_path(path).is_err(), "{path}");
        }
        assert!(relative_path("folder\\note").is_ok());
    }
    #[test]
    fn deletion_persists_and_undo_refuses_external_changes() {
        let (_dir, library, path) = fixture();
        let op = library
            .manage(
                std::slice::from_ref(&path),
                "delete",
                "",
                &AtomicBool::new(false),
                &|_, _| {},
            )
            .unwrap();
        assert!(!path.exists());
        let library = Library::new(&library.root).unwrap();
        library.undo(&op.id).unwrap();
        assert_eq!(fs::read(path.join("cursor.png")).unwrap(), b"original");
        let op = library
            .manage(
                &[path],
                "duplicate",
                "Copy",
                &AtomicBool::new(false),
                &|_, _| {},
            )
            .unwrap();
        fs::write(library.root.join("Skins/Copy/cursor.png"), b"external").unwrap();
        assert!(library.undo(&op.id).is_err());
    }
    #[test]
    fn hide_rename_and_export_keep_files() {
        let (_dir, library, path) = fixture();
        let op = library
            .manage(
                std::slice::from_ref(&path),
                "hide",
                "",
                &AtomicBool::new(false),
                &|_, _| {},
            )
            .unwrap();
        assert!(
            library
                .root
                .join("HiddenSkins/Original/cursor.png")
                .exists()
        );
        library.undo(&op.id).unwrap();
        let out = tempfile::tempdir().unwrap();
        let archive = out.path().join("test.osk");
        library
            .export(&path, &archive, &AtomicBool::new(false))
            .unwrap();
        let mut zip = zip::ZipArchive::new(fs::File::open(archive).unwrap()).unwrap();
        let mut bytes = vec![];
        zip.by_name("cursor.png")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        assert_eq!(bytes, b"original");
    }
}
