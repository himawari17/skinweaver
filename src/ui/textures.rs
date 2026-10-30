//! Background loading of skin element images.
//!
//! The UI thread never touches the disk or decodes images: it asks for an element,
//! gets `Loading` on the first frames and the finished texture a moment later.
//! Workers take the newest request first, so whatever is on screen right now wins.

use eframe::egui;
use skinweaver::{
    library::walk,
    modify::{decode_image, default_bytes},
};
use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
};

/// Where an element comes from: a skin folder or the bundled default skin.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ElementKey {
    pub folder: Option<PathBuf>,
    pub stem: String,
    pub animated: bool,
    /// Longest side of the decoded texture in pixels.
    pub max: u32,
}

pub struct Element {
    pub frames: Vec<egui::TextureHandle>,
    /// Size of the original file in pixels (first frame).
    pub size: [u32; 2],
    /// Size osu! draws it at: `@2x` files count half.
    pub logical: egui::Vec2,
    /// The skin has no such file; the image is the default skin's.
    pub fallback: bool,
    /// File name shown to the user, e.g. `hitcircle@2x.png`.
    pub file: String,
    /// Frames the element has on disk; with reduced motion only the first is decoded.
    pub frame_count: usize,
    /// Pixel memory of all decoded frames, counted against the cache budget.
    pub bytes: usize,
}

#[derive(Clone)]
pub enum State {
    Loading,
    Ready(Arc<Element>),
    /// Neither the skin nor the default skin has this element.
    Missing,
}

struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
    /// One index per skin folder. `OnceLock` makes threads that ask for the same folder
    /// wait for a single walk instead of each walking it again; other folders go on in parallel.
    folders: Mutex<HashMap<PathBuf, Arc<OnceLock<FolderIndex>>>>,
    /// Decode only the first frame of animations ("Reduce motion").
    still: AtomicBool,
}

#[derive(Default)]
struct Queue {
    /// Element images, newest request last (and served first).
    loads: VecDeque<ElementKey>,
    /// Folders whose file list someone asked about; served after visible images.
    indexes: VecDeque<PathBuf>,
}

/// Lower-case relative path -> real path, built once per skin folder.
pub struct FolderIndex(HashMap<String, PathBuf>);

impl FolderIndex {
    fn build(folder: &Path) -> Self {
        let mut map = HashMap::new();
        let _ = walk(folder, &mut |path, _| {
            if let Ok(relative) = path.strip_prefix(folder) {
                let key = relative.to_string_lossy().replace('\\', "/").to_lowercase();
                map.insert(key, path);
            }
        });
        Self(map)
    }
    fn get(&self, name: &str) -> Option<&PathBuf> {
        self.0.get(&name.to_lowercase())
    }
    /// Whether the folder has its own file for an image element (any frame, any suffix).
    pub fn has_image(&self, stem: &str, animated: bool) -> bool {
        !frame_names(&|n| self.get(n).map(|_| String::new()), stem, animated).is_empty()
    }
    /// Whether the folder has its own file for a sound (no default fallback).
    pub fn has_sound(&self, stem: &str) -> bool {
        sound_candidates(stem).iter().any(|candidate| {
            SOUND_EXTENSIONS
                .iter()
                .any(|ext| self.get(&format!("{candidate}.{ext}")).is_some())
        })
    }
}

pub struct Textures {
    shared: Arc<Shared>,
    results: Receiver<(ElementKey, State)>,
    cache: HashMap<ElementKey, (State, u64)>,
    /// Pixel bytes of every ready element in `cache`.
    bytes: usize,
    frame: u64,
}

const IMAGE_SUFFIXES: [&str; 5] = ["@2x.png", ".png", "@2x.jpg", ".jpg", ".jpeg"];
const MAX_FRAMES: usize = 60;
/// Entries and pixel memory kept for elements that are off screen.
const MAX_CACHE: usize = 3000;
const MAX_BYTES: usize = 384 * 1024 * 1024;
/// A request nobody looked at for this many frames is dropped before it is loaded.
const STALE_FRAMES: u64 = 90;

