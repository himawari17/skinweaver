mod editor;
#[allow(dead_code)]
mod icons;
mod motion;
mod pages;
mod preview;
mod scenes;
mod stage_view;
mod stages;
mod textures;
mod theme;

use anyhow::Result;
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, Stroke, Vec2, pos2, vec2};
use skinweaver::{
    catalog::{OptionNode, catalog},
    library::{Library, Operation, ScanCache, Skin, library_stamp},
    mixer::Source,
    modify::Modifications,
    settings::{Language, Settings},
};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
use textures::Textures;
use theme::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Mix,
    Library,
    Modify,
    History,
    Settings,
}

enum Outcome {
    Created(String),
    Changed(Vec<PathBuf>),
    Exported(PathBuf),
    Undone,
}
enum Event {
    Progress(f32, String),
    Done(Result<Outcome, String>),
}
struct Job {
    events: Receiver<Event>,
    cancel: Arc<AtomicBool>,
    progress: f32,
    status: String,
}

struct ScanResult {
    skins: Vec<Skin>,
    warnings: Vec<String>,
    history: Result<Vec<Operation>, String>,
    cache: ScanCache,
}
struct ScanJob {
    result: Receiver<Result<ScanResult, String>>,
    cancel: Arc<AtomicBool>,
}

enum Dialog {
    Name { kind: &'static str, value: String },
    Delete,
}

struct Notice {
    tone: Tone,
    title: String,
    text: String,
    shown: Instant,
    /// Set for a freshly created skin: offer undo and the folder right in the notice.
    created: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum PreviewTab {
    Element,
    Scene,
    /// Stage mode: big tiles of the focused element from every skin.
    Variants,
}
#[derive(Clone, Copy, PartialEq)]
enum CompareMode {
    Side,
    Wipe,
}

/// What the library list depends on besides the skins themselves.
type VisibleKey = (String, Filter, Sort);

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Visible,
    Hidden,
}
#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Name,
    Newest,
    Size,
}

/// Per-page state of the element tree (the mixer and the batch editor keep separate choices).
#[derive(Default)]
pub(crate) struct Editor {
    choices: HashMap<String, Source>,
    /// What an element resolved to before its last change: "compare with the previous".
    previous: HashMap<String, Source>,
    focus: Option<String>,
    query: String,
    expanded: HashSet<String>,
    /// Manual comparison source; `None` compares with the previous choice.
    compare: Option<Source>,
    /// Several elements picked with Ctrl/Shift; a source chosen then goes to all of them.
    picked: Vec<String>,
    /// Where a Shift range starts.
    anchor: Option<String>,
    /// Ids of the element rows in drawing order: this frame's and the last one's.
    row_order: Vec<String>,
    row_order_prev: Vec<String>,
}

