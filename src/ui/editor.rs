//! The element tree shared by "Mix skins" and "Modify skins".

use super::{icons, textures::ElementKey, textures::State, theme::*, *};
use skinweaver::{
    catalog::Target,
    mixer::{self, Selection},
};

/// Resolved place to read an element from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Origin {
    /// `None` is the bundled default skin.
    Folder(Option<PathBuf>),
    Blank,
}

/// How a click changes the selection of elements.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Pick {
    One,
    /// Ctrl: add to or remove from the selection.
    Toggle,
    /// Shift: everything between the last click and this one.
    Range,
}

impl Pick {
    pub(crate) fn from(ui: &egui::Ui) -> Self {
        ui.input(|i| {
            if i.modifiers.shift {
                Self::Range
            } else if i.modifiers.command {
                Self::Toggle
            } else {
                Self::One
            }
        })
    }
}

pub(crate) fn node_at<'a>(nodes: &'a [OptionNode], id: &str) -> Option<&'a OptionNode> {
    let mut parts = id.split('.').map(|p| p.parse::<usize>().ok());
    let mut node = nodes.get(parts.next()??)?;
    for part in parts {
        node = node.children.get(part?)?;
    }
    Some(node)
}

fn ancestors(id: &str) -> impl Iterator<Item = &str> {
    let mut current = Some(id);
    std::iter::from_fn(move || {
        let this = current?;
        current = this.rfind('.').map(|i| &this[..i]);
        Some(this)
    })
}

/// Image stem shown for a file target (`taikobigcircle*` + suffixes → `taikobigcircle`).
pub(crate) fn image_stem(target: &Target) -> Option<(String, bool)> {
    match target {
        Target::File {
            pattern,
            audio: false,
            animated,
            suffixes,
        } => Some(match pattern.strip_suffix('*') {
            Some(prefix) => (
                format!(
                    "{prefix}{}",
                    suffixes
                        .as_ref()
                        .and_then(|s| s.first())
                        .map_or("", String::as_str)
                ),
                false,
            ),
            None => (pattern.clone(), *animated),
        }),
        _ => None,
    }
}

pub(crate) fn audio_stem(target: &Target) -> Option<String> {
    match target {
        Target::File {
            pattern,
            audio: true,
            ..
        } => Some(pattern.trim_end_matches('*').to_owned()),
        _ => None,
    }
}

/// First image element inside a group, used for thumbnails.
pub(crate) fn representative(node: &OptionNode) -> Option<(String, bool)> {
    if let Some(target) = &node.target {
        return image_stem(target);
    }
    node.children.iter().find_map(representative)
}

pub(crate) fn has_audio(node: &OptionNode) -> bool {
    node.target
        .as_ref()
        .is_some_and(|t| audio_stem(t).is_some())
        || (!node.children.is_empty() && node.children.iter().all(has_audio))
}

impl App {
    pub(crate) fn editor_ref(&self) -> &Editor {
        if self.page == Page::Modify {
            &self.modify
        } else {
            &self.mix
        }
    }
    pub(crate) fn editor_mut(&mut self) -> &mut Editor {
        if self.page == Page::Modify {
            &mut self.modify
        } else {
            &mut self.mix
        }
    }
    pub(crate) fn base_source(&self) -> Source {
        match (self.page, &self.mix_base) {
            (Page::Modify, _) => Source::Unchanged,
            (_, Some(base)) => Source::Skin(base.clone()),
            (_, None) => Source::Default,
        }
    }

    /// The source an element ends up with: its own choice or the nearest parent's.
    pub(crate) fn effective(&self, id: &str) -> Source {
        let editor = self.editor_ref();
        ancestors(id)
            .find_map(|a| editor.choices.get(a).cloned())
            .unwrap_or_else(|| self.base_source())
    }

    /// What the element was before its last change (or the starting point).
    pub(crate) fn previous(&self, id: &str) -> Source {
        let editor = self.editor_ref();
        if let Some(source) = &editor.compare {
            return source.clone();
        }
        if self.page == Page::Modify {
            return Source::Unchanged;
        }
        ancestors(id)
            .find_map(|a| editor.previous.get(a).cloned())
            .unwrap_or_else(|| self.base_source())
    }

    /// The skin a batch edit is previewed on.
    pub(crate) fn target_skin(&self) -> Option<PathBuf> {
        self.focus_skin
            .clone()
            .filter(|p| self.selected.contains(p))
            .or_else(|| self.selected.first().cloned())
    }

    pub(crate) fn origin(&self, source: &Source) -> Origin {
        match source {
            Source::Default => Origin::Folder(None),
            Source::Blank => Origin::Blank,
            Source::Skin(path) => Origin::Folder(Some(path.clone())),
            Source::Unchanged => Origin::Folder(self.target_skin()),
        }
    }

    pub(crate) fn element(
        &mut self,
        origin: &Origin,
        stem: &str,
        animated: bool,
        max: u32,
    ) -> Option<State> {
        match origin {
            Origin::Blank => None,
            Origin::Folder(folder) => Some(self.textures.get(&ElementKey {
                folder: folder.clone(),
                stem: stem.to_owned(),
                animated,
                max,
            })),
        }
    }

    pub(crate) fn selections(&self) -> Vec<Selection> {
        fn walk(
            nodes: &[OptionNode],
            prefix: &str,
            inherited: &Source,
            choices: &HashMap<String, Source>,
            output: &mut Vec<Selection>,
        ) {
            for (i, node) in nodes.iter().enumerate() {
                let id = format!("{prefix}{i}");
                let source = choices.get(&id).unwrap_or(inherited);
                if let Some(target) = &node.target {
                    output.push(Selection {
                        target: target.clone(),
                        source: source.clone(),
                    });
                }
                walk(&node.children, &format!("{id}."), source, choices, output);
            }
        }
        let mut selections = vec![];
        walk(
            &self.nodes,
            "",
            &self.base_source(),
            &self.editor_ref().choices,
            &mut selections,
        );
        selections
    }