impl Textures {
    pub fn new(ctx: &egui::Context) -> Self {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
            folders: Mutex::new(HashMap::new()),
            still: AtomicBool::new(false),
        });
        let (tx, rx) = mpsc::channel();
        let workers = std::thread::available_parallelism()
            .map_or(2, |n| n.get())
            .clamp(2, 4);
        for i in 0..workers {
            let shared = shared.clone();
            let tx: Sender<_> = tx.clone();
            let ctx = ctx.clone();
            std::thread::Builder::new()
                .name(format!("textures-{i}"))
                .spawn(move || worker(&shared, &tx, &ctx))
                .expect("texture worker");
        }
        Self {
            shared,
            results: rx,
            cache: HashMap::new(),
            bytes: 0,
            frame: 0,
        }
    }

    /// With reduced motion animations load as their first frame only.
    pub fn set_still(&mut self, still: bool) {
        if self.shared.still.swap(still, Ordering::Relaxed) != still && !still {
            // Bring the other frames back for elements loaded while motion was reduced.
            self.cache
                .retain(|key, (state, _)| !key.animated || matches!(state, State::Loading));
            self.recount();
        }
    }

    fn recount(&mut self) {
        self.bytes = self
            .cache
            .values()
            .map(|(state, _)| match state {
                State::Ready(e) => e.bytes,
                _ => 0,
            })
            .sum();
    }

    /// Call once per frame before drawing.
    pub fn begin_frame(&mut self) {
        self.frame += 1;
        while let Ok((key, state)) = self.results.try_recv() {
            if let Some(entry) = self.cache.get_mut(&key) {
                if let State::Ready(e) = &state {
                    self.bytes += e.bytes;
                }
                entry.0 = state;
            }
        }
        // Requests for tiles that scrolled away (or a panel that closed) are dropped before
        // a worker spends time on them.
        if self.frame.is_multiple_of(30) {
            let frame = self.frame;
            let before = self.cache.len();
            self.cache.retain(|_, (state, used)| {
                !matches!(state, State::Loading) || frame - *used < STALE_FRAMES
            });
            if self.cache.len() != before {
                let cache = &self.cache;
                self.shared
                    .queue
                    .lock()
                    .unwrap()
                    .loads
                    .retain(|key| cache.contains_key(key));
            }
        }
        if self.cache.len() > MAX_CACHE || self.bytes > MAX_BYTES {
            self.evict();
        }
    }

    /// Drops the least recently drawn elements until both limits have room again.
    /// Whatever was drawn in the last two frames stays.
    fn evict(&mut self) {
        let mut ages: Vec<(u64, usize, ElementKey)> = self
            .cache
            .iter()
            .filter(|(_, (state, used))| !matches!(state, State::Loading) && *used + 2 < self.frame)
            .map(|(key, (state, used))| {
                let bytes = match state {
                    State::Ready(e) => e.bytes,
                    _ => 0,
                };
                (*used, bytes, key.clone())
            })
            .collect();
        ages.sort_unstable_by_key(|(used, ..)| *used);
        let (count_goal, byte_goal) = (MAX_CACHE * 3 / 4, MAX_BYTES * 3 / 4);
        for (_, bytes, key) in ages {
            if self.cache.len() <= count_goal && self.bytes <= byte_goal {
                break;
            }
            self.cache.remove(&key);
            self.bytes = self.bytes.saturating_sub(bytes);
        }
    }

    pub fn get(&mut self, key: &ElementKey) -> State {
        if let Some((state, used)) = self.cache.get_mut(key) {
            *used = self.frame;
            return state.clone();
        }
        self.cache.insert(key.clone(), (State::Loading, self.frame));
        self.shared
            .queue
            .lock()
            .unwrap()
            .loads
            .push_back(key.clone());
        self.shared.wake.notify_one();
        State::Loading
    }

    /// The file list of a skin folder if it is already built; otherwise asks a worker to
    /// build it and returns `None`. Never touches the disk on the calling thread.
    pub fn index(&self, folder: &Path) -> Option<Arc<OnceLock<FolderIndex>>> {
        let mut folders = self.shared.folders.lock().unwrap();
        if let Some(index) = folders.get(folder) {
            return index.get().is_some().then(|| index.clone());
        }
        folders.insert(folder.to_path_buf(), Arc::new(OnceLock::new()));
        drop(folders);
        self.shared
            .queue
            .lock()
            .unwrap()
            .indexes
            .push_back(folder.to_path_buf());
        self.shared.wake.notify_one();
        None
    }

    /// Forget everything loaded from one skin folder (after it changed on disk).
    pub fn invalidate(&mut self, folder: &Path) {
        self.cache.retain(|key, (state, _)| {
            key.folder.as_deref() != Some(folder) || matches!(state, State::Loading)
        });
        self.shared.folders.lock().unwrap().remove(folder);
        self.recount();
    }

    pub fn invalidate_all_skins(&mut self) {
        self.cache
            .retain(|key, (state, _)| key.folder.is_none() || matches!(state, State::Loading));
        self.shared.folders.lock().unwrap().clear();
        self.recount();
    }

    /// Cached entries and the pixel memory of the ready ones.
    pub fn stats(&self) -> (usize, usize) {
        (self.cache.len(), self.bytes)
    }

    /// Images or folder lists still on their way.
    pub fn pending(&self) -> bool {
        let queue = self.shared.queue.lock().unwrap();
        !queue.loads.is_empty()
            || !queue.indexes.is_empty()
            || self
                .cache
                .values()
                .any(|(s, _)| matches!(s, State::Loading))
    }
}