pub struct App {
    settings: Settings,
    data: PathBuf,
    _lock: fs::File,
    _demo: Option<tempfile::TempDir>,
    demo: bool,
    library: Option<Library>,
    skins: Vec<Skin>,
    skin_index: HashMap<PathBuf, usize>,
    /// Catalog code per skin, like a circle's space number: first letter + number in that block.
    skin_codes: Vec<String>,
    history: Vec<Operation>,
    /// Per operation: a newer, not undone change touches one of its files (undo is blocked).
    /// Worked out once when the history is read, not on every frame.
    history_blocked: Vec<bool>,
    /// Bumped whenever the skin list is replaced; part of every memo key that reads skins.
    skins_generation: u64,
    scene_memo: std::cell::RefCell<Option<preview::SceneMemo>>,
    profile: Option<Profile>,
    /// (skin folder, element) -> the skin has its own file for it; see `skin_has_file`.
    own_files: HashMap<(PathBuf, String), bool>,
    /// Rows of the library list for the current search, filter and sort.
    visible_cache: std::cell::RefCell<Option<(VisibleKey, std::rc::Rc<Vec<usize>>)>>,
    page: Page,
    nodes: Arc<Vec<OptionNode>>,
    mix: Editor,
    /// An existing skin the mix starts from (shown, changed, saved as a copy or into itself).
    mix_base: Option<PathBuf>,
    /// After "save into the base skin" succeeds, the picks are part of it and are cleared.
    clear_mix_after_job: bool,
    modify: Editor,
    query: String,
    filter: Filter,
    sort: Sort,
    selected: BTreeSet<PathBuf>,
    focus_skin: Option<PathBuf>,
    focus_element: Option<usize>,
    mix_name: String,
    created_name: Option<String>,
    /// When the last skin was created (egui time), for the specimen's "ready" moment.
    created_at: Option<f64>,
    flags: Modifications,
    edit_colors: bool,
    edit_cursor: bool,
    colors: Vec<[u8; 3]>,
    cursor_color: [u8; 3],
    folder_input: String,
    notice: Option<Notice>,
    job: Option<Job>,
    scan: Option<ScanJob>,
    scan_cache: Option<ScanCache>,
    library_stamp: u128,
    last_stamp_check: Instant,
    rescan_queued: bool,
    last_warnings: String,
    dialog: Option<Dialog>,
    textures: Textures,
    audio: Option<rodio::MixerDeviceSink>,
    player: Option<Arc<rodio::Player>>,
    /// Decoding errors from sound threads, shown on the next frame.
    audio_errors: (mpsc::Sender<String>, mpsc::Receiver<String>),
    preview_tab: PreviewTab,
    compare_mode: CompareMode,
    wipe: f32,
    group_after: bool,
    sound_sample: usize,
    closing: bool,
    /// "By stage" mode of the mixer (the element list is the other mode).
    stage_view: bool,
    stage: usize,
    stage_open: HashSet<usize>,
    /// Folded sound rows of the stage tree that are unfolded: (stage, sound group).
    stage_sounds_open: HashSet<(usize, usize)>,
    stage_index: Arc<stages::Index>,
    /// Hitsound set shown in the gameplay stage: normal, soft or drum.
    hit_set: usize,
    variant_columns: usize,
    variant_query: String,
    variant_own_only: bool,
    /// Skins chosen most recently, newest first: offered at the top of every source list.
    recent: Vec<PathBuf>,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        data: PathBuf,
        settings: Settings,
        lock: fs::File,
        demo: Option<tempfile::TempDir>,
        initial_error: Option<String>,
    ) -> Self {
        theme::install(&cc.egui_ctx, settings.reduce_motion, settings.ui_scale);
        pages::session_start();
        egui_extras::install_image_loaders(&cc.egui_ctx);
        let folder_input = display_path(&settings.osu_folder);
        let mix_name = settings.language.text("Мой микс", "My mix").into();
        let library = if settings.osu_folder.as_os_str().is_empty() {
            None
        } else {
            Library::new(&settings.osu_folder).ok()
        };
        let nodes = Arc::new(catalog());
        let stage_index = Arc::new(stages::Index::build(&nodes));
        let mut app = Self {
            settings,
            data,
            _lock: lock,
            demo: demo.is_some(),
            _demo: demo,
            library,
            skins: vec![],
            skin_index: HashMap::new(),
            skin_codes: vec![],
            history: vec![],
            history_blocked: vec![],
            skins_generation: 0,
            scene_memo: Default::default(),
            profile: Profile::new(),
            own_files: HashMap::new(),
            visible_cache: Default::default(),
            page: Page::Mix,
            nodes,
            mix: Editor::default(),
            mix_base: None,
            clear_mix_after_job: false,
            modify: Editor::default(),
            query: String::new(),
            filter: Filter::All,
            sort: Sort::Name,
            selected: BTreeSet::new(),
            focus_skin: None,
            focus_element: None,
            mix_name,
            created_name: None,
            created_at: None,
            flags: Modifications::default(),
            edit_colors: false,
            edit_cursor: false,
            colors: vec![[255, 102, 170], [76, 201, 194], [134, 187, 255]],
            cursor_color: [255, 102, 170],
            folder_input,
            notice: None,
            job: None,
            scan: None,
            scan_cache: None,
            library_stamp: 0,
            last_stamp_check: Instant::now(),
            rescan_queued: false,
            last_warnings: String::new(),
            dialog: None,
            textures: Textures::new(&cc.egui_ctx),
            audio: None,
            player: None,
            audio_errors: mpsc::channel(),
            preview_tab: PreviewTab::Scene,
            compare_mode: CompareMode::Side,
            wipe: 0.5,
            group_after: true,
            sound_sample: 0,
            closing: false,
            stage_view: true,
            stage: 0,
            stage_open: HashSet::from([0]),
            stage_sounds_open: HashSet::new(),
            stage_index,
            hit_set: 0,
            variant_columns: 2,
            variant_query: String::new(),
            variant_own_only: true,
            recent: vec![],
        };
        if let Some(error) = initial_error {
            app.error(error);
        }
        app.open_library(&cc.egui_ctx);
        app
    }

    pub(crate) fn t<'a>(&self, ru: &'a str, en: &'a str) -> &'a str {
        self.settings.language.text(ru, en)
    }
    fn ru(&self) -> bool {
        self.settings.language == Language::Ru
    }

    fn report(&mut self, result: Result<()>) {
        if let Err(error) = result {
            self.error(format!("{error:#}"));
        }
    }
    fn error(&mut self, message: String) {
        if let Ok(mut log) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.data.join("app.log"))
        {
            let _ = writeln!(log, "{message}");
        }
        self.notice = Some(Notice {
            tone: Tone::Danger,
            title: self.t("Что-то пошло не так", "Something went wrong").into(),
            text: message,
            shown: Instant::now(),
            created: None,
        });
    }
    fn success(&mut self, title: impl Into<String>, text: impl Into<String>) {
        self.notice = Some(Notice {
            tone: Tone::Success,
            title: title.into(),
            text: text.into(),
            shown: Instant::now(),
            created: None,
        });
    }
    fn save_settings(&mut self) {
        let result = self.settings.save(&self.data);
        self.report(result);
    }
    fn cache_file(&self) -> PathBuf {
        self.data.join("library-cache.json")
    }
    pub(crate) fn skin(&self, path: &Path) -> Option<&Skin> {
        self.skin_index.get(path).and_then(|&i| self.skins.get(i))
    }
    fn set_history(&mut self, history: Vec<Operation>) {
        let blocked = skinweaver::library::undo_blocked(&history);
        self.history = history;
        self.history_blocked = blocked;
    }

    fn set_skins(&mut self, skins: Vec<Skin>) {
        self.visible_cache.take();
        self.skins_generation += 1;
        self.own_files.clear();
        // Element textures are only dropped for skins that changed on disk.
        for skin in &skins {
            if let Some(old) = self.skin(&skin.path)
                && (old.modified != skin.modified
                    || old.bytes != skin.bytes
                    || old.elements != skin.elements)
            {
                let path = skin.path.clone();
                self.textures.invalidate(&path);
            }
        }
        self.skins = skins;
        let mut blocks: HashMap<char, usize> = HashMap::new();
        self.skin_codes = self
            .skins
            .iter()
            .map(|skin| {
                let first = skin
                    .name
                    .chars()
                    .find(|c| c.is_alphanumeric())
                    .map(|c| {
                        if c.is_ascii_digit() {
                            return '#';
                        }
                        // Cyrillic letters that look Latin share one block, so codes never repeat visually.
                        match c.to_uppercase().next().unwrap_or(c) {
                            'А' => 'A',
                            'В' => 'B',
                            'Е' => 'E',
                            'К' => 'K',
                            'М' => 'M',
                            'Н' => 'H',
                            'О' => 'O',
                            'Р' => 'P',
                            'С' => 'C',
                            'Т' => 'T',
                            'Х' => 'X',
                            other => other,
                        }
                    })
                    .unwrap_or('#');
                let n = blocks.entry(first).or_insert(0);
                *n += 1;
                format!("{first}{:02}", n)
            })
            .collect();
        self.skin_index = self
            .skins
            .iter()
            .enumerate()
            .map(|(i, s)| (s.path.clone(), i))
            .collect();
        self.selected.retain(|p| self.skin_index.contains_key(p));
        if self
            .mix_base
            .as_ref()
            .is_some_and(|p| !self.skin_index.contains_key(p))
        {
            self.mix_base = None;
        }
        if self
            .focus_skin
            .as_ref()
            .is_some_and(|p| !self.skin_index.contains_key(p))
        {
            self.focus_skin = None;
        }
    }

    /// Shows the cached library at once and refreshes it in the background.
    fn open_library(&mut self, ctx: &egui::Context) {
        let Some(library) = self.library.clone() else {
            return;
        };
        self.scan_cache = ScanCache::load(&self.cache_file(), &library.root);
        if let Some(cache) = &self.scan_cache {
            let skins = cache.skins();
            self.set_skins(skins);
        }
        if let Ok(history) = library.history() {
            self.set_history(history);
        }
        self.library_stamp = library_stamp(&library.root);
        self.rescan(ctx, false);
    }

    /// Background scan; never blocks the interface. `full` ignores the cache.
    fn rescan(&mut self, ctx: &egui::Context, full: bool) {
        let Some(library) = self.library.clone() else {
            return;
        };
        if self.scan.is_some() {
            self.rescan_queued = true;
            return;
        }
        let cache = if full { None } else { self.scan_cache.clone() };
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let repaint = ctx.clone();
        let cache_file = self.cache_file();
        std::thread::Builder::new()
            .name("library-scan".into())
            .spawn(move || {
                let result = library
                    .scan_cached(cache.as_ref(), &worker_cancel)
                    .map(|(skins, warnings, cache)| {
                        // Written here, not by the interface: the file is synced to disk.
                        let _ = cache.save(&cache_file);
                        (skins, warnings, cache)
                    })
                    .map(|(skins, warnings, cache)| ScanResult {
                        skins,
                        warnings,
                        history: library.history().map_err(|e| format!("{e:#}")),
                        cache,
                    })
                    .map_err(|e| format!("{e:#}"));
                let _ = tx.send(result);
                repaint.request_repaint();
            })
            .expect("scan thread");
        self.scan = Some(ScanJob { result: rx, cancel });
    }

    fn spawn(
        &mut self,
        ctx: &egui::Context,
        task: impl FnOnce(Arc<AtomicBool>, mpsc::Sender<Event>) -> Result<Outcome> + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                task(worker_cancel, tx.clone())
            }));
            let result = match result {
                Ok(result) => result.map_err(|e| format!("{e:#}")),
                Err(_) => Err(
                    "Фоновая операция упала. Оригиналы в папке восстановления / Background task failed; originals are in the recovery folder"
                        .into(),
                ),
            };
            let _ = tx.send(Event::Done(result));
            repaint.request_repaint();
        });
        self.job = Some(Job {
            events: rx,
            cancel,
            progress: 0.0,
            status: self.t("Готовлю…", "Preparing…").into(),
        });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        if let Ok(error) = self.audio_errors.1.try_recv() {
            self.sound_error(error);
        }
        if let Some(scan) = &self.scan
            && let Ok(result) = scan.result.try_recv()
        {
            self.scan = None;
            match result {
                Ok(result) => {
                    self.set_skins(result.skins);
                    match result.history {
                        Ok(history) => self.set_history(history),
                        Err(error) => self.error(error),
                    }
                    self.scan_cache = Some(result.cache);
                    let warnings = result.warnings.join("\n");
                    if !result.warnings.is_empty() && warnings != self.last_warnings {
                        self.notice = Some(Notice {
                            tone: Tone::Info,
                            title: self
                                .t(
                                    "Некоторые скины не прочитались",
                                    "Some skins could not be read",
                                )
                                .into(),
                            text: warnings.clone(),
                            shown: Instant::now(),
                            created: None,
                        });
                    }
                    self.last_warnings = warnings;
                }
                Err(error) if !error.contains("Cancelled") => self.error(error),
                Err(_) => {}
            }
            if std::mem::take(&mut self.rescan_queued) {
                self.rescan(ctx, false);
            }
        }
        let mut done = None;
        if let Some(job) = &mut self.job {
            while let Ok(event) = job.events.try_recv() {
                match event {
                    Event::Progress(p, s) => {
                        job.progress = p;
                        job.status = s;
                    }
                    Event::Done(result) => {
                        done = Some(result);
                        break;
                    }
                }
            }
        }
        if let Some(result) = done {
            self.job = None;
            match result {
                Ok(Outcome::Created(name)) => {
                    self.success(
                        format!("{} «{name}» {}", self.t("Скин", "Skin"), self.t("готов", "is ready")),
                        self.t(
                            "Он уже в папке Skins — osu! увидит его при следующем выборе скина.",
                            "It is already in your Skins folder; osu! will list it next time you pick a skin.",
                        ),
                    );
                    if let Some(notice) = &mut self.notice {
                        notice.created = Some(name.clone());
                    }
                    // Suggest the next free name so the following mix needs no typing.
                    if let Some(library) = &self.library {
                        let base = name
                            .rsplit_once(' ')
                            .filter(|(_, n)| n.parse::<u32>().is_ok())
                            .map_or(name.as_str(), |(b, _)| b)
                            .to_owned();
                        if let Some(next) = (2..1000)
                            .map(|n| format!("{base} {n}"))
                            .find(|candidate| library.ensure_new_name(candidate).is_ok())
                        {
                            self.mix_name = next;
                        }
                    }
                    self.created_name = Some(name);
                    self.created_at = Some(ctx.input(|i| i.time));
                    self.rescan(ctx, false);
                }
                Ok(Outcome::Changed(paths)) => {
                    for path in &paths {
                        self.textures.invalidate(path);
                    }
                    if std::mem::take(&mut self.clear_mix_after_job) {
                        self.mix.choices.clear();
                        self.mix.previous.clear();
                    }
                    self.success(
                        self.t("Готово", "Done"),
                        self.t(
                            "Оригиналы сохранены — отменить можно в истории.",
                            "Originals are kept, so you can undo this in History.",
                        ),
                    );
                    self.rescan(ctx, false);
                }
                Ok(Outcome::Undone) => {
                    self.textures.invalidate_all_skins();
                    self.success(self.t("Изменение отменено", "Change undone"), "");
                    self.rescan(ctx, false);
                }
                Ok(Outcome::Exported(path)) => self.success(
                    self.t("Архив .osk сохранён", "The .osk archive is saved"),
                    display_path(&path),
                ),
                Err(error) => {
                    self.clear_mix_after_job = false;
                    self.error(error);
                }
            }
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(60));
        }
    }

    /// Cheap check (two folder timestamps) instead of re-reading every skin on a timer.
    fn watch_library(&mut self, ctx: &egui::Context) {
        if !self.settings.auto_refresh || self.last_stamp_check.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.last_stamp_check = Instant::now();
        let Some(library) = &self.library else { return };
        let stamp = library_stamp(&library.root);
        let focused = ctx.input(|i| i.focused);
        if stamp != self.library_stamp && focused {
            self.library_stamp = stamp;
            self.rescan(ctx, false);
        }
        ctx.request_repaint_after(Duration::from_secs(2));
    }

    fn set_folder(&mut self, ctx: &egui::Context) {
        match Library::new(Path::new(self.folder_input.trim())) {
            Ok(library) => {
                if let Some(scan) = self.scan.take() {
                    scan.cancel.store(true, Ordering::Relaxed);
                }
                self.settings.osu_folder = library.root.clone();
                self.folder_input = display_path(&library.root);
                self.library = Some(library);
                self.selected.clear();
                self.focus_skin = None;
                self.mix = Editor::default();
                self.mix_base = None;
                self.modify = Editor::default();
                self.set_skins(vec![]);
                self.textures.invalidate_all_skins();
                self.save_settings();
                self.open_library(ctx);
            }
            Err(error) => self.error(format!("{error:#}")),
        }
    }

    fn go(&mut self, page: Page) {
        self.page = page;
    }

    // ------------------------------------------------------------ shell

    fn nav(&mut self, ui: &mut egui::Ui) {
        let narrow = ui.ctx().content_rect().width() < 1180.0;
        let width = if narrow { NAV_NARROW } else { NAV_WIDE };
        egui::Panel::left("navigation")
            .exact_size(width)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(BG_000)
                    .inner_margin(egui::Margin::symmetric(8, 16)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(if narrow { 6.0 } else { 8.0 });
                    ui.add(
                        egui::Image::new(egui::include_image!("../../assets/icon.png"))
                            .fit_to_exact_size(Vec2::splat(30.0)),
                    );
                    if !narrow {
                        ui.vertical(|ui| {
                            ui.add_space(-2.0);
                            ui.label(
                                RichText::new("Skinweaver")
                                    .font(serif(22.0))
                                    .color(INK),
                            );
                            ui.add_space(-8.0);
                            ui.label(
                                RichText::new(self.t("для osu!", "for osu!"))
                                    .size(12.5)
                                    .color(INK_SUBTLE),
                            );
                        });
                    }
                });
                ui.add_space(20.0);
                // The current item's tint slides between items (kohu! NavPane).
                let slide = ui.painter().add(egui::Shape::Noop);
                let mut current_rect = None;
                let pages = [
                    (
                        Page::Mix,
                        self.t("Собрать скин", "Mix skins"),
                        icons::blend(),
                    ),
                    (
                        Page::Library,
                        self.t("Библиотека", "Library"),
                        icons::library_big(),
                    ),
                    (
                        Page::Modify,
                        self.t("Изменить скины", "Modify skins"),
                        icons::wand_sparkles(),
                    ),
                    (
                        Page::History,
                        self.t("История", "History"),
                        icons::history(),
                    ),
                ];
                for (page, label, icon) in pages {
                    let (clicked, rect) = self.nav_item(ui, page, label, icon, narrow);
                    if page == self.page {
                        current_rect = Some(rect);
                    }
                    if clicked {
                        self.go(page);
                    }
                }
                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    self.nav_footer(ui, narrow);
                    if self.library.is_some() {
                        // kohu!'s account block: the library sits under a hairline.
                        ui.add_space(10.0);
                        let r = ui.available_rect_before_wrap();
                        hairline(ui, r.bottom(), r.left() + 12.0..=r.right() - 12.0);
                        ui.add_space(10.0);
                    } else {
                        ui.add_space(8.0);
                    }
                    let label = self.t("Настройки", "Settings");
                    let (clicked, rect) =
                        self.nav_item(ui, Page::Settings, label, icons::settings(), narrow);
                    if self.page == Page::Settings {
                        current_rect = Some(rect);
                    }
                    if clicked {
                        self.go(Page::Settings);
                    }
                });
                if let Some(rect) = current_rect {
                    let y = ui.ctx().animate_value_with_time(
                        ui.make_persistent_id("nav_slide"),
                        rect.top(),
                        if motion::reduced(ui.ctx()) { 0.0 } else { 0.22 },
                    );
                    ui.painter().set(
                        slide,
                        egui::Shape::rect_filled(
                            egui::Rect::from_min_size(pos2(rect.left(), y), rect.size()),
                            RADIUS_SM,
                            ACCENT_SOFT,
                        ),
                    );
                }
            });
    }

    fn nav_item(
        &self,
        ui: &mut egui::Ui,
        page: Page,
        label: &str,
        icon: egui::ImageSource<'static>,
        narrow: bool,
    ) -> (bool, egui::Rect) {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
        let current = self.page == page;
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Tab, ui.is_enabled(), current, label)
        });
        // kohu! NavPane: current item on the plum tint (painted by `nav`, it slides) with a
        // pink icon, no side tick.
        let h = ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            response.hovered() && !current,
            ui.style().animation_time,
        );
        let fill = if response.is_pointer_button_down_on() && !current {
            BG_400
        } else {
            BG_300.gamma_multiply(h)
        };
        ui.painter().rect_filled(rect, RADIUS_SM, fill);
        let icon_rect = egui::Rect::from_min_size(
            pos2(
                if narrow {
                    rect.center().x - 10.0
                } else {
                    rect.min.x + 12.0
                },
                rect.center().y - 10.0,
            ),
            Vec2::splat(20.0),
        );
        egui::Image::new(icon)
            .tint(if current { ACCENT_TEXT } else { INK_MUTED })
            .paint_at(ui, icon_rect);
        if !narrow {
            text_at(
                ui,
                pos2(rect.min.x + 44.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                if current {
                    font_strong(14.0)
                } else {
                    font(14.0)
                },
                INK,
            );
        }
        focus_ring(ui, &response, rect);
        let response = if narrow {
            response.on_hover_text(label)
        } else {
            response
        };
        ui.add_space(2.0);
        (
            response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked(),
            rect,
        )
    }

    fn nav_footer(&mut self, ui: &mut egui::Ui, narrow: bool) {
        if self.library.is_none() {
            return;
        }
        let scanning = self.scan.is_some();
        ui.horizontal(|ui| {
            ui.add_space(if narrow { 14.0 } else { 12.0 });
            if scanning {
                spinner(ui, 16.0, self.settings.reduce_motion);
            } else {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 4.0, SUCCESS);
            }
            // The count runs up to the real number, like the price on the site.
            let shown = ui
                .ctx()
                .animate_value_with_time(
                    ui.make_persistent_id("skin_count"),
                    self.skins.len() as f32,
                    if motion::reduced(ui.ctx()) { 0.0 } else { 0.6 },
                )
                .round() as usize;
            if narrow {
                ui.label(RichText::new(shown.to_string()).font(condensed(18.0)));
            } else {
                let count = self.settings.language.count(
                    shown,
                    ["скин", "скина", "скинов"],
                    ["skin", "skins"],
                );
                ui.label(
                    RichText::new(if scanning && self.skins.is_empty() {
                        self.t("Читаю библиотеку…", "Reading library…").to_owned()
                    } else {
                        count
                    })
                    .color(INK)
                    .font(condensed(18.0)),
                );
            }
        })
        .response
        .on_hover_text(display_path(
            &self
                .library
                .as_ref()
                .map(|l| l.root.clone())
                .unwrap_or_default(),
        ));
        if self.demo {
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                let label = if narrow {
                    self.t("Демо", "Demo")
                } else {
                    self.t("Демо-библиотека", "Demo library")
                };
                badge(ui, label, true).on_hover_text(self.t(
                    "Временная библиотека с синтетическими скинами",
                    "A temporary library with synthetic skins",
                ));
            });
        }
    }

    fn job_bar(&mut self, ui: &mut egui::Ui) {
        let Some(job) = &self.job else { return };
        let (progress, status, cancel) = (job.progress, job.status.clone(), job.cancel.clone());
        let cancel_label = self.t("Остановить", "Stop");
        motion::glass(ui, BG_200, egui::Margin::symmetric(12, 10), |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                spinner(ui, 20.0, self.settings.reduce_motion);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&status).font(font_strong(13.0)));
                    ui.horizontal(|ui| {
                        progress_bar(ui, progress);
                    });
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let stoppable = progress < 0.9 && !cancel.load(Ordering::Relaxed);
                    if btn(Kind::Ghost, cancel_label)
                        .enabled(stoppable)
                        .show(ui)
                        .clicked()
                    {
                        cancel.store(true, Ordering::Relaxed);
                    }
                });
            });
        });
        ui.add_space(16.0);
    }

    fn notice_bar(&mut self, ui: &mut egui::Ui) {
        let Some(notice) = &self.notice else { return };
        let lifetime = if notice.created.is_some() { 30 } else { 8 };
        if notice.tone == Tone::Success && notice.shown.elapsed() > Duration::from_secs(lifetime) {
            self.notice = None;
            return;
        }
        if notice.tone == Tone::Success {
            ui.ctx().request_repaint_after(Duration::from_secs(1));
        }
        // History already lists the change; the notice would only repeat it there.
        if self.page == Page::History && notice.tone == Tone::Success {
            return;
        }
        let dismiss = self.t("Скрыть", "Dismiss");
        let created = notice.created.clone();
        let actions: Vec<&str> = if created.is_some() {
            vec![
                self.t("Отменить", "Undo"),
                self.t("Открыть папку", "Open folder"),
            ]
        } else {
            vec![]
        };
        let result = infobar(
            ui,
            notice.tone,
            &notice.title.clone(),
            &notice.text.clone(),
            dismiss,
            &actions,
        );
        if result.dismissed {
            self.notice = None;
        }
        if let (Some(action), Some(name)) = (result.action, created) {
            match action {
                0 => {
                    if let Some(library) = &self.library
                        && let Ok(history) = library.history()
                    {
                        self.set_history(history);
                    }
                    let operation = self
                        .history
                        .iter()
                        .rev()
                        .find(|op| op.kind == "mix" && !op.undone && op.label == name)
                        .map(|op| op.id.clone());
                    if let (Some(id), Some(library)) = (operation, self.library.clone()) {
                        self.notice = None;
                        self.spawn(ui.ctx(), move |_, _| {
                            library.undo(&id)?;
                            Ok(Outcome::Undone)
                        });
                    }
                }
                _ => {
                    if let Some(library) = &self.library {
                        let path = library.root.join("Skins").join(&name);
                        self.report(open::that(path).map_err(anyhow::Error::from));
                    }
                }
            }
        }
        ui.add_space(16.0);
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.dialog.take() else {
            return;
        };
        let mut keep = true;
        let frame = egui::Frame::new()
            .fill(BG_200)
            .stroke(Stroke::new(1.0, LINE_100))
            .corner_radius(RADIUS_MD)
            .inner_margin(24);
        let response = egui::Modal::new(egui::Id::unique("dialog"))
            .frame(frame)
            .backdrop_color(Color32::from_rgba_unmultiplied(27, 26, 31, 110))
            .show(ctx, |ui| {
                ui.set_width((ctx.content_rect().width() - 64.0).clamp(200.0, 400.0));
                match &mut dialog {
                    Dialog::Delete => {
                        let count = self.settings.language.count(
                            self.selected.len(),
                            ["скин", "скина", "скинов"],
                            ["skin", "skins"],
                        );
                        ui.label(
                            RichText::new(format!("{} {count}?", self.t("Удалить", "Delete")))
                                .font(font_strong(20.0)),
                        );
                        ui.add_space(8.0);
                        ui.label(RichText::new(self.t(
                            "Они исчезнут из osu!, но файлы останутся в папке восстановления — вернуть можно в истории.",
                            "They will disappear from osu!, but the files stay in the recovery folder, so you can bring them back from History.",
                        )).color(INK_MUTED));
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            if btn(Kind::Secondary, self.t("Отмена", "Cancel")).show(ui).clicked() {
                                keep = false;
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if btn(Kind::Danger, self.t("Удалить", "Delete"))
                                    .icon(icons::trash_2())
                                    .show(ui)
                                    .clicked()
                                {
                                    self.run_manage(ctx, "delete", String::new());
                                    keep = false;
                                }
                            });
                        });
                    }
                    Dialog::Name { kind, value } => {
                        let rename = *kind == "rename";
                        ui.label(
                            RichText::new(if rename {
                                self.t("Переименовать скин", "Rename skin")
                            } else {
                                self.t("Сделать копию", "Duplicate skin")
                            })
                            .font(font_strong(20.0)),
                        );
                        ui.add_space(12.0);
                        let name_label = ui.label(RichText::new(self.t("Название", "Name")).font(font_strong(13.0)));
                        let edit = ui.add(
                            egui::TextEdit::singleline(value)
                                .desired_width(f32::INFINITY)
                                .margin(egui::Margin::symmetric(10, 7)),
                        ).labelled_by(name_label.id);
                        if ui.memory(|m| m.focused().is_none()) {
                            edit.request_focus();
                        }
                        let check = skinweaver::library::validate_name(value).and_then(|_| {
                            self.library
                                .as_ref()
                                .map_or(Ok(()), |l| l.ensure_new_name(value))
                        });
                        let valid = check.is_ok();
                        if !valid && !value.is_empty() {
                            ui.label(
                                RichText::new(name_problem(self.settings.language, value))
                                    .size(12.0)
                                    .color(DANGER),
                            );
                        }
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            if btn(Kind::Secondary, self.t("Отмена", "Cancel")).show(ui).clicked() {
                                keep = false;
                            }
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                if (btn(Kind::Primary, self.t("Сохранить", "Save"))
                                    .enabled(valid)
                                    .show(ui)
                                    .clicked()
                                    || enter)
                                    && valid
                                {
                                    let kind = *kind;
                                    let value = value.clone();
                                    self.run_manage(ctx, kind, value);
                                    keep = false;
                                }
                            });
                        });
                    }
                }
            });
        if response.should_close() {
            keep = false;
        }
        if keep {
            self.dialog = Some(dialog);
        }
    }

    pub(crate) fn run_manage(&mut self, ctx: &egui::Context, kind: &'static str, name: String) {
        let Some(library) = self.library.clone() else {
            return;
        };
        let paths = self.selected.iter().cloned().collect::<Vec<_>>();
        self.spawn(ctx, move |cancel, tx| {
            library.manage(&paths, kind, &name, &cancel, &|p, s| {
                let _ = tx.send(Event::Progress(p, s.into()));
            })?;
            Ok(Outcome::Changed(paths))
        });
    }

    // ------------------------------------------------------------ audio

    /// Plays an element sound; the output device is opened once and reused. The file is
    /// read and decoded on a short-lived thread so a slow disk never stalls a frame.
    pub(crate) fn play(&mut self, folder: Option<&Path>, stem: &str) {
        let result = (|| -> Result<Arc<rodio::Player>> {
            if self.audio.is_none() {
                self.audio = Some(rodio::DeviceSinkBuilder::open_default_sink()?);
            }
            let sink = self.audio.as_ref().unwrap();
            let player = Arc::new(rodio::Player::connect_new(sink.mixer()));
            player.set_volume(self.settings.volume);
            Ok(player)
        })();
        let player = match result {
            Ok(player) => player,
            Err(error) => {
                self.player = None;
                self.sound_error(error);
                return;
            }
        };
        // Replacing the player stops whatever played before, even if it is still loading.
        self.player = Some(player.clone());
        let folder = folder.map(Path::to_path_buf);
        let stem = stem.to_owned();
        let errors = self.audio_errors.0.clone();
        let _ = std::thread::Builder::new()
            .name("sound".into())
            .spawn(move || {
                let Some((bytes, _, _)) = textures::audio_bytes(folder.as_deref(), &stem) else {
                    return;
                };
                match rodio::Decoder::try_from(std::io::Cursor::new(bytes)) {
                    Ok(decoder) => player.append(decoder),
                    Err(error) => {
                        let _ = errors.send(format!("{error:#}"));
                    }
                }
            });
    }
    fn sound_error(&mut self, error: impl std::fmt::Display) {
        self.error(format!(
            "{}: {error:#}",
            self.t(
                "Не получилось воспроизвести звук",
                "Could not play the sound"
            )
        ));
    }
    pub(crate) fn stop_audio(&mut self) {
        self.player = None;
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && self.job.is_some() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.closing = true;
            if let Some(job) = &self.job {
                job.cancel.store(true, Ordering::Relaxed);
            }
        }
        if self.closing && self.job.is_none() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let start = Instant::now();
        self.frame_ui(ui, frame);
        if let Some(profile) = &mut self.profile {
            profile.frame(start.elapsed(), &self.textures);
        }
    }
}