    pub(crate) fn set_choice(&mut self, id: &str, source: Source) {
        let old = self.effective(id);
        if old == source && self.editor_ref().choices.get(id) == Some(&source) {
            return;
        }
        let prefix = format!("{id}.");
        let editor = self.editor_mut();
        let overridden: Vec<(String, Source)> = editor
            .choices
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        editor.previous.retain(|k, _| !k.starts_with(&prefix));
        for (key, value) in overridden {
            editor.previous.insert(key, value);
        }
        editor.previous.insert(id.to_owned(), old);
        editor.choices.retain(|k, _| !k.starts_with(&prefix));
        editor.choices.insert(id.to_owned(), source.clone());
        editor.focus = Some(id.to_owned());
        if let Source::Skin(path) = source {
            self.recent.retain(|p| *p != path);
            self.recent.insert(0, path);
            self.recent.truncate(3);
        }
        self.reveal_element();
    }

    // ------------------------------------------------------------ several elements

    /// Elements a chosen source goes to: the Ctrl/Shift selection, or just the focus.
    pub(crate) fn picked_ids(&self) -> Vec<String> {
        let editor = self.editor_ref();
        if editor.picked.len() > 1 {
            editor.picked.clone()
        } else {
            editor.focus.iter().cloned().collect()
        }
    }

    pub(crate) fn is_picked(&self, id: &str) -> bool {
        let editor = self.editor_ref();
        editor.picked.len() > 1 && editor.picked.iter().any(|p| p == id)
    }

    /// A click on an element: plain picks it alone, Ctrl adds or removes it,
    /// Shift takes every element between the last click and this one in `order`.
    pub(crate) fn pick(&mut self, id: &str, how: Pick, order: &[String]) {
        let editor = self.editor_mut();
        let range = match how {
            Pick::Range => {
                let anchor = editor.anchor.clone().or_else(|| editor.focus.clone());
                let from = anchor.and_then(|a| order.iter().position(|o| *o == a));
                let to = order.iter().position(|o| o == id);
                from.zip(to)
            }
            _ => None,
        };
        match (how, range) {
            (Pick::One, _) => {
                editor.picked.clear();
                editor.focus = Some(id.to_owned());
                editor.anchor = Some(id.to_owned());
            }
            (Pick::Range, Some((from, to))) => {
                editor.picked = order[from.min(to)..=from.max(to)].to_vec();
                editor.focus = Some(id.to_owned());
            }
            // Ctrl, or Shift where there is no list to take a range from.
            _ => {
                if editor.picked.is_empty()
                    && let Some(focus) = editor.focus.clone()
                    && focus != id
                {
                    editor.picked.push(focus);
                }
                if let Some(i) = editor.picked.iter().position(|p| p == id) {
                    editor.picked.remove(i);
                    editor.focus = editor.picked.last().cloned();
                } else {
                    editor.picked.push(id.to_owned());
                    editor.focus = Some(id.to_owned());
                }
                editor.anchor = Some(id.to_owned());
            }
        }
        if editor.picked.len() <= 1 {
            editor.picked.clear();
        }
        self.reveal_element();
    }

    pub(crate) fn clear_picked(&mut self) {
        self.editor_mut().picked.clear();
    }

    /// Gives `source` to the element clicked and, if it is part of the selection, to all of it.
    pub(crate) fn choose(&mut self, id: &str, source: Source) {
        let ids = if self.is_picked(id) {
            self.picked_ids()
        } else {
            vec![id.to_owned()]
        };
        let focus = self.editor_ref().focus.clone();
        for id in &ids {
            self.set_choice(id, source.clone());
        }
        self.editor_mut().focus = focus;
    }

    /// Skins worth having at the top of a source list for this element, with why.
    pub(crate) fn pinned_skins(&self, id: &str) -> Vec<(PathBuf, &'static str)> {
        let language = self.settings.language;
        let mut pinned: Vec<(PathBuf, &'static str)> = vec![];
        let earlier = ancestors(id).find_map(|a| self.editor_ref().previous.get(a));
        if let Some(Source::Skin(path)) = earlier {
            pinned.push((path.clone(), language.text("было раньше", "before")));
        }
        for path in &self.recent {
            if !pinned.iter().any(|(p, _)| p == path) {
                pinned.push((path.clone(), language.text("недавний", "recent")));
            }
        }
        if self.page == Page::Mix
            && let Some(base) = &self.mix_base
            && !pinned.iter().any(|(p, _)| p == base)
        {
            pinned.push((base.clone(), language.text("основа", "base")));
        }
        pinned.retain(|(p, _)| self.skin_index.contains_key(p));
        pinned
    }

    fn reset_choices(&mut self) {
        let editor = self.editor_mut();
        editor.previous = std::mem::take(&mut editor.choices).into_iter().collect();
    }

    fn random_mix(&mut self) {
        let visible: Vec<_> = self
            .skins
            .iter()
            .filter(|s| !s.hidden)
            .map(|s| s.path.clone())
            .collect();
        if visible.is_empty() {
            return;
        }
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64
            | 1;
        for i in 0..self.nodes.len() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let pick = visible[seed as usize % visible.len()].clone();
            self.set_choice(&i.to_string(), Source::Skin(pick));
        }
        self.mix.focus = None;
        self.preview_tab = PreviewTab::Scene;
    }

    // ------------------------------------------------------------ page

