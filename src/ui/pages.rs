//! Library, history, settings and first-run pages.

use super::{editor::Origin, icons, textures::State, theme::*, *};

impl App {
    pub(crate) fn setup(&mut self, ui: &mut egui::Ui) {
        ui.add_space(40.0);
        hit_mark(ui, 56.0);
        ui.add_space(12.0);
        page_title(
            ui,
            self.t("Покажи, где лежит osu!", "Show me where osu! lives"),
            self.t(
                "Нужна папка, внутри которой есть Skins. Скины никуда не уходят с компьютера.",
                "Pick the folder that contains Skins. Your skins never leave this computer.",
            ),
        );
        self.notice_bar(ui);
        ui.set_max_width(640.0);
        let folder_label = ui.label(RichText::new(self.t("Папка osu!", "osu! folder")).font(font_strong(13.0)));
        ui.add(
            egui::TextEdit::singleline(&mut self.folder_input)
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(10, 8))
                .hint_text("C:\\Users\\…\\AppData\\Local\\osu!"),
        ).labelled_by(folder_label.id);
        ui.label(caption(self.t(
            "Обычно это %LOCALAPPDATA%\\osu!",
            "Usually %LOCALAPPDATA%\\osu!",
        )));
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if btn(Kind::Primary, self.t("Открыть библиотеку", "Open library"))
                .enabled(!self.folder_input.trim().is_empty())
                .show(ui)
                .clicked()
            {
                self.set_folder(ui.ctx());
            }
            if btn(Kind::Ghost, self.t("Выбрать папку…", "Browse…"))
                .icon(icons::folder_search())
                .show(ui)
                .clicked()
                && let Some(folder) = rfd::FileDialog::new().pick_folder()
            {
                self.folder_input = display_path(&folder);
                self.set_folder(ui.ctx());
            }
        });
    }

    /// Library rows for the current search, filter and sort; recomputed only when one of
    /// them or the library changes, not on every repaint.
    fn visible_skins(&self) -> std::rc::Rc<Vec<usize>> {
        let key: super::VisibleKey = (self.query.clone(), self.filter, self.sort);
        if let Some((cached, rows)) = &*self.visible_cache.borrow()
            && *cached == key
        {
            return rows.clone();
        }
        let rows = std::rc::Rc::new(self.compute_visible_skins());
        *self.visible_cache.borrow_mut() = Some((key, rows.clone()));
        rows
    }

    fn compute_visible_skins(&self) -> Vec<usize> {
        let query = self.query.to_lowercase();
        let mut visible: Vec<usize> = self
            .skins
            .iter()
            .enumerate()
            .filter(|(_, s)| match self.filter {
                Filter::All => true,
                Filter::Visible => !s.hidden,
                Filter::Hidden => s.hidden,
            })
            .filter(|(_, s)| {
                query.is_empty()
                    || s.name.to_lowercase().contains(&query)
                    || s.author.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        match self.sort {
            Sort::Newest => visible.sort_by_key(|&i| std::cmp::Reverse(self.skins[i].modified)),
            Sort::Size => visible.sort_by_key(|&i| std::cmp::Reverse(self.skins[i].bytes)),
            Sort::Name => {}
        }
        visible
    }

    fn filter_tabs(&mut self, ui: &mut egui::Ui) {
        let options = [
            (Filter::All, self.t("Все", "All")),
            (Filter::Visible, self.t("Видимые", "Visible")),
            (Filter::Hidden, self.t("Скрытые", "Hidden")),
        ];
        tabs(ui, &mut self.filter, &options);
    }

    fn sort_box(&mut self, ui: &mut egui::Ui) {
        let sort_label = match self.sort {
            Sort::Name => self.t("По названию", "By name"),
            Sort::Newest => self.t("Сначала новые", "Newest"),
            Sort::Size => self.t("По размеру", "By size"),
        };
        let language = self.settings.language;
        egui::ComboBox::from_id_salt("library_sort")
            .icon(combo_chevron)
            .width(128.0)
            .selected_text(RichText::new(sort_label).size(13.0))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.sort,
                    Sort::Name,
                    language.text("По названию", "By name"),
                );
                ui.selectable_value(
                    &mut self.sort,
                    Sort::Newest,
                    language.text("Сначала новые", "Newest"),
                );
                ui.selectable_value(
                    &mut self.sort,
                    Sort::Size,
                    language.text("По размеру", "By size"),
                );
            });
    }

    pub(crate) fn library_page(&mut self, ui: &mut egui::Ui) {
        page_title(
            ui,
            self.t("Библиотека", "Library"),
            self.t(
                "Отметь скины, чтобы изменить их пачкой, или нажми на скин, чтобы рассмотреть.",
                "Tick skins to edit them in bulk, or click one to take a closer look.",
            ),
        );
        self.job_bar(ui);
        self.notice_bar(ui);
        let wide = ui.available_width() >= 700.0;
        ui.horizontal_wrapped(|ui| {
            let reserved = if wide { 460.0 } else { 190.0 };
            let width = (ui.available_width() - reserved).max(140.0);
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .prefix(
                        egui::Image::new(icons::search())
                            .tint(INK_SUBTLE)
                            .fit_to_exact_size(Vec2::splat(16.0)),
                    )
                    .hint_text(
                        self.settings
                            .language
                            .text("Название или автор…", "Name or author…"),
                    )
                    .desired_width(width)
                    .margin(egui::Margin::symmetric(10, 7)),
            ).accessible_name(self.t("Поиск скинов", "Search skins"));
            if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
                response.request_focus();
            }
            if wide {
                self.filter_tabs(ui);
            }
            self.sort_box(ui);
            if icon_button(
                ui,
                icons::refresh_cw(),
                self.t("Перечитать библиотеку (F5)", "Re-read library (F5)"),
            )
            .clicked()
            {
                self.rescan(ui.ctx(), true);
            }
        });
        if !wide {
            ui.add_space(8.0);
            self.filter_tabs(ui);
        }
        ui.add_space(12.0);
        let visible = self.visible_skins();
        self.selection_bar(ui, visible.as_slice());
        ui.add_space(8.0);
        if visible.is_empty() {
            ui.add_space(32.0);
            hit_mark(ui, 44.0);
            ui.add_space(8.0);
            let empty_library = self.skins.is_empty();
            ui.label(
                RichText::new(if empty_library {
                    if self.scan.is_some() {
                        self.t("Читаю библиотеку…", "Reading your library…")
                    } else {
                        self.t(
                            "Добавь скины в папку Skins",
                            "Add skins to your Skins folder",
                        )
                    }
                } else {
                    self.t("Ничего не нашлось", "Nothing found")
                })
                .font(condensed(30.0)),
            );
            ui.label(
                RichText::new(if empty_library {
                    self.t(
                        "Новые скины появятся здесь сами.",
                        "New skins will show up here on their own.",
                    )
                } else {
                    self.t(
                        "Попробуй другой запрос или фильтр.",
                        "Try another search or filter.",
                    )
                })
                .color(INK_MUTED),
            );
            return;
        }
        let height = if self.settings.compact { 44.0 } else { 60.0 };
        egui::ScrollArea::vertical()
            .id_salt("skin_library")
            .auto_shrink([false, false])
            .show_rows(ui, height, visible.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for &index in &visible[range] {
                    self.skin_row(ui, index, height);
                }
            });
    }

    fn selection_bar(&mut self, ui: &mut egui::Ui, visible: &[usize]) {
        ui.horizontal_wrapped(|ui| {
            let all = !visible.is_empty()
                && visible
                    .iter()
                    .all(|&i| self.selected.contains(&self.skins[i].path));
            if check(ui, all, self.t("Выбрать все видимые скины", "Select all shown skins"))
                .on_hover_text(self.t("Выбрать все видимые", "Select all shown"))
                .clicked()
            {
                if all {
                    for &i in visible {
                        self.selected.remove(&self.skins[i].path);
                    }
                } else {
                    self.selected
                        .extend(visible.iter().map(|&i| self.skins[i].path.clone()));
                }
            }
            if self.selected.is_empty() {
                ui.label(caption(self.settings.language.count(
                    visible.len(),
                    ["скин", "скина", "скинов"],
                    ["skin", "skins"],
                )));
                return;
            }
            ui.label(
                RichText::new(format!(
                    "{} {}",
                    self.t("Выбрано:", "Selected:"),
                    self.selected.len()
                ))
                .font(font_strong(13.0)),
            );
            ui.add_space(8.0);
            if btn(Kind::Primary, self.t("Изменить", "Modify"))
                .icon(icons::wand_sparkles())
                .small()
                .show(ui)
                .clicked()
            {
                self.go(Page::Modify);
            }
            let language = self.settings.language;
            let single = self.selected.len() == 1;
            if btn(
                Kind::Ghost,
                language.text("Скрыть / показать", "Hide / show"),
            )
            .icon(icons::eye_off())
            .small()
            .show(ui)
            .clicked()
            {
                self.run_manage(ui.ctx(), "hide", String::new());
            }
            if single {
                let name = self
                    .selected
                    .first()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if btn(Kind::Ghost, "")
                    .icon(icons::copy())
                    .small()
                    .show(ui)
                    .accessible_name(language.text("Сделать копию", "Duplicate"))
                    .on_hover_text(language.text("Сделать копию", "Duplicate"))
                    .clicked()
                {
                    self.dialog = Some(Dialog::Name {
                        kind: "duplicate",
                        value: format!("{name}{}", language.text(" — копия", " — copy")),
                    });
                }
                if btn(Kind::Ghost, "")
                    .icon(icons::pencil_line())
                    .small()
                    .show(ui)
                    .accessible_name(language.text("Переименовать", "Rename"))
                    .on_hover_text(language.text("Переименовать", "Rename"))
                    .clicked()
                {
                    self.dialog = Some(Dialog::Name {
                        kind: "rename",
                        value: name,
                    });
                }
                if btn(Kind::Ghost, "")
                    .icon(icons::package())
                    .small()
                    .show(ui)
                    .accessible_name(language.text("Сохранить .osk", "Export .osk"))
                    .on_hover_text(language.text("Сохранить .osk", "Export .osk"))
                    .clicked()
                {
                    self.export_selected(ui.ctx());
                }
            }
            if btn(Kind::Ghost, "")
                .icon(icons::trash_2())
                .small()
                .show(ui)
                .accessible_name(language.text("Удалить", "Delete"))
                .on_hover_text(language.text("Удалить", "Delete"))
                .clicked()
            {
                self.dialog = Some(Dialog::Delete);
            }
            if btn(Kind::Ghost, "")
                .icon(icons::x())
                .small()
                .show(ui)
                .accessible_name(language.text("Снять выделение", "Clear selection"))
                .on_hover_text(language.text("Снять выделение", "Clear selection"))
                .clicked()
            {
                self.selected.clear();
            }
        });
    }

    fn skin_row(&mut self, ui: &mut egui::Ui, index: usize, height: f32) {
        let skin = &self.skins[index];
        let path = skin.path.clone();
        let name = skin.name.clone();
        let author = skin.author.clone();
        let hidden = skin.hidden;
        let elements = skin.elements;
        let bytes = skin.bytes;
        let color = skin.ini.colors()[0];
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        let selected = self.selected.contains(&path);
        let focused = self.focus_skin.as_ref() == Some(&path);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), focused, &name)
        });
        if !ui.is_rect_visible(rect) {
            return;
        }
        let fill = if focused {
            ACCENT_SOFT
        } else if response.hovered() {
            BG_300
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, 0, fill);
        hairline(ui, rect.bottom() - 0.5, rect.left()..=rect.right());
        // Catalog code: name block letter + place in that block; stable under sort and filter.
        let code = self.skin_codes.get(index).cloned().unwrap_or_default();
        text_at(
            ui,
            pos2(rect.left() + 44.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &code,
            condensed(19.0),
            if focused { ACCENT_TEXT } else { INK_SUBTLE },
        );
        // Checkbox.
        let check_rect = egui::Rect::from_center_size(
            pos2(rect.left() + 20.0, rect.center().y),
            Vec2::splat(24.0),
        );
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(check_rect));
        if check(&mut child, selected, &format!("{} {name}", self.t("Выбрать скин", "Select skin"))).clicked() {
            if selected {
                self.selected.remove(&path);
            } else {
                self.selected.insert(path.clone());
            }
        }
        // Thumbnail: the skin's own hit circle in its first combo colour.
        let thumb_size = height - 16.0;
        let thumb = egui::Rect::from_min_size(
            pos2(rect.left() + 82.0, rect.center().y - thumb_size / 2.0),
            Vec2::splat(thumb_size),
        );
        ui.painter().rect(
            thumb,
            0,
            CUT,
            Stroke::new(1.0, LINE_100),
            egui::StrokeKind::Inside,
        );
        let origin = Origin::Folder(Some(path.clone()));
        let circle = self.element(&origin, "hitcircle", false, 96);
        let overlay = self.element(&origin, "hitcircleoverlay", false, 96);
        // The combo number "1" in the skin's own font (HitCirclePrefix), sized as in osu!:
        // digits keep their size relative to the circle.
        let digit_stem = self.skin_stem(&origin, "default-1");
        let mut digit = self.element(&origin, &digit_stem, false, 64);
        if matches!(digit, Some(State::Missing)) && digit_stem != "default-1" {
            // A prefix whose files are missing: osu! falls back to the default digits.
            digit = self.element(&origin, "default-1", false, 64);
        }
        let drawn = thumb_size * 0.86;
        let overlay_above = self
            .skin(&path)
            .and_then(|s| s.ini.get("General", "HitCircleOverlayAboveNumber"))
            .is_none_or(|v| v.trim() != "0");
        if let Some(State::Ready(e)) = &circle {
            paint_centered(ui, &e.frames[0], thumb.center(), drawn, rgb(color));
        }
        let paint_digit = |ui: &egui::Ui| {
            if let Some(State::Ready(e)) = &digit {
                // osu! scales digits from the nominal 128 px circle, not from the circle's
                // picture: skins with a blank 1 px hitcircle would blow the digit up otherwise.
                let size = (e.logical.x.max(e.logical.y) * drawn / 128.0 * 0.85).min(drawn);
                let texture = &e.frames[0];
                let dims = texture.size_vec2();
                let rect = egui::Rect::from_center_size(
                    thumb.center(),
                    dims * (size / dims.x.max(dims.y)),
                );
                ui.painter().with_clip_rect(thumb).image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        };
        if overlay_above {
            paint_digit(ui);
        }
        if let Some(State::Ready(e)) = &overlay {
            paint_centered(ui, &e.frames[0], thumb.center(), drawn, Color32::WHITE);
        }
        if !overlay_above {
            paint_digit(ui);
        }
        // Name, author and stats.
        let x = thumb.right() + 14.0;
        let show_files = rect.width() >= 500.0;
        let stats_width = if show_files { 150.0 } else { 80.0 };
        let text_width = (rect.right() - stats_width - x - 12.0).max(80.0);
        let title = truncated(ui, &name, font_strong(15.0), INK, text_width);
        let clip = egui::Rect::from_min_size(pos2(x, rect.top()), vec2(text_width, height));
        if self.settings.compact {
            ui.painter().with_clip_rect(clip).galley(
                pos2(x, rect.center().y - title.size().y / 2.0),
                title,
                INK,
            );
        } else {
            ui.painter()
                .with_clip_rect(clip)
                .galley(pos2(x, rect.center().y - 19.0), title, INK);
            let author = if author.is_empty() {
                self.t("Автор не указан", "Unknown author").to_owned()
            } else {
                author
            };
            let galley = truncated(ui, &author, font(12.0), INK_SUBTLE, text_width);
            ui.painter().with_clip_rect(clip).galley(
                pos2(x, rect.center().y + 3.0),
                galley,
                INK_SUBTLE,
            );
        }
        let mut right = rect.right() - 12.0;
        let stats = megabytes(self.settings.language, bytes);
        let r = text_at(
            ui,
            pos2(right, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            &stats,
            font(12.0),
            INK_SUBTLE,
        );
        right = r.left() - 12.0;
        if show_files {
            let files = elements_label(self.settings.language, elements);
            let r = text_at(
                ui,
                pos2(right, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                &files,
                font(12.0),
                INK_SUBTLE,
            );
            right = r.left() - 12.0;
        }
        if hidden && show_files {
            let label = self.t("скрыт", "hidden");
            let galley = ui
                .painter()
                .layout_no_wrap(label.to_owned(), condensed(14.0), INK);
            let badge_rect = egui::Rect::from_min_size(
                pos2(
                    right - galley.size().x - 10.0,
                    rect.center().y - galley.size().y / 2.0 - 1.0,
                ),
                galley.size() + vec2(10.0, 2.0),
            );
            ui.painter().rect_filled(badge_rect, RADIUS_SM, BG_400);
            ui.painter()
                .galley(badge_rect.min + vec2(5.0, 1.0), galley, INK_MUTED);
        } else if hidden {
            egui::Image::new(icons::eye_off()).tint(INK_MUTED).paint_at(
                ui,
                egui::Rect::from_center_size(pos2(right - 8.0, rect.center().y), Vec2::splat(16.0)),
            );
        }
        focus_ring(ui, &response, rect.shrink(3.0));
        let response = response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text_at_pointer(&name);
        if response.clicked() {
            if ui.input(|i| i.modifiers.command) {
                if selected {
                    self.selected.remove(&path);
                } else {
                    self.selected.insert(path.clone());
                }
            }
            self.focus_skin = Some(path.clone());
            self.focus_element = None;
        }
        if response.double_clicked() {
            self.report(open::that(&path).map_err(anyhow::Error::from));
        }
    }

    fn export_selected(&mut self, ctx: &egui::Context) {
        let Some(path) = self.selected.first().cloned() else {
            return;
        };
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            + ".osk";
        if let Some(destination) = rfd::FileDialog::new()
            .set_file_name(name)
            .add_filter("osu! skin", &["osk"])
            .save_file()
        {
            let library = self.library.clone().unwrap();
            self.spawn(ctx, move |cancel, _| {
                library.export(&path, &destination, &cancel)?;
                Ok(Outcome::Exported(destination))
            });
        }
    }

    pub(crate) fn history_page(&mut self, ui: &mut egui::Ui) {
        struct Row {
            id: String,
            kind: String,
            label: String,
            time: u64,
            changes: usize,
            undone: bool,
        }
        page_title(
            ui,
            self.t("История", "History"),
            self.t(
                "Все изменения скинов. Оригиналы лежат на диске, так что отмена работает и после перезапуска.",
                "Every change to your skins. Originals stay on disk, so undo works even after a restart.",
            ),
        );
        self.job_bar(ui);
        self.notice_bar(ui);
        let entries: Vec<usize> = (0..self.history.len())
            .rev()
            .filter(|&i| self.history[i].kind != "undo")
            .collect();
        if entries.is_empty() {
            ui.add_space(24.0);
            hit_mark(ui, 44.0);
            ui.add_space(8.0);
            ui.label(
                RichText::new(self.t("Собери первый скин", "Mix your first skin"))
                    .font(condensed(30.0)),
            );
            ui.label(
                RichText::new(self.t(
                    "Всё, что ты меняешь, появится здесь — и всё можно будет отменить.",
                    "Everything you change shows up here, and all of it can be undone.",
                ))
                .color(INK_MUTED),
            );
            ui.add_space(12.0);
            if btn(Kind::Primary, self.t("Собрать скин", "Mix skins"))
                .icon(icons::blend())
                .show(ui)
                .clicked()
            {
                self.go(Page::Mix);
            }
            return;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Only the rows on screen are laid out; the history can hold thousands.
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, 60.0, entries.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for &index in &entries[range] {
                    // Only what the row shows: an operation may list thousands of files.
                    let operation = {
                        let o = &self.history[index];
                        Row {
                            id: o.id.clone(),
                            kind: o.kind.clone(),
                            label: o.label.clone(),
                            time: o.time,
                            changes: o.changes.len(),
                            undone: o.undone,
                        }
                    };
                    let blocked = self.history_blocked.get(index).copied().unwrap_or(false);
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 60.0), Sense::hover());
                    if response.hovered() {
                        ui.painter().rect_filled(rect, RADIUS_SM, BG_300);
                    }
                    hairline(ui, rect.bottom() - 0.5, rect.left()..=rect.right());
                    let label = operation_label(&operation.kind, self.settings.language);
                    let kind_rect = text_at(
                        ui,
                        pos2(rect.left() + 12.0, rect.center().y - 9.0),
                        egui::Align2::LEFT_CENTER,
                        label,
                        font_strong(14.0),
                        INK,
                    );
                    let galley =
                        ui.painter()
                            .layout_no_wrap(operation.label.clone(), font(14.0), INK_MUTED);
                    ui.painter()
                        .with_clip_rect(egui::Rect::from_min_max(
                            pos2(kind_rect.right() + 8.0, rect.top()),
                            pos2(rect.right() - 160.0, rect.bottom()),
                        ))
                        .galley(
                            pos2(kind_rect.right() + 8.0, kind_rect.top()),
                            galley,
                            INK_MUTED,
                        );
                    let meta = format!(
                        "{} · {}",
                        relative_time(self.settings.language, now.saturating_sub(operation.time)),
                        self.settings.language.count(
                            operation.changes,
                            ["изменение", "изменения", "изменений"],
                            ["change", "changes"]
                        )
                    );
                    response.widget_info(|| egui::WidgetInfo::labeled(
                        egui::Role::Label,
                        ui.is_enabled(),
                        format!("{label}: {}, {meta}", operation.label),
                    ));
                    let meta_rect = text_at(
                        ui,
                        pos2(rect.left() + 12.0, rect.center().y + 11.0),
                        egui::Align2::LEFT_CENTER,
                        &meta,
                        font(12.0),
                        INK_SUBTLE,
                    );
                    // kohu!'s ChangeList: what happened in this session is marked «Новое».
                    if operation.time >= session_start() && !operation.undone {
                        let label = self.t("Новое", "New");
                        let galley =
                            ui.painter()
                                .layout_no_wrap(label.to_owned(), font_strong(11.0), ACCENT_TEXT);
                        let badge = egui::Rect::from_min_size(
                            pos2(meta_rect.right() + 8.0, meta_rect.center().y - 9.0),
                            galley.size() + vec2(12.0, 4.0),
                        );
                        ui.painter().rect_filled(badge, RADIUS_SM, ACCENT_SOFT);
                        ui.painter()
                            .galley(badge.min + vec2(6.0, 2.0), galley, ACCENT_TEXT);
                    }
                    let button_rect =
                        egui::Rect::from_min_max(pos2(rect.right() - 150.0, rect.top()), rect.max);
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(button_rect)
                            .layout(Layout::right_to_left(Align::Center)),
                    );
                    child.add_space(8.0);
                    if operation.undone {
                        child.label(
                            RichText::new(self.t("Отменено", "Undone"))
                                .color(SUCCESS)
                                .size(13.0),
                        );
                        child.add(
                            egui::Image::new(icons::check())
                                .tint(SUCCESS)
                                .fit_to_exact_size(Vec2::splat(16.0)),
                        );
                    } else {
                        let undo = btn(Kind::Secondary, self.t("Отменить", "Undo"))
                            .icon(icons::undo_2())
                            .enabled(!blocked)
                            .show(&mut child)
                            .accessible_name(format!("{}: {label} — {}", self.t("Отменить", "Undo"), operation.label))
                            .on_hover_text(if blocked {
                                self.t(
                                    "Сначала отмени более позднее изменение этого скина.",
                                    "Undo the newer change to this skin first.",
                                )
                            } else {
                                self.t("Вернуть исходные файлы", "Restore the original files")
                            });
                        if undo.clicked()
                            && let Some(library) = self.library.clone()
                        {
                            let id = operation.id.clone();
                            self.spawn(ui.ctx(), move |_, _| {
                                library.undo(&id)?;
                                Ok(Outcome::Undone)
                            });
                        }
                    }
                }
            });
    }

    pub(crate) fn settings_page(&mut self, ui: &mut egui::Ui) {
        page_title(ui, self.t("Настройки", "Settings"), "");
        self.notice_bar(ui);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(640.0);
            section_title(ui, self.t("Библиотека osu!", "osu! library"));
            ui.add_space(4.0);
            let folder_label = ui.label(RichText::new(self.t("Папка osu! (внутри должна быть Skins)", "osu! folder (must contain Skins)")).font(font_strong(13.0)));
            ui.horizontal_wrapped(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.folder_input)
                        .desired_width((ui.available_width() - 210.0).max(160.0))
                        .margin(egui::Margin::symmetric(10, 7)),
                ).labelled_by(folder_label.id);
                if btn(Kind::Secondary, self.t("Применить", "Apply")).show(ui).clicked() {
                    self.set_folder(ui.ctx());
                }
                if icon_button(ui, icons::folder_search(), self.t("Выбрать папку…", "Browse…")).clicked()
                    && let Some(folder) = rfd::FileDialog::new().pick_folder()
                {
                    self.folder_input = display_path(&folder);
                    self.set_folder(ui.ctx());
                }
            });
            ui.add_space(8.0);
            let mut changed = false;
            let language = self.settings.language;
            changed |= toggle(ui, &mut self.settings.auto_refresh, language.text(
                "Замечать новые скины автоматически",
                "Notice new skins automatically",
            )).changed();
            changed |= toggle(ui, &mut self.settings.compact, language.text("Компактный список скинов", "Compact skin list")).changed();

            ui.add_space(32.0);
            section_title(ui, self.t("Интерфейс и звук", "Interface and sound"));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(language.text("Язык", "Language")).font(font_strong(13.0)));
                ui.add_space(8.0);
                let mut lang = self.settings.language;
                if tabs(ui, &mut lang, &[(Language::Ru, "Русский"), (Language::En, "English")]) {
                    self.settings.language = lang;
                    changed = true;
                }
            });
            ui.add_space(4.0);
            if toggle(ui, &mut self.settings.reduce_motion, language.text("Меньше анимации", "Reduce motion")).changed() {
                changed = true;
                let time = if self.settings.reduce_motion { 0.0 } else { 0.12 };
                ui.ctx().style_mut_of(egui::Theme::Dark, |s| s.animation_time = time);
            }
            ui.add_space(8.0);
            // Sliders read in percent, and their filled part is the pink state line.
            ui.visuals_mut().selection.bg_fill = ACCENT_LINE;
            let percent = |v: f64, _: std::ops::RangeInclusive<usize>| format!("{:.0} %", v * 100.0);
            let parse = |text: &str| {
                text.trim()
                    .trim_end_matches('%')
                    .trim()
                    .replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .map(|v| v / 100.0)
            };
            let volume = ui.add(
                egui::Slider::new(&mut self.settings.volume, 0.0..=1.0)
                    .custom_formatter(percent)
                    .custom_parser(parse)
                    .text(language.text("Громкость звуков", "Sound volume")),
            );
            if volume.changed()
                && let Some(player) = &self.player
            {
                player.set_volume(self.settings.volume);
            }
            // Saved when the drag ends: settings are synced to disk, not once per frame.
            changed |= volume.drag_stopped() || (volume.changed() && !volume.dragged());
            let scale = ui.add(
                egui::Slider::new(&mut self.settings.ui_scale, 0.8..=1.6)
                    .step_by(0.05)
                    .custom_formatter(percent)
                    .custom_parser(parse)
                    .text(language.text("Масштаб интерфейса", "Interface scale")),
            );
            ui.visuals_mut().selection.bg_fill = ACCENT_SOFT;
            if scale.drag_stopped() || (scale.changed() && !scale.dragged()) {
                ui.ctx().set_zoom_factor(self.settings.ui_scale);
                changed = true;
            }
            if changed {
                self.save_settings();
            }

            ui.add_space(32.0);
            section_title(ui, self.t("Восстановление", "Recovery"));
            ui.add_space(4.0);
            ui.label(RichText::new(self.t(
                "Резервные копии лежат в .osu-skin-mixer-backups рядом с Skins и сами не удаляются.",
                "Backups live in .osu-skin-mixer-backups next to Skins and are never deleted automatically.",
            )).color(INK_MUTED));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if btn(Kind::Secondary, self.t("Папка восстановления", "Recovery folder")).icon(icons::folder_open()).show(ui).clicked()
                    && let Some(library) = &self.library
                {
                    let result = open::that(library.root.join(".osu-skin-mixer-backups")).map_err(anyhow::Error::from);
                    self.report(result);
                }
                if btn(Kind::Ghost, self.t("Журнал ошибок", "Error log")).icon(icons::file_cog()).show(ui).clicked() {
                    let result = open::that(&self.data).map_err(anyhow::Error::from);
                    self.report(result);
                }
                if btn(Kind::Ghost, self.t("Сообщить о проблеме", "Report a problem")).icon(icons::external_link()).show(ui).clicked() {
                    self.report(open::that("https://github.com/himawari17/Skinweaver/issues").map_err(anyhow::Error::from));
                }
            });
            ui.add_space(32.0);
            ui.label(caption(format!(
                "Skinweaver {}\n{}",
                env!("CARGO_PKG_VERSION"),
                self.t(
                    "Основано на osu! Skin Mixer от rednir (MIT). Стандартные элементы — ppy/osu-resources (CC BY-NC 4.0).\nШрифты Onest, Cormorant Garamond и JetBrains Mono — OFL. Иконки Lucide — ISC.",
                    "Based on osu! Skin Mixer by rednir (MIT). Default elements from ppy/osu-resources (CC BY-NC 4.0).\nFonts Onest, Cormorant Garamond and JetBrains Mono — OFL. Lucide icons — ISC.",
                )
            )));
        });
    }
}