enum Work {
    Load(ElementKey),
    Index(PathBuf),
}

fn worker(shared: &Shared, tx: &Sender<(ElementKey, State)>, ctx: &egui::Context) {
    loop {
        let work = {
            let mut queue = shared.queue.lock().unwrap();
            loop {
                // Newest first: rows that just scrolled into view load before old requests.
                if let Some(key) = queue.loads.pop_back() {
                    break Work::Load(key);
                }
                if let Some(folder) = queue.indexes.pop_front() {
                    break Work::Index(folder);
                }
                queue = shared.wake.wait(queue).unwrap();
            }
        };
        match work {
            Work::Load(key) => {
                let state = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    load(shared, &key, ctx)
                }))
                .unwrap_or(State::Missing);
                if tx.send((key, state)).is_err() {
                    return;
                }
            }
            Work::Index(folder) => {
                let _ = std::panic::catch_unwind(|| folder_index(shared, &folder));
            }
        }
        ctx.request_repaint();
    }
}

fn folder_index(shared: &Shared, folder: &Path) -> Arc<OnceLock<FolderIndex>> {
    let cell = shared
        .folders
        .lock()
        .unwrap()
        .entry(folder.to_path_buf())
        .or_default()
        .clone();
    cell.get_or_init(|| FolderIndex::build(folder));
    cell
}

/// Names of the files that make up an element, in frame order.
fn frame_names(exists: &dyn Fn(&str) -> Option<String>, stem: &str, animated: bool) -> Vec<String> {
    let single = |name: &str| {
        IMAGE_SUFFIXES
            .iter()
            .find_map(|s| exists(&format!("{name}{s}")))
    };
    if animated {
        for separator in ["-", ""] {
            let frames: Vec<_> = (0..MAX_FRAMES)
                .map_while(|i| single(&format!("{stem}{separator}{i}")))
                .collect();
            if !frames.is_empty() {
                return frames;
            }
        }
    }
    single(stem).into_iter().collect()
}

/// Decodes one frame and turns it into a texture no larger than `max` on its long side.
fn texture(
    ctx: &egui::Context,
    key: &ElementKey,
    name: &str,
    bytes: &[u8],
) -> Option<(egui::TextureHandle, [u32; 2], usize)> {
    let image = decode_image(bytes).ok()?;
    let size = [image.width(), image.height()];
    let longest = image.width().max(image.height());
    let image = if longest > key.max {
        let scale = key.max as f32 / longest as f32;
        image::imageops::resize(
            &image,
            ((image.width() as f32 * scale).round() as u32).max(1),
            ((image.height() as f32 * scale).round() as u32).max(1),
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };
    let pixels = image.as_raw().len();
    let color = egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    );
    let handle = ctx.load_texture(
        format!("{:?}/{name}/{}", key.folder, key.max),
        color,
        egui::TextureOptions::LINEAR,
    );
    Some((handle, size, pixels))
}