    pub(crate) fn editor(&mut self, ui: &mut egui::Ui) {
        if ui.available_height() < 600.0 || ui.available_width() < 560.0 {
            egui::ScrollArea::vertical()
                .id_salt(("compact_editor", self.page as u8))
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_height(680.0);
                    self.editor_content(ui);
                });
        } else {
            self.editor_content(ui);
        }
    }

    fn editor_content(&mut self, ui: &mut egui::Ui) {
        let modifying = self.page == Page::Modify;
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.clear_picked();
        }
        if modifying {
            page_title(
                ui,
                self.t("Изменить скины", "Modify skins"),
                self.t(
                    "Замени элементы и добавь правки сразу в нескольких скинах.",
                    "Swap elements and apply tweaks to several skins at once.",
                ),
            );
        } else {
            page_title(
                ui,
                self.t("Собрать скин", "Mix skins"),
                if self.stage_view {
                    self.t(
                        "Пройди экраны osu! по порядку и нажимай на то, что хочешь заменить.",
                        "Walk through the osu! screens and click whatever you want to replace.",
                    )
                } else {
                    self.t(
                        "Возьми лучшее из своих скинов — по группам или по одному файлу. Ctrl+клик и Shift+клик выделяют несколько строк сразу.",
                        "Take the best of your skins — by group or file by file. Ctrl+click and Shift+click pick several rows at once.",
                    )
                },
            );
        }
        self.job_bar(ui);
        self.notice_bar(ui);
        if modifying {
            self.targets_strip(ui);
        } else {
            self.base_row(ui);
        }
        self.toolbar(ui, modifying);
        ui.add_space(12.0);
        if !modifying && self.stage_view {
            let mut bottom = 168.0;
            if self.mix_base.is_some() {
                bottom += 40.0;
            }
            let height = (ui.available_height() - bottom).max(120.0);
            self.stage_body(ui, height);
            ui.add_space(8.0);
            self.create_bar(ui);
            return;
        }
        let bottom = if modifying {
            64.0
        } else {
            168.0
        } + if !modifying && self.mix_base.is_some() {
            40.0
        } else {
            0.0
        };
        let height = (ui.available_height() - bottom).max(120.0);
        let nodes = self.nodes.clone();
        egui::ScrollArea::vertical()
            .id_salt(("tree", modifying))
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let query = self.editor_ref().query.to_lowercase();
                let mut shown = 0;
                for (i, node) in nodes.iter().enumerate() {
                    shown += self.tree_row(ui, node, &i.to_string(), 0, &query);
                }
                if shown == 0 {
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new(self.t("Ничего не нашлось", "Nothing found"))
                            .font(font_strong(16.0)),
                    );
                    ui.label(
                        RichText::new(self.t(
                            "Попробуй имя файла, например hitcircle или cursor.",
                            "Try a file name such as hitcircle or cursor.",
                        ))
                        .color(INK_MUTED),
                    );
                }
                ui.spacing_mut().item_spacing.y = 8.0;
                if modifying {
                    self.modification_controls(ui);
                }
            });
        ui.add_space(8.0);
        if modifying {
            self.apply_bar(ui);
        } else {
            self.create_bar(ui);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, modifying: bool) {
        ui.horizontal_wrapped(|ui| {
            if !modifying {
                let mut stage_view = self.stage_view;
                let options = [
                    (true, self.t("По этапам", "By stage")),
                    (false, self.t("Списком", "List")),
                ];
                if tabs(ui, &mut stage_view, &options) {
                    self.stage_view = stage_view;
                    self.preview_tab = if stage_view {
                        PreviewTab::Variants
                    } else if self.mix.focus.is_some() {
                        PreviewTab::Element
                    } else {
                        PreviewTab::Scene
                    };
                }
                ui.add_space(8.0);
            }
            if modifying || !self.stage_view {
                let buttons = if modifying { 120.0 } else { 250.0 };
                let width = (ui.available_width() - buttons).max(160.0);
                let hint = self.t("Найти элемент или файл…", "Find an element or file…");
                let editor = self.editor_mut();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut editor.query)
                        .prefix(
                            egui::Image::new(icons::search())
                                .tint(INK_SUBTLE)
                                .fit_to_exact_size(Vec2::splat(16.0)),
                        )
                        .hint_text(hint)
                        .desired_width(width)
                        .margin(egui::Margin::symmetric(10, 7)),
                ).accessible_name(hint);
                if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
                    response.request_focus();
                }
            }
                let has_choices = !self.editor_ref().choices.is_empty();
                if btn(Kind::Ghost, self.t("Сбросить", "Reset"))
                    .icon(icons::rotate_ccw())
                    .enabled(has_choices)
                    .show(ui)
                    .clicked()
                {
                    self.reset_choices();
                }
                if !modifying
                    && btn(Kind::Ghost, self.t("Случайно", "Random"))
                        .icon(icons::dices())
                        .show(ui)
                        .on_hover_text(self.t(
                            "Каждая группа — из случайного видимого скина",
                            "Each group from a random visible skin",
                        ))
                        .clicked()
                {
                    self.random_mix();
                }
        });
    }

    fn targets_strip(&mut self, ui: &mut egui::Ui) {
        if self.selected.is_empty() {
            super::motion::glass(ui, BG_200, 16, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(self.t("Сначала выбери скины", "Pick the skins first")).font(font_strong(16.0)));
                    ui.label(RichText::new(self.t(
                        "Отметь один или несколько скинов в библиотеке — изменения применятся ко всем.",
                        "Tick one or more skins in the library — the changes apply to all of them.",
                    )).color(INK_MUTED));
                    ui.add_space(8.0);
                    if btn(Kind::Primary, self.t("Открыть библиотеку", "Open library"))
                        .icon(icons::library_big())
                        .show(ui)
                        .clicked()
                    {
                        self.go(Page::Library);
                    }
                });
            ui.add_space(16.0);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(self.t("Изменяем:", "Modifying:")).color(INK_MUTED));
            let target = self.target_skin();
            let paths: Vec<_> = self.selected.iter().cloned().collect();
            for path in paths.iter().take(6) {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let current = target.as_ref() == Some(path);
                if btn(Kind::Secondary, &name)
                    .small()
                    .selected(current)
                    .show(ui)
                    .on_hover_text(self.t("Показать в предпросмотре", "Show in preview"))
                    .clicked()
                {
                    self.focus_skin = Some(path.clone());
                }
            }
            if paths.len() > 6 {
                ui.label(caption(format!("+{}", paths.len() - 6)));
            }
            if btn(Kind::Ghost, self.t("Изменить выбор", "Change selection"))
                .small()
                .show(ui)
                .clicked()
            {
                self.go(Page::Library);
            }
        });
        ui.add_space(12.0);
    }

    /// One row of the tree; returns the number of rows shown.
    pub(crate) fn tree_row(
        &mut self,
        ui: &mut egui::Ui,
        node: &OptionNode,
        id: &str,
        depth: usize,
        query: &str,
    ) -> usize {
        let russian = self.ru();
        if !query.is_empty() && !node.matches_query(query, russian) && !target_matches(node, query)
        {
            return 0;
        }
        let is_group = !node.children.is_empty();
        let expanded = is_group && (!query.is_empty() || self.editor_ref().expanded.contains(id));
        let focused = self.editor_ref().focus.as_deref() == Some(id) || self.is_picked(id);
        self.editor_mut().row_order.push(id.to_owned());
        let height = if depth == 0 { 54.0 } else { 46.0 };
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), focused, node.label(russian))
        });
        let source = self.effective(id);
        let prefix = format!("{id}.");
        let mixed = self
            .editor_ref()
            .choices
            .keys()
            .any(|k| k.starts_with(&prefix));
        if ui.is_rect_visible(rect) {
            let fill = if focused {
                ACCENT_SOFT
            } else if response.hovered() {
                BG_300
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, 0, fill);
            if depth == 0 {
                // Each top-level group opens a new catalog section with a full ink rule.
                ui.painter()
                    .hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, LINE_200));
            } else {
                hairline(ui, rect.bottom() - 0.5, rect.left()..=rect.right());
            }
            // Catalog code column.
            let code = element_code(id);
            text_at(
                ui,
                pos2(rect.left() + 6.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &code,
                condensed(if depth == 0 { 22.0 } else { 15.0 }),
                if focused {
                    ACCENT_TEXT
                } else if depth == 0 {
                    INK
                } else {
                    INK_SUBTLE
                },
            );
            let mut x = rect.left() + 52.0 + depth.saturating_sub(1) as f32 * 14.0;
            if is_group {
                let icon = if expanded {
                    icons::chevron_down()
                } else {
                    icons::chevron_right()
                };
                egui::Image::new(icon).tint(INK).paint_at(
                    ui,
                    egui::Rect::from_center_size(pos2(x + 7.0, rect.center().y), Vec2::splat(16.0)),
                );
            }
            x += 20.0;
            let thumb_size = if depth == 0 { 30.0 } else { 34.0 };
            let thumb = egui::Rect::from_min_size(
                pos2(x, rect.center().y - thumb_size / 2.0),
                Vec2::splat(thumb_size),
            );
            self.row_thumb(ui, node, &source, thumb, depth);
            x += thumb_size + 12.0;
            // Text block.
            let picker_width = (rect.width() * 0.34).clamp(150.0, 220.0);
            let text_width = (rect.right() - picker_width - 16.0 - x).max(60.0);
            let title_font = if depth == 0 {
                condensed(22.0)
            } else {
                font(14.5)
            };
            let title = truncated(ui, node.label(russian), title_font, INK, text_width);
            let meta = truncated(ui, &self.row_meta(node), font(12.5), INK_SUBTLE, text_width);
            let block = title.size().y + meta.size().y - 1.0;
            let top = rect.center().y - block / 2.0;
            ui.painter().galley(pos2(x, top), title, INK);
            ui.painter().galley(
                pos2(x, rect.center().y + block / 2.0 - meta.size().y),
                meta,
                INK_SUBTLE,
            );
            // Source picker.
            let picker = egui::Rect::from_min_size(
                pos2(
                    rect.right() - picker_width - 8.0,
                    rect.center().y - CONTROL / 2.0,
                ),
                vec2(picker_width, CONTROL),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(picker)
                    .layout(Layout::left_to_right(Align::Center)),
            );
            self.source_picker(&mut child, node, id, &source, mixed, picker_width);
        }
        focus_ring(ui, &response, rect.shrink(3.0));
        if response.clicked() {
            match Pick::from(ui) {
                Pick::One => {
                    let editor = self.editor_mut();
                    if is_group && !editor.expanded.remove(id) {
                        editor.expanded.insert(id.to_owned());
                    }
                    self.pick(id, Pick::One, &[]);
                }
                how => {
                    let order = self.editor_ref().row_order_prev.clone();
                    self.pick(id, how, &order);
                }
            }
        }
        let mut shown = 1;
        if expanded {
            for (i, child) in node.children.iter().enumerate() {
                shown += self.tree_row(ui, child, &format!("{id}.{i}"), depth + 1, query);
            }
        }
        shown
    }

    fn row_meta(&self, node: &OptionNode) -> String {
        let language = self.settings.language;
        match &node.target {
            Some(Target::File {
                pattern,
                audio,
                animated,
                ..
            }) => {
                let mut text = format!("{pattern}{}", if *audio { ".wav / .ogg" } else { ".png" });
                if *animated {
                    text.push_str(language.text(" · анимация", " · animated"));
                }
                text
            }
            Some(Target::Property { section, key }) => format!("skin.ini · [{section}] {key}"),
            Some(Target::Section {
                section,
                key,
                value,
            }) => format!("skin.ini · [{section}] {key}: {value}"),
            None => language.count(
                node.leaves().len(),
                ["элемент", "элемента", "элементов"],
                ["element", "elements"],
            ),
        }
    }

    pub(crate) fn row_thumb(
        &mut self,
        ui: &egui::Ui,
        node: &OptionNode,
        source: &Source,
        rect: egui::Rect,
        depth: usize,
    ) {
        // Non-image rows get a printed pictogram; image rows get a dark cut.
        let icon_only = |ui: &egui::Ui, icon: egui::ImageSource<'static>| {
            egui::Image::new(icon).tint(INK).paint_at(
                ui,
                egui::Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
            );
        };
        if depth == 0 && node.target.is_none() {
            let name = node.name.to_lowercase();
            let icon = if name.contains("sound") {
                icons::audio_lines()
            } else if name.contains("gameplay") {
                icons::gamepad_2()
            } else if name.contains("cursor") {
                icons::cursor()
            } else {
                icons::layers()
            };
            icon_only(ui, icon);
            return;
        }
        if has_audio(node) {
            icon_only(ui, icons::audio_lines());
            return;
        }
        if let Some(Target::Property { section, key }) = &node.target {
            if section == "Colours" {
                let origin = self.origin(source);
                if let Some(color) = self
                    .ini_value(&origin, section, key)
                    .and_then(|v| parse_color(&v))
                {
                    let swatch = egui::Rect::from_center_size(rect.center(), Vec2::splat(22.0));
                    ui.painter().rect(
                        swatch,
                        0,
                        rgb(color),
                        Stroke::new(1.0, LINE_200),
                        egui::StrokeKind::Inside,
                    );
                    return;
                }
                icon_only(ui, icons::palette());
            } else {
                icon_only(ui, icons::sliders_horizontal());
            }
            return;
        }
        if matches!(node.target, Some(Target::Section { .. })) {
            icon_only(ui, icons::sliders_horizontal());
            return;
        }
        let Some((stem, animated)) = representative(node) else {
            icon_only(ui, icons::layers());
            return;
        };
        let origin = self.origin(source);
        ui.painter().rect(
            rect,
            0,
            CUT,
            Stroke::new(1.0, LINE_100),
            egui::StrokeKind::Inside,
        );
        let icon_light = |ui: &egui::Ui| {
            egui::Image::new(icons::image_off()).tint(BG_400).paint_at(
                ui,
                egui::Rect::from_center_size(rect.center(), Vec2::splat(14.0)),
            );
        };
        match self.element(&origin, &stem, animated, 64) {
            None => icon_light(ui),
            Some(State::Ready(element)) => {
                let tint = self.combo_tint(&origin, &stem);
                paint_fit(ui, &element.frames[0], rect.shrink(3.0), tint);
            }
            Some(State::Missing) => icon_light(ui),
            Some(State::Loading) => {}
        }
    }

    pub(crate) fn ini_value(&self, origin: &Origin, section: &str, key: &str) -> Option<String> {
        match origin {
            Origin::Folder(Some(path)) => self.skin(path)?.ini.get(section, key).map(str::to_owned),
            _ => skinweaver::ini::SkinIni::default()
                .get(section, key)
                .map(str::to_owned)
                .or_else(|| default_ini_value(section, key).map(str::to_owned)),
        }
    }

    fn source_picker(
        &mut self,
        ui: &mut egui::Ui,
        node: &OptionNode,
        id: &str,
        source: &Source,
        mixed: bool,
        width: f32,
    ) {
        let language = self.settings.language;
        let label = if mixed {
            language
                .text("Разные источники", "Mixed sources")
                .to_owned()
        } else {
            source_name(language, source)
        };
        let chosen_skin = mixed || matches!(source, Source::Skin(_));
        let color = if chosen_skin { INK } else { INK_SUBTLE };
        let thumb_stem = representative(node).filter(|_| !has_audio(node));
        // A group with mixed sources has no single checked option.
        let current = (!mixed).then_some(source);
        let mut chosen = None;
        let popup_id = ui.make_persistent_id(("picker", id));
        egui::ComboBox::from_id_salt(("picker", id))
            .icon(combo_chevron)
            .width(width)
            .height(360.0)
            .truncate()
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .selected_text(if chosen_skin {
                RichText::new(label).color(color).font(font_strong(13.5))
            } else {
                RichText::new(label).color(color).size(13.5)
            })
            .show_ui(ui, |ui| {
                ui.set_min_width(260.0);
                let query_id = popup_id.with("query");
                let mut query = ui.data_mut(|d| d.get_temp::<String>(query_id).unwrap_or_default());
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut query)
                        .prefix(
                            egui::Image::new(icons::search())
                                .tint(INK_SUBTLE)
                                .fit_to_exact_size(Vec2::splat(14.0)),
                        )
                        .hint_text(language.text("Найти скин…", "Find a skin…"))
                        .desired_width(f32::INFINITY)
                        .margin(egui::Margin::symmetric(8, 5)),
                );
                if !edit.has_focus() && ui.memory(|m| m.focused().is_none()) {
                    edit.request_focus();
                }
                ui.data_mut(|d| d.insert_temp(query_id, query.clone()));
                ui.add_space(4.0);
                let mut options = vec![];
                if self.page == Page::Modify {
                    options.push(Source::Unchanged);
                }
                options.push(Source::Default);
                options.push(Source::Blank);
                let query = query.to_lowercase();
                for option in options {
                    if self
                        .picker_row(ui, &option, current, None, &thumb_stem)
                        .clicked()
                    {
                        chosen = Some(option);
                    }
                }
                let pinned = self.pinned_skins(id);
                if !pinned.is_empty() {
                    ui.separator();
                }
                for (path, why) in &pinned {
                    let option = Source::Skin(path.clone());
                    let name = source_name(language, &option);
                    if self
                        .picker_row(ui, &option, current, Some((name, why)), &thumb_stem)
                        .clicked()
                    {
                        chosen = Some(option);
                    }
                }
                ui.separator();
                let skins: Vec<(PathBuf, String, bool)> = self
                    .skins
                    .iter()
                    .filter(|s| {
                        query.is_empty()
                            || s.name.to_lowercase().contains(&query)
                            || s.author.to_lowercase().contains(&query)
                    })
                    .map(|s| (s.path.clone(), s.name.clone(), s.hidden))
                    .collect();
                if skins.is_empty() {
                    ui.label(caption(
                        language.text("Скинов не нашлось", "No skins found"),
                    ));
                }
                for (path, name, hidden) in skins {
                    let option = Source::Skin(path);
                    let extra = hidden.then(|| language.text("скрыт", "hidden"));
                    if self
                        .picker_row(
                            ui,
                            &option,
                            current,
                            extra.map(|e| (name.clone(), e)),
                            &thumb_stem,
                        )
                        .clicked()
                    {
                        chosen = Some(option);
                    }
                }
                if chosen.is_some() {
                    ui.close();
                }
            });
        if let Some(choice) = chosen {
            self.choose(id, choice);
        }
    }

    fn picker_row(
        &mut self,
        ui: &mut egui::Ui,
        option: &Source,
        current: Option<&Source>,
        extra: Option<(String, &str)>,
        thumb: &Option<(String, bool)>,
    ) -> egui::Response {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let selected = current == Some(option);
        let name = source_name(self.settings.language, option);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, &name)
        });
        ui.painter().rect_filled(
            rect,
            RADIUS_SM,
            if selected {
                ACCENT_SOFT
            } else if response.hovered() {
                BG_400
            } else {
                Color32::TRANSPARENT
            },
        );
        let mut x = rect.left() + 6.0;
        if let Some((stem, animated)) = thumb {
            let box_rect =
                egui::Rect::from_min_size(pos2(x, rect.center().y - 13.0), Vec2::splat(26.0));
            ui.painter().rect(
                box_rect,
                0,
                CUT,
                Stroke::new(1.0, LINE_100),
                egui::StrokeKind::Inside,
            );
            let origin = self.origin(option);
            match self.element(&origin, stem, *animated, 64) {
                Some(State::Ready(element)) => {
                    let tint = self.combo_tint(&origin, stem);
                    paint_fit(ui, &element.frames[0], box_rect.shrink(2.0), tint);
                }
                None => {
                    egui::Image::new(icons::image_off()).tint(BG_400).paint_at(
                        ui,
                        egui::Rect::from_center_size(box_rect.center(), Vec2::splat(14.0)),
                    );
                }
                _ => {}
            }
            x += 34.0;
        }
        let color = INK;
        let galley = truncated(
            ui,
            &name,
            if selected {
                font_strong(13.5)
            } else {
                font(13.5)
            },
            color,
            (rect.right() - 60.0 - x).max(40.0),
        );
        ui.painter().galley(
            pos2(x, rect.center().y - galley.size().y / 2.0),
            galley,
            color,
        );
        if let Some((_, extra)) = extra {
            text_at(
                ui,
                pos2(rect.right() - 8.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                extra,
                font(11.0),
                INK_SUBTLE,
            );
        } else if selected {
            egui::Image::new(icons::check()).tint(INK).paint_at(
                ui,
                egui::Rect::from_center_size(
                    pos2(rect.right() - 16.0, rect.center().y),
                    Vec2::splat(16.0),
                ),
            );
        }
        focus_ring(ui, &response, rect.shrink(3.0));
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// The mix's moment, as kohu!'s StatusSpecimen: a plum plate whose rings tell the state
    /// (empty, ready, busy, wrong name, done), with the name field and the one pink button.
    fn create_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        let language = self.settings.language;
        let base = self.mix_base.clone();
        let base_name = base
            .as_ref()
            .map(|p| source_name(language, &Source::Skin(p.clone())));
        // The mix as a shopping list: how many picks come from each skin.
        let mut tally: Vec<(String, usize)> = vec![];
        for source in self.mix.choices.values() {
            let name = source_name(language, source);
            match tally.iter_mut().find(|(n, _)| *n == name) {
                Some(entry) => entry.1 += 1,
                None => tally.push((name, 1)),
            }
        }
        tally.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let valid = skinweaver::library::validate_name(&self.mix_name).is_ok()
            && self
                .library
                .as_ref()
                .is_some_and(|lib| lib.ensure_new_name(&self.mix_name).is_ok());
        let now = ui.input(|i| i.time);
        let just_created = self.created_at.is_some_and(|t| now - t < 6.0);
        if just_created {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
        let busy = self.job.as_ref().map(|job| job.status.clone());
        let state = if busy.is_some() {
            Rings::Busy
        } else if just_created {
            Rings::Running
        } else if !valid {
            Rings::Error
        } else if self.mix.choices.is_empty() && base.is_none() {
            Rings::Idle
        } else {
            Rings::Ready
        };
        let rest = match &base_name {
            Some(name) => format!("{} «{name}»", self.t("остальное — из", "everything else from")),
            None => self
                .t("остальное — из стандартного скина", "everything else from the default skin")
                .to_owned(),
        };
        let mut made_of: Vec<String> = tally
            .iter()
            .take(4)
            .map(|(name, count)| format!("{name} ×{count}"))
            .collect();
        if tally.len() > 4 {
            made_of.push(format!("+{}", tally.len() - 4));
        }
        made_of.push(rest);
        let (title, text) = match state {
            Rings::Busy => (
                self.t("Собираю скин…", "Mixing your skin…").to_owned(),
                busy.unwrap_or_default(),
            ),
            Rings::Running => (
                self.t("Скин готов", "Your skin is ready").to_owned(),
                format!(
                    "«{}» {}",
                    self.created_name.clone().unwrap_or_default(),
                    self.t(
                        "уже в библиотеке.",
                        "is in your library.",
                    )
                ),
            ),
            Rings::Error => (
                self.t("Нужно другое название", "This name won't do").to_owned(),
                name_problem(language, &self.mix_name).to_owned(),
            ),
            Rings::Idle => (
                self.t("Микс пока пуст", "The mix is empty so far").to_owned(),
                self.t(
                    "Выбирай элементы на сцене — всё нетронутое останется из стандартного скина.",
                    "Pick elements on the scene; anything untouched stays from the default skin.",
                )
                .to_owned(),
            ),
            Rings::Ready => (
                match &base_name {
                    Some(name) => format!("{} «{name}»", self.t("Копия", "A copy of")),
                    None => self.t("Микс готов", "The mix is ready").to_owned(),
                },
                made_of.join("  ·  "),
            ),
        };
        let mut create = false;
        let mut save_into = false;
        let has_changes = !self.mix.choices.is_empty();
        plate(ui, state, &[], |ui| {
            ui.add(egui::Label::new(RichText::new(&title).font(serif(27.0))).truncate())
                .on_hover_text(&title);
            ui.add_space(-4.0);
            ui.add(
                egui::Label::new(RichText::new(&text).size(13.0).color(PLUM_MUTED)).truncate(),
            ).on_hover_text(&text);
            ui.add_space(6.0);
            let inline = ui.available_width() >= 360.0;
            let show_shortcuts = ui.available_width() >= 520.0;
            ui.horizontal_wrapped(|ui| {
                let hint = if base.is_some() {
                    language.text("Название копии", "Copy name")
                } else {
                    language.text("Название нового скина", "New skin name")
                };
                let reserve = if show_shortcuts { 330.0 } else { 230.0 };
                let width = if inline { (ui.available_width() - reserve).max(140.0) } else { ui.available_width() };
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.mix_name)
                        .desired_width(width)
                        .hint_text(hint)
                        .margin(egui::Margin::symmetric(10, 12)),
                ).accessible_name(hint);
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let hotkey = ui.is_enabled()
                    && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter));
                let label = if base.is_some() {
                    self.t("Сохранить копией", "Save as a copy")
                } else {
                    self.t("Создать скин", "Create skin")
                };
                let clicked = btn(Kind::Accent, label)
                    .icon(icons::add_box())
                    .large()
                    .min_width(180.0)
                    .enabled(valid)
                    .show(ui)
                    .clicked();
                if show_shortcuts {
                    ui.add_space(4.0);
                    kbd(ui, "Ctrl", true);
                    kbd(ui, "Enter", true);
                }
                create = (clicked || enter || hotkey) && valid;
            });
            // An existing skin can also take the changes itself.
            if let Some(base_name) = &base_name {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    save_into = btn(Kind::OnPlate, self.t("Записать в основу", "Save into base"))
                        .icon(icons::wand_sparkles())
                        .enabled(has_changes)
                        .show(ui)
                        .on_hover_text(base_name)
                        .clicked();
                    ui.label(RichText::new(if has_changes {
                        self.t(
                            "Оригинал сохранится — отменить можно в истории.",
                            "The original is kept, so you can undo this in History.",
                        )
                    } else {
                        self.t("Сначала выбери, что поменять.", "Pick something to change first.")
                    }).size(12.5).color(PLUM_MUTED));
                });
            }
        });
        if create {
            let library = self.library.clone().unwrap();
            let name = self.mix_name.clone();
            match &base {
                Some(base) => {
                    let base = base.clone();
                    let selections = self.selections_onto(&base);
                    self.spawn(ui.ctx(), move |cancel, tx| {
                        mixer::create_from_skin(
                            &library,
                            &base,
                            &name,
                            &selections,
                            &cancel,
                            &|p, s| {
                                let _ = tx.send(Event::Progress(p, s.into()));
                            },
                        )?;
                        Ok(Outcome::Created(name))
                    });
                }
                None => {
                    let selections = self.selections();
                    self.spawn(ui.ctx(), move |cancel, tx| {
                        mixer::create_mix(&library, &name, &selections, &cancel, &|p, s| {
                            let _ = tx.send(Event::Progress(p, s.into()));
                        })?;
                        Ok(Outcome::Created(name))
                    });
                }
            }
        }
        if save_into && let Some(base) = base {
            let library = self.library.clone().unwrap();
            let selections = self.selections_onto(&base);
            self.clear_mix_after_job = true;
            self.spawn(ui.ctx(), move |cancel, tx| {
                mixer::modify_skins(
                    &library,
                    std::slice::from_ref(&base),
                    &selections,
                    &skinweaver::modify::Modifications::default(),
                    &cancel,
                    &|p, s| {
                        let _ = tx.send(Event::Progress(p, s.into()));
                    },
                )?;
                Ok(Outcome::Changed(vec![base]))
            });
        }
    }

    /// Selections for a mix built on an existing skin: whatever still comes from the
    /// base is left alone instead of being copied onto itself.
    pub(crate) fn selections_onto(&self, base: &Path) -> Vec<Selection> {
        let base = Source::Skin(base.to_path_buf());
        self.selections()
            .into_iter()
            .map(|mut selection| {
                if selection.source == base {
                    selection.source = Source::Unchanged;
                }
                selection
            })
            .collect()
    }

    /// The skin the mix starts from; `None` builds a new skin on the default one.
    pub(crate) fn set_mix_base(&mut self, base: Option<PathBuf>) {
        if self.mix_base == base {
            return;
        }
        self.mix_base = base;
        self.mix.previous.clear();
        self.mix.compare = None;
        if let (Some(path), Some(library)) = (&self.mix_base, &self.library) {
            let name = source_name(self.settings.language, &Source::Skin(path.clone()));
            let suffix = self.settings.language.text("копия", "copy");
            let first = format!("{name} ({suffix})");
            self.mix_name = std::iter::once(first.clone())
                .chain((2..1000).map(|n| format!("{name} ({suffix} {n})")))
                .find(|candidate| library.ensure_new_name(candidate).is_ok())
                .unwrap_or(first);
        }
    }

    fn base_row(&mut self, ui: &mut egui::Ui) {
        let language = self.settings.language;
        let current = self.mix_base.clone();
        let label = match &current {
            None => language
                .text(
                    "Новый скин на основе стандартного",
                    "New skin on the default one",
                )
                .to_owned(),
            Some(path) => source_name(language, &Source::Skin(path.clone())),
        };
        let mut chosen: Option<Option<PathBuf>> = None;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(language.text("Основа", "Start from")).font(font_strong(13.5)));
            egui::ComboBox::from_id_salt("mix_base")
                .icon(combo_chevron)
                .width(ui.available_width().clamp(120.0, 300.0))
                .height(400.0)
                .truncate()
                .selected_text(if current.is_some() {
                    RichText::new(label).font(font_strong(13.5))
                } else {
                    RichText::new(label).size(13.5)
                })
                .show_ui(ui, |ui| {
                    ui.set_min_width(300.0);
                    let query_id = ui.make_persistent_id("mix_base_query");
                    let mut query =
                        ui.data_mut(|d| d.get_temp::<String>(query_id).unwrap_or_default());
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut query)
                            .hint_text(language.text("Найти скин…", "Find a skin…"))
                            .desired_width(f32::INFINITY)
                            .margin(egui::Margin::symmetric(8, 5)),
                    );
                    if !edit.has_focus() && ui.memory(|m| m.focused().is_none()) {
                        edit.request_focus();
                    }
                    ui.data_mut(|d| d.insert_temp(query_id, query.clone()));
                    if ui
                        .selectable_label(
                            current.is_none(),
                            language.text(
                                "Новый скин на основе стандартного",
                                "New skin on the default one",
                            ),
                        )
                        .clicked()
                    {
                        chosen = Some(None);
                    }
                    ui.separator();
                    let query = query.to_lowercase();
                    for skin in &self.skins {
                        if !query.is_empty() && !skin.name.to_lowercase().contains(&query) {
                            continue;
                        }
                        if ui
                            .selectable_label(current.as_ref() == Some(&skin.path), &skin.name)
                            .clicked()
                        {
                            chosen = Some(Some(skin.path.clone()));
                        }
                    }
                    if chosen.is_some() {
                        ui.close();
                    }
                });
        });
            ui.add(
                egui::Label::new(caption(if current.is_some() {
                    language.text(
                        "Это твой скин целиком: смотри его по этапам и меняй что угодно.",
                        "This is your whole skin: browse it stage by stage and change anything.",
                    )
                } else {
                    language.text(
                        "Или выбери свой скин — посмотреть его и изменить.",
                        "Or pick one of your skins to look at and change it.",
                    )
                }))
                .wrap(),
            );
        ui.add_space(10.0);
        if let Some(base) = chosen {
            self.set_mix_base(base);
        }
    }

    fn apply_bar(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        hairline(ui, rect.top(), rect.left()..=rect.right());
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            let count = self.selected.len();
            let label = if count == 0 {
                self.t("Применить", "Apply").to_owned()
            } else {
                format!(
                    "{} {}",
                    self.t("Применить к", "Apply to"),
                    self.settings.language.count(
                        count,
                        ["скину", "скинам", "скинам"],
                        ["skin", "skins"]
                    )
                )
            };
            let note = self.t(
                "Оригиналы сохраняются — любое изменение можно отменить.",
                "Originals are kept, so every change can be undone.",
            );
            let width = (ui.available_width() - 240.0).max(80.0);
            let galley = ui
                .painter()
                .layout(note.to_owned(), font(13.0), INK_MUTED, width);
            let (rect, _) = ui.allocate_exact_size(vec2(width, 44.0), Sense::hover());
            ui.painter().galley(
                pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
                galley,
                INK_MUTED,
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if btn(Kind::Accent, &label)
                    .icon(icons::wand_sparkles())
                    .large()
                    .enabled(count > 0)
                    .show(ui)
                    .clicked()
                {
                    let library = self.library.clone().unwrap();
                    let paths = self.selected.iter().cloned().collect::<Vec<_>>();
                    let selections = self.selections();
                    let mut flags = self.flags.clone();
                    flags.combo_colors = self.edit_colors.then(|| self.colors.clone());
                    flags.cursor_color = self.edit_cursor.then_some(self.cursor_color);
                    self.spawn(ui.ctx(), move |cancel, tx| {
                        mixer::modify_skins(
                            &library,
                            &paths,
                            &selections,
                            &flags,
                            &cancel,
                            &|p, s| {
                                let _ = tx.send(Event::Progress(p, s.into()));
                            },
                        )?;
                        Ok(Outcome::Changed(paths))
                    });
                }
            });
        });
    }

    fn modification_controls(&mut self, ui: &mut egui::Ui) {
        ui.add_space(28.0);
        section_title(ui, self.t("Дополнительные правки", "Extra tweaks"));
        ui.add_space(4.0);
        let language = self.settings.language;
        toggle(
            ui,
            &mut self.flags.smooth_trail,
            language.text("Плавный след курсора", "Smooth cursor trail"),
        );
        toggle(
            ui,
            &mut self.flags.instafade,
            language.text(
                "Круги исчезают сразу (instafade)",
                "Instantly fading circles (instafade)",
            ),
        );
        toggle(
            ui,
            &mut self.flags.static_interface,
            language.text("Без анимаций интерфейса", "No interface animations"),
        );
        toggle(
            ui,
            &mut self.flags.invisible_cursor,
            language.text("Невидимый курсор", "Invisible cursor"),
        );
        ui.add_space(8.0);
        toggle(
            ui,
            &mut self.edit_colors,
            language.text("Свои цвета комбо", "Custom combo colours"),
        );
        if self.edit_colors {
            ui.horizontal_wrapped(|ui| {
                ui.add_space(52.0);
                for color in &mut self.colors {
                    ui.color_edit_button_srgb(color);
                }
                if icon_button(
                    ui,
                    icons::plus(),
                    language.text("Добавить цвет", "Add colour"),
                )
                .clicked()
                    && self.colors.len() < 8
                {
                    self.colors.push([76, 201, 194]);
                }
                if icon_button(
                    ui,
                    icons::minus(),
                    language.text("Убрать цвет", "Remove colour"),
                )
                .clicked()
                    && self.colors.len() > 1
                {
                    self.colors.pop();
                }
            });
        }
        toggle(
            ui,
            &mut self.edit_cursor,
            language.text("Перекрасить курсор", "Recolour cursor"),
        );
        if self.edit_cursor {
            ui.horizontal(|ui| {
                ui.add_space(52.0);
                ui.color_edit_button_srgb(&mut self.cursor_color);
                ui.add(
                    egui::Slider::new(&mut self.flags.saturation_threshold, 0.0..=1.0)
                        .text(language.text("Порог насыщенности", "Saturation threshold")),
                );
            });
        }
        ui.add_space(4.0);
        ui.label(caption(language.text(
            "Instafade и плавный след видны в игре после применения; цвета и курсор — уже в предпросмотре.",
            "Instafade and the smooth trail show in game after applying; colours and the cursor show in the preview now.",
        )));
        ui.add_space(12.0);
    }
}