fn operation_label(kind: &str, language: Language) -> &str {
    match kind {
        "mix" => language.text("Новый микс", "New mix"),
        "modify" => language.text("Изменение", "Modification"),
        "hide" => language.text("Видимость", "Visibility"),
        "rename" => language.text("Переименование", "Rename"),
        "duplicate" => language.text("Копия", "Duplicate"),
        "delete" => language.text("Удаление", "Deletion"),
        _ => kind,
    }
}

fn relative_time(language: Language, seconds: u64) -> String {
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    if minutes < 1 {
        language.text("только что", "just now").into()
    } else if hours < 1 {
        format!(
            "{} {}",
            language.count(
                minutes as usize,
                ["минуту", "минуты", "минут"],
                ["minute", "minutes"]
            ),
            language.text("назад", "ago")
        )
    } else if days < 1 {
        format!(
            "{} {}",
            language.count(hours as usize, ["час", "часа", "часов"], ["hour", "hours"]),
            language.text("назад", "ago")
        )
    } else if days == 1 {
        language.text("вчера", "yesterday").into()
    } else if days < 7 {
        format!(
            "{} {}",
            language.count(days as usize, ["день", "дня", "дней"], ["day", "days"]),
            language.text("назад", "ago")
        )
    } else {
        // Older entries get a date in words: "12 окт.".
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        date_words(language, now.saturating_sub(seconds))
    }
}

pub(crate) fn date_words(language: Language, unix: u64) -> String {
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm), UTC.
    let z = (unix / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as usize;
    let ru = [
        "янв.",
        "февр.",
        "марта",
        "апр.",
        "мая",
        "июня",
        "июля",
        "авг.",
        "сент.",
        "окт.",
        "нояб.",
        "дек.",
    ];
    let en = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    match language {
        Language::Ru => format!("{day} {}", ru[month - 1]),
        Language::En => format!("{} {day}", en[month - 1]),
    }
}

/// When this run of the app started, in Unix seconds.
pub(crate) fn session_start() -> u64 {
    static START: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *START.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(1)
    })
}