/// `OSU_MIXER_PROFILE=1`: prints frame times of the interface code and the texture cache
/// size to stderr every two seconds (for development builds and measurements).
struct Profile {
    since: Instant,
    frames: u32,
    total: Duration,
    worst: Duration,
}
impl Profile {
    fn new() -> Option<Self> {
        std::env::var_os("OSU_MIXER_PROFILE").map(|_| Self {
            since: Instant::now(),
            frames: 0,
            total: Duration::ZERO,
            worst: Duration::ZERO,
        })
    }
    fn frame(&mut self, took: Duration, textures: &Textures) {
        self.frames += 1;
        self.total += took;
        self.worst = self.worst.max(took);
        if self.since.elapsed() < Duration::from_secs(2) {
            return;
        }
        let (entries, bytes) = textures.stats();
        let rss = fs::read_to_string("/proc/self/statm")
            .ok()
            .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
            .map_or(String::new(), |pages| {
                format!(", rss {} MiB", pages * 4096 / 1048576)
            });
        eprintln!(
            "profile: {} frames, ui mean {:.2} ms, worst {:.2} ms, textures {entries} ({} MiB){rss}",
            self.frames,
            self.total.as_secs_f64() * 1000.0 / self.frames as f64,
            self.worst.as_secs_f64() * 1000.0,
            bytes / 1048576,
        );
        *self = Self::new().unwrap_or(Self {
            since: Instant::now(),
            frames: 0,
            total: Duration::ZERO,
            worst: Duration::ZERO,
        });
    }
}