/// Catalog address of an element: top-level group letter plus the path below it,
/// e.g. `0` → `A`, `1.0.2` → `B1-3`.
pub(crate) fn element_code(id: &str) -> String {
    let mut parts = id.split('.');
    let top = parts.next().and_then(|p| p.parse::<u8>().ok()).unwrap_or(0);
    let letter = char::from(b'A' + top.min(25));
    let rest: Vec<String> = parts
        .filter_map(|p| p.parse::<usize>().ok())
        .map(|n| (n + 1).to_string())
        .collect();
    format!("{letter}{}", rest.join("-"))
}

fn target_matches(node: &OptionNode, query: &str) -> bool {
    match &node.target {
        Some(Target::File { pattern, .. }) => pattern.to_lowercase().contains(query),
        Some(Target::Property { key, .. }) => key.to_lowercase().contains(query),
        _ => node.children.iter().any(|c| target_matches(c, query)),
    }
}

pub(crate) fn parse_color(value: &str) -> Option<[u8; 3]> {
    let parts: Vec<u8> = value
        .split(',')
        .take(3)
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    (parts.len() == 3).then(|| [parts[0], parts[1], parts[2]])
}

/// Values osu! uses when skin.ini leaves a colour out.
pub(crate) fn default_ini_value(section: &str, key: &str) -> Option<&'static str> {
    if !section.eq_ignore_ascii_case("Colours") {
        return None;
    }
    Some(match key {
        "SliderBorder" => "255,255,255",
        "SliderTrackOverride" => "0,0,0",
        "MenuGlow" => "0,78,155",
        "SongSelectActiveText" => "0,0,0",
        "SongSelectInactiveText" => "255,255,255",
        "SpinnerBackground" => "100,100,100",
        "StarBreakAdditive" => "255,182,193",
        "InputOverlayText" => "0,0,0",
        "Combo1" => "255,192,0",
        "Combo2" => "0,202,0",
        "Combo3" => "18,124,255",
        "Combo4" => "242,24,57",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ids_resolve_and_walk_up() {
        let nodes = catalog();
        assert_eq!(node_at(&nodes, "2").unwrap().name, "Cursor");
        assert!(node_at(&nodes, "2.0").is_some());
        assert!(node_at(&nodes, "9").is_none());
        assert_eq!(
            ancestors("1.2.3").collect::<Vec<_>>(),
            ["1.2.3", "1.2", "1"]
        );
    }
    #[test]
    fn wildcard_stems_use_first_suffix() {
        let target = Target::File {
            pattern: "taikobigcircle*".into(),
            audio: false,
            animated: false,
            suffixes: Some(vec!["".into(), "overlay".into()]),
        };
        assert_eq!(image_stem(&target), Some(("taikobigcircle".into(), false)));
    }
}