/// Where a frame's bytes come from.
enum From {
    Disk(PathBuf),
    Default(&'static [u8]),
}

struct Decoded {
    frames: Vec<egui::TextureHandle>,
    size: [u32; 2],
    bytes: usize,
    file: String,
}

/// Reads and decodes frames one at a time, so a 60-frame animation never holds all of its
/// compressed files in memory at once.
fn decode_frames(
    ctx: &egui::Context,
    key: &ElementKey,
    files: &[(String, From)],
) -> Option<Decoded> {
    let mut out: Option<Decoded> = None;
    for (name, from) in files {
        let data;
        let raw: &[u8] = match from {
            From::Default(b) => b,
            From::Disk(path) => {
                match std::fs::metadata(path) {
                    Ok(m) if m.len() <= 64 * 1024 * 1024 => {}
                    _ => continue,
                }
                let Ok(read) = std::fs::read(path) else {
                    continue;
                };
                data = read;
                &data
            }
        };
        let Some((handle, size, pixels)) = texture(ctx, key, name, raw) else {
            continue;
        };
        let out = out.get_or_insert_with(|| Decoded {
            frames: vec![],
            size,
            bytes: 0,
            file: name.clone(),
        });
        out.bytes += pixels;
        out.frames.push(handle);
    }
    out
}

fn load(shared: &Shared, key: &ElementKey, ctx: &egui::Context) -> State {
    let still = shared.still.load(Ordering::Relaxed);
    let limit = |files: &mut Vec<(String, From)>| {
        let count = files.len();
        if key.max <= 96 || still {
            files.truncate(1);
        }
        count
    };
    let mut decoded = None;
    let mut frame_count = 0;
    if let Some(folder) = &key.folder {
        let index = folder_index(shared, folder);
        let index = index.get().expect("built by folder_index");
        let mut files: Vec<(String, From)> = frame_names(
            &|n| index.get(n).map(|_| n.to_owned()),
            &key.stem,
            key.animated,
        )
        .into_iter()
        .filter_map(|name| {
            let path = index.get(&name)?.clone();
            let shown = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(name);
            Some((shown, From::Disk(path)))
        })
        .collect();
        frame_count = limit(&mut files);
        decoded = decode_frames(ctx, key, &files);
    }
    let fallback = key.folder.is_some() && decoded.is_none();
    if decoded.is_none() {
        // Default skin files have no folders: "fonts/score-0" falls back to "score-0".
        let stem = key.stem.rsplit('/').next().unwrap_or(&key.stem);
        let mut files: Vec<(String, From)> = frame_names(
            &|n| default_bytes(n).map(|_| n.to_owned()),
            stem,
            key.animated,
        )
        .into_iter()
        .filter_map(|n| default_bytes(&n).map(|b| (n, From::Default(b))))
        .collect();
        frame_count = limit(&mut files);
        decoded = decode_frames(ctx, key, &files);
    }
    let Some(Decoded {
        frames,
        size,
        bytes,
        file,
    }) = decoded
    else {
        return State::Missing;
    };
    let hd = if file.to_lowercase().contains("@2x") {
        2.0
    } else {
        1.0
    };
    State::Ready(Arc::new(Element {
        frames,
        size,
        logical: egui::vec2(size[0] as f32, size[1] as f32) / hd,
        fallback,
        file,
        frame_count: frame_count.max(1),
        bytes,
    }))
}

/// Whether the bundled default skin has an image element.
pub fn default_has_image(stem: &str, animated: bool) -> bool {
    let stem = stem.rsplit('/').next().unwrap_or(stem);
    !frame_names(&|n| default_bytes(n).map(|_| String::new()), stem, animated).is_empty()
}

/// File stems a sound element may use: `menu-play-` (from `menu-play-*`) covers
/// `menu-play-click`, `menu-play-hover` and the like.
fn sound_candidates(stem: &str) -> Vec<String> {
    match stem.strip_suffix('-') {
        Some(base) => ["click", "hover", "confirm", "1", "start"]
            .iter()
            .map(|s| format!("{base}-{s}"))
            .collect(),
        None => vec![stem.to_owned()],
    }
}

const SOUND_EXTENSIONS: [&str; 3] = ["wav", "ogg", "mp3"];

/// Finds an audio file of a skin (or the default skin) without blocking on large folders.
pub fn audio_bytes(folder: Option<&Path>, stem: &str) -> Option<(Vec<u8>, String, bool)> {
    let candidates = sound_candidates(stem);
    if let Some(folder) = folder {
        for candidate in &candidates {
            for ext in SOUND_EXTENSIONS {
                let path = folder.join(format!("{candidate}.{ext}"));
                if path.is_file()
                    && skinweaver::library::reject_link(&path).is_ok()
                    && path.metadata().is_ok_and(|m| m.len() < 64 * 1024 * 1024)
                    && let Ok(bytes) = std::fs::read(&path)
                {
                    return Some((bytes, format!("{candidate}.{ext}"), false));
                }
            }
        }
    }
    candidates.iter().find_map(|candidate| {
        let candidate = candidate.rsplit('/').next().unwrap_or(candidate);
        SOUND_EXTENSIONS.into_iter().find_map(|ext| {
            default_bytes(&format!("{candidate}.{ext}"))
                .map(|b| (b.to_vec(), format!("{candidate}.{ext}"), folder.is_some()))
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_prefer_dash_numbering_and_hd() {
        let files = [
            "menu-back-0@2x.png",
            "menu-back-1.png",
            "menu-back-2@2x.png",
            "cursor.png",
        ];
        let exists = |n: &str| files.contains(&n).then(|| n.to_owned());
        assert_eq!(
            frame_names(&exists, "menu-back", true),
            [
                "menu-back-0@2x.png",
                "menu-back-1.png",
                "menu-back-2@2x.png"
            ]
        );
        assert_eq!(frame_names(&exists, "cursor", true), ["cursor.png"]);
        assert!(frame_names(&exists, "hitcircle", false).is_empty());
    }
    #[test]
    fn wildcard_sounds_find_their_files() {
        assert_eq!(sound_candidates("menuhit"), ["menuhit"]);
        assert_eq!(sound_candidates("menu-play-")[0], "menu-play-click");
        let (_, name, _) = audio_bytes(None, "menu-play-").expect("default sound");
        assert_eq!(name, "menu-play-click.wav");
        let (_, name, _) = audio_bytes(None, "menu-edit-").expect("default sound");
        assert_eq!(name, "menu-edit-hover.mp3");
    }
}