impl App {
    fn frame_ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.textures.set_still(self.settings.reduce_motion);
        self.textures.begin_frame();
        for editor in [&mut self.mix, &mut self.modify] {
            editor.row_order_prev = std::mem::take(&mut editor.row_order);
        }
        self.watch_library(&ctx);
        self.nav(ui);
        let opacity =
            motion::panel_opacity(&ctx, egui::Id::unique("page_transition"), self.page as u64);
        let busy = self.job.is_some();
        let has_preview =
            self.library.is_some() && matches!(self.page, Page::Mix | Page::Library | Page::Modify);
        if has_preview {
            self.preview_panel(ui, opacity);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG_100).inner_margin(egui::Margin {
                left: 32,
                right: 32,
                top: 24,
                bottom: 20,
            }))
            .show(ui, |ui| {
                let rect = ui.max_rect().expand2(vec2(32.0, 24.0));
                motion::cursor_glow(ui, rect);
                ui.painter()
                    .vline(rect.left(), rect.y_range(), Stroke::new(1.0, LINE_100));
                if has_preview {
                    ui.painter().vline(
                        rect.right() + 0.5,
                        rect.y_range(),
                        Stroke::new(1.0, LINE_100),
                    );
                }
                ui.multiply_opacity(opacity);
                if self.library.is_none() && self.page != Page::Settings {
                    self.setup(ui);
                    return;
                }
                ui.add_enabled_ui(!busy && self.dialog.is_none(), |ui| match self.page {
                    Page::Mix | Page::Modify => self.editor(ui),
                    Page::Library => self.library_page(ui),
                    Page::History => self.history_page(ui),
                    Page::Settings => self.settings_page(ui),
                });
            });
        self.dialogs(&ctx);
        if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.rescan(&ctx, true);
        }
        if self.textures.pending() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

fn progress_bar(ui: &mut egui::Ui, value: f32) {
    let width = (ui.available_width() - 120.0).clamp(120.0, 360.0);
    progress(ui, value, width);
    ui.label(caption(format!("{:.0}%", value * 100.0)));
}

/// How many image and sound elements a skin holds; an empty one has only skin.ini.
pub(crate) fn elements_label(language: Language, elements: usize) -> String {
    if elements == 0 {
        language.text("только skin.ini", "skin.ini only").into()
    } else {
        language.count(
            elements,
            ["элемент", "элемента", "элементов"],
            ["element", "elements"],
        )
    }
}

/// Size in megabytes with the language's decimal separator: `0,7 МБ` / `0.7 MB`.
pub(crate) fn megabytes(language: Language, bytes: u64) -> String {
    let (value, ru, en) = if bytes < 102_400 {
        (
            format!("{:.0}", (bytes as f64 / 1024.0).max(1.0)),
            "КБ",
            "KB",
        )
    } else {
        (format!("{:.1}", bytes as f64 / 1_048_576.0), "МБ", "MB")
    };
    match language {
        Language::Ru => format!("{} {ru}", value.replace('.', ",")),
        Language::En => format!("{value} {en}"),
    }
}

pub(crate) fn display_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_owned()
}

pub(crate) fn name_problem(language: Language, name: &str) -> &'static str {
    if skinweaver::library::validate_name(name).is_err() {
        language.text(
            "Так назвать нельзя: от 1 до 120 символов, без < > : \" / \\ | ? *, точки в конце и пробелов по краям.",
            "This name won't work: use 1–120 characters without < > : \" / \\ | ? *, a trailing dot or surrounding spaces.",
        )
    } else {
        language.text(
            "Скин с таким названием уже есть — придумай другое.",
            "A skin with this name already exists — pick another one.",
        )
    }
}

pub(crate) fn source_name(language: Language, source: &Source) -> String {
    match source {
        Source::Default => language.text("Стандартный скин", "Default skin").into(),
        Source::Unchanged => language.text("Без изменений", "Unchanged").into(),
        Source::Blank => language.text("Пустой элемент", "Blank element").into(),
        Source::Skin(path) => path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}
