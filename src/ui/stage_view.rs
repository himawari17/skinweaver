//! "By stage" mode of the mixer: a stage tree, the stage scene with its sounds,
//! and big variant tiles in the preview panel.

use super::{
    editor::{Origin, Pick, audio_stem, has_audio, node_at, parse_color, representative},
    icons,
    scenes::{H, W},
    stages::{HIT_SETS, Look, STAGES, SceneKind, StageDef, also_on, related},
    textures::{self, State},
    theme::*,
    *,
};
use skinweaver::catalog::Target;

/// What a variant tile shows for a node.
enum Show {
    Images(Vec<String>, Layout2),
    Colors(Vec<String>),
    Value(String, String),
    Sound(String),
    Nothing,
}

#[derive(Clone, Copy, PartialEq)]
enum Layout2 {
    Stack,
    Corner,
    Row,
}

#[derive(Clone, Copy, PartialEq)]
enum Presence {
    Own,
    Fallback,
    Missing,
    Loading,
}

impl App {
    pub(crate) fn stage_mode(&self) -> bool {
        self.page == Page::Mix && self.stage_view
    }

    /// Shows the focused element in the preview: variants in stage mode, the comparison otherwise.
    pub(crate) fn reveal_element(&mut self) {
        if self.stage_mode() {
            if self.preview_tab != PreviewTab::Element {
                self.preview_tab = PreviewTab::Variants;
            }
        } else {
            self.preview_tab = PreviewTab::Element;
        }
    }

    pub(crate) fn focus_node(&mut self, id: &str) {
        let editor = self.editor_mut();
        editor.focus = Some(id.to_owned());
        editor.picked.clear();
        editor.anchor = Some(id.to_owned());
        self.reveal_element();
    }

    /// A click on a row or chip of a list: Ctrl and Shift pick several elements.
    fn click_listed(&mut self, ui: &egui::Ui, id: &str, sound: bool) {
        let how = Pick::from(ui);
        if sound && how == Pick::One {
            self.play_node(id);
        }
        let order = self.mix.row_order_prev.clone();
        self.pick(id, how, &order);
    }

    /// Opens a stage; the tree works as an accordion so later stages stay in reach.
    fn go_stage(&mut self, index: usize) {
        if self.stage != index && self.mix.picked.is_empty() {
            // A new screen starts with nothing picked; the panel invites a click.
            // A Ctrl selection survives, so elements of several screens can change together.
            self.mix.focus = None;
        }
        self.stage = index.min(STAGES.len() - 1);
        self.stage_open.clear();
        self.stage_open.insert(self.stage);
    }

    fn changed(&self, id: &str) -> bool {
        self.mix.choices.keys().any(|k| related(k, id))
    }

    fn stage_ids(&self, stage: &StageDef) -> Vec<String> {
        let mut keys: Vec<&str> = stage.items.to_vec();
        for group in stage.sounds {
            match group.group {
                Some(key) => keys.push(key),
                None => keys.extend(group.leaves),
            }
        }
        keys.into_iter()
            .filter_map(|k| self.stage_index.id(k).map(str::to_owned))
            .collect()
    }

    // ------------------------------------------------------------ page body

    pub(crate) fn stage_body(&mut self, ui: &mut egui::Ui, height: f32) {
        let width = ui.available_width();
        let compact = width < 820.0;
        let tree_width = if compact {
            64.0
        } else {
            (width * 0.26).clamp(220.0, 290.0)
        };
        ui.allocate_ui_with_layout(
            vec2(width, height),
            Layout::left_to_right(Align::Min),
            |ui| {
                ui.allocate_ui_with_layout(
                    vec2(tree_width, height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.set_min_size(vec2(tree_width, height));
                        egui::ScrollArea::vertical()
                            .id_salt("stage_tree")
                            .auto_shrink([false, false])
                            .max_height(height)
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                self.stage_tree(ui, compact);
                            });
                    },
                );
                let x = ui.cursor().left() + 10.0;
                ui.painter().vline(
                    x,
                    ui.cursor().top()..=ui.cursor().top() + height,
                    Stroke::new(1.0, LINE_100),
                );
                ui.add_space(20.0);
                let rest = ui.available_width();
                ui.allocate_ui_with_layout(
                    vec2(rest, height),
                    Layout::top_down(Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("stage_main")
                            .auto_shrink([false, false])
                            .max_height(height)
                            .show(ui, |ui| {
                                let stage = &STAGES[self.stage];
                                self.stage_header(ui, stage);
                                ui.add_space(10.0);
                                let available = ui.available_width();
                                let reserve = if stage.scene == SceneKind::Play {
                                    170.0
                                } else {
                                    120.0
                                };
                                let fit = ((height - 60.0 - reserve).max(180.0)) * W / H;
                                self.stage_scene(ui, stage.scene, available.min(fit));
                                ui.add_space(14.0);
                                if compact {
                                    self.element_strip(ui, stage);
                                    ui.add_space(10.0);
                                }
                                self.sound_strip(ui, stage);
                                ui.add_space(14.0);
                                self.stage_steps(ui);
                            });
                    },
                );
            },
        );
    }

    fn stage_header(&mut self, ui: &mut egui::Ui, stage: &StageDef) {
        let roomy = ui.available_width() >= 560.0;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(0.0, 30.0), Sense::hover());
            let tab = code_tab(ui, pos2(r.left(), r.center().y - 9.0), stage.code, true);
            ui.add_space(tab.width() + 6.0);
            ui.add(
                egui::Label::new(RichText::new(self.t(stage.ru, stage.en)).font(condensed(28.0)))
                    .truncate(),
            );
            if roomy {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    self.stage_source_picker(ui, stage);
                });
            }
        });
        if !roomy {
            self.stage_source_picker(ui, stage);
            ui.add_space(4.0);
        }
        ui.label(caption(self.t(
            "Выбери элемент сцены или списка — его варианты появятся в предпросмотре. Ctrl+клик выбирает несколько элементов, Shift+клик — диапазон. Круглые значки проигрывают звук.",
            "Pick an element in the scene or list to see its variants in the preview. Ctrl+click selects several elements, Shift+click selects a range. Round badges play sounds.",
        )));
    }

    /// "Take the whole stage from…": one choice for every element and sound of the stage.
    fn stage_source_picker(&mut self, ui: &mut egui::Ui, stage: &StageDef) {
        let language = self.settings.language;
        let mut chosen = None;
        egui::ComboBox::from_id_salt(("stage_from", stage.code))
            .icon(combo_chevron)
            .width(200.0)
            .height(380.0)
            .truncate()
            .selected_text(
                RichText::new(language.text("Весь этап из…", "Whole stage from…")).size(13.5),
            )
            .show_ui(ui, |ui| {
                ui.set_min_width(260.0);
                let query_id = ui.make_persistent_id(("stage_from_query", stage.code));
                let mut query = ui.data_mut(|d| d.get_temp::<String>(query_id).unwrap_or_default());
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
                    .selectable_label(false, language.text("Стандартный скин", "Default skin"))
                    .clicked()
                {
                    chosen = Some(Source::Default);
                }
                ui.separator();
                let query = query.to_lowercase();
                for skin in &self.skins {
                    if !query.is_empty() && !skin.name.to_lowercase().contains(&query) {
                        continue;
                    }
                    if ui.selectable_label(false, &skin.name).clicked() {
                        chosen = Some(Source::Skin(skin.path.clone()));
                    }
                }
                if chosen.is_some() {
                    ui.close();
                }
            });
        if let Some(source) = chosen {
            let focus = self.mix.focus.clone();
            for id in self.stage_ids(stage) {
                self.set_choice(&id, source.clone());
            }
            self.mix.focus = focus;
        }
    }

    // ------------------------------------------------------------ tree

    fn stage_tree(&mut self, ui: &mut egui::Ui, compact: bool) {
        let russian = self.ru();
        for (index, stage) in STAGES.iter().enumerate() {
            let current = self.stage == index;
            let open = self.stage_open.contains(&index);
            let height = if stage.depth == 0 { 52.0 } else { 44.0 };
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::Role::Tab, ui.is_enabled(), current, self.t(stage.ru, stage.en))
            });
            let ids = self.stage_ids(stage);
            let changed = ids.iter().filter(|id| self.changed(id)).count();
            if ui.is_rect_visible(rect) {
                let fill = if current {
                    ACCENT_SOFT
                } else if response.hovered() {
                    BG_300
                } else {
                    Color32::TRANSPARENT
                };
                ui.painter().rect_filled(rect, 0, fill);
                if stage.depth == 0 {
                    ui.painter().hline(
                        rect.x_range(),
                        rect.top() + 0.5,
                        Stroke::new(1.0, LINE_200),
                    );
                } else {
                    hairline(ui, rect.top() + 0.5, rect.left() + 20.0..=rect.right());
                }
                let indent = if compact {
                    0.0
                } else {
                    stage.depth as f32 * 18.0
                };
                let code = text_at(
                    ui,
                    pos2(rect.left() + 6.0 + indent, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    stage.code,
                    condensed(if stage.depth == 0 { 24.0 } else { 18.0 }),
                    if current { ACCENT_TEXT } else { INK },
                );
                if changed > 0 {
                    ui.painter().circle_filled(
                        pos2(code.right() + 5.0, code.top() + 5.0),
                        3.5,
                        ACCENT_LINE,
                    );
                }
                if !compact {
                    let x = rect.left() + 50.0 + indent;
                    let width = rect.right() - x - 30.0;
                    let title = truncated(
                        ui,
                        if russian { stage.ru } else { stage.en },
                        if stage.depth == 0 {
                            condensed(21.0)
                        } else {
                            font_strong(14.0)
                        },
                        INK,
                        width,
                    );
                    let meta = if changed == 0 {
                        self.t("без изменений", "unchanged").to_owned()
                    } else {
                        format!(
                            "{} {} {}",
                            self.t("изменено", "changed"),
                            changed,
                            self.t(&format!("из {}", ids.len()), &format!("of {}", ids.len()))
                        )
                    };
                    let meta = truncated(ui, &meta, font(12.0), INK_SUBTLE, width);
                    let block = title.size().y + meta.size().y - 2.0;
                    let top = rect.center().y - block / 2.0;
                    ui.painter().galley(pos2(x, top), title.clone(), INK);
                    ui.painter()
                        .galley(pos2(x, top + title.size().y - 2.0), meta, INK_SUBTLE);
                    let icon = if open {
                        icons::chevron_down()
                    } else {
                        icons::chevron_right()
                    };
                    egui::Image::new(icon).tint(INK_MUTED).paint_at(
                        ui,
                        egui::Rect::from_center_size(
                            pos2(rect.right() - 14.0, rect.center().y),
                            Vec2::splat(16.0),
                        ),
                    );
                }
            }
            focus_ring(ui, &response, rect.shrink(3.0));
            let response = if compact {
                response.on_hover_text(self.t(stage.ru, stage.en))
            } else {
                response
            };
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                if current {
                    if !self.stage_open.remove(&index) {
                        self.stage_open.insert(index);
                    }
                } else {
                    self.go_stage(index);
                }
            }
            if open && !compact {
                for key in stage.items {
                    if let Some(id) = self.stage_index.id(key).map(str::to_owned) {
                        self.stage_item_row(ui, index, &id, false, stage.depth);
                    }
                }
                for (g, group) in stage.sounds.iter().enumerate() {
                    if let Some(id) = group
                        .group
                        .and_then(|k| self.stage_index.id(k))
                        .map(str::to_owned)
                    {
                        self.stage_item_row(ui, index, &id, true, stage.depth);
                        continue;
                    }
                    // Loose sounds fold into one row so the tree stays short.
                    let ids: Vec<String> = group
                        .leaves
                        .iter()
                        .filter_map(|k| self.stage_index.id(k).map(str::to_owned))
                        .collect();
                    let label = if stage.sounds.len() == 1 {
                        self.t("Звуки этапа", "Stage sounds").to_owned()
                    } else {
                        format!(
                            "{} · {}",
                            self.t("Звуки", "Sounds"),
                            if russian { group.ru } else { group.en }
                        )
                    };
                    let unfolded = self.stage_sounds_open.contains(&(index, g));
                    if self.sound_fold_row(ui, &label, &ids, unfolded, stage.depth)
                        && !self.stage_sounds_open.remove(&(index, g))
                    {
                        self.stage_sounds_open.insert((index, g));
                    }
                    if unfolded {
                        for id in &ids {
                            self.stage_item_row(ui, index, id, true, stage.depth + 1);
                        }
                    }
                }
                ui.add_space(6.0);
            }
        }
        ui.add_space(12.0);
        if !compact {
            ui.spacing_mut().item_spacing.y = 4.0;
            ui.label(caption(self.t(
                "Этапы можно пропускать: всё, что не тронуто, останется из стандартного скина. Редкие элементы — во вкладке «Списком».",
                "Skip any stage: whatever you leave alone comes from the default skin. Rare elements live in the List tab.",
            )));
        }
    }

    fn sound_fold_row(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        ids: &[String],
        unfolded: bool,
        depth: u8,
    ) -> bool {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
        if ui.is_rect_visible(rect) {
            if response.hovered() {
                ui.painter().rect_filled(rect, 0, BG_300);
            }
            let changed = ids.iter().filter(|id| self.changed(id)).count();
            let mut x = rect.left() + 18.0 + depth as f32 * 18.0;
            if changed > 0 {
                ui.painter()
                    .circle_filled(pos2(x - 8.0, rect.center().y), 3.0, ACCENT_LINE);
            }
            let thumb =
                egui::Rect::from_min_size(pos2(x, rect.center().y - 14.0), Vec2::splat(28.0));
            egui::Image::new(icons::audio_lines()).tint(INK).paint_at(
                ui,
                egui::Rect::from_center_size(thumb.center(), Vec2::splat(16.0)),
            );
            x += 36.0;
            let width = rect.right() - x - 28.0;
            let title = truncated(ui, label, font_strong(13.5), INK, width);
            let meta = if changed > 0 {
                format!(
                    "{} · {} {changed}",
                    self.settings.language.count(
                        ids.len(),
                        ["звук", "звука", "звуков"],
                        ["sound", "sounds"]
                    ),
                    self.t("изменено", "changed")
                )
            } else {
                self.settings.language.count(
                    ids.len(),
                    ["звук", "звука", "звуков"],
                    ["sound", "sounds"],
                )
            };
            let meta = truncated(ui, &meta, font(11.5), INK_SUBTLE, width);
            let block = title.size().y + meta.size().y - 3.0;
            let top = rect.center().y - block / 2.0;
            ui.painter().galley(pos2(x, top), title.clone(), INK);
            ui.painter()
                .galley(pos2(x, top + title.size().y - 3.0), meta, INK_SUBTLE);
            let icon = if unfolded {
                icons::chevron_down()
            } else {
                icons::chevron_right()
            };
            egui::Image::new(icon).tint(INK_MUTED).paint_at(
                ui,
                egui::Rect::from_center_size(
                    pos2(rect.right() - 14.0, rect.center().y),
                    Vec2::splat(14.0),
                ),
            );
        }
        focus_ring(ui, &response, rect.shrink(3.0));
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
    }

    fn stage_item_row(
        &mut self,
        ui: &mut egui::Ui,
        stage: usize,
        id: &str,
        sound: bool,
        depth: u8,
    ) {
        let nodes = self.nodes.clone();
        let Some(node) = node_at(&nodes, id) else {
            return;
        };
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 38.0), Sense::click());
        let focused = self.mix.focus.as_deref() == Some(id) || self.is_picked(id);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), focused, node.label(self.ru()))
        });
        self.mix.row_order.push(id.to_owned());
        let source = self.effective(id);
        let prefix = format!("{id}.");
        let mixed = self.mix.choices.keys().any(|k| k.starts_with(&prefix));
        let changed = self.changed(id);
        if ui.is_rect_visible(rect) {
            let fill = if self.is_picked(id) {
                ACCENT_SOFT
            } else if focused {
                BG_400
            } else if response.hovered() {
                BG_300
            } else {
                Color32::TRANSPARENT
            };
            ui.painter().rect_filled(rect, 0, fill);
            if focused {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(rect.min, vec2(2.0, rect.height())),
                    0,
                    ACCENT_LINE,
                );
            }
            let mut x = rect.left() + 18.0 + depth as f32 * 18.0;
            if changed {
                ui.painter()
                    .circle_filled(pos2(x - 8.0, rect.center().y), 3.0, ACCENT_LINE);
            }
            let thumb =
                egui::Rect::from_min_size(pos2(x, rect.center().y - 14.0), Vec2::splat(28.0));
            if sound {
                egui::Image::new(icons::audio_lines()).tint(INK).paint_at(
                    ui,
                    egui::Rect::from_center_size(thumb.center(), Vec2::splat(16.0)),
                );
            } else {
                self.row_thumb(ui, node, &source, thumb, 1);
            }
            x += 36.0;
            let width = rect.right() - x - 6.0;
            let title = truncated(ui, node.label(self.ru()), font(13.5), INK, width);
            let mut from = if mixed {
                self.t("разные источники", "mixed sources").to_owned()
            } else {
                source_name(self.settings.language, &source)
            };
            let also = also_on(&self.stage_index, id, stage);
            if !also.is_empty() {
                from.push_str(&format!(
                    " · {} {}",
                    self.t("ещё на", "also on"),
                    also.join(", ")
                ));
            }
            let meta = truncated(
                ui,
                &from,
                font(11.5),
                if changed { ACCENT_TEXT } else { INK_SUBTLE },
                width,
            );
            let block = title.size().y + meta.size().y - 3.0;
            let top = rect.center().y - block / 2.0;
            ui.painter().galley(pos2(x, top), title.clone(), INK);
            ui.painter()
                .galley(pos2(x, top + title.size().y - 3.0), meta, INK_SUBTLE);
        }
        let also = also_on(&self.stage_index, id, stage);
        focus_ring(ui, &response, rect.shrink(3.0));
        let response = if also.is_empty() {
            response
        } else {
            response.on_hover_text(format!(
                "{} {}. {}",
                self.t(
                    "Этот же элемент виден на этапах",
                    "The same element shows on stages"
                ),
                also.join(", "),
                self.t(
                    "Выбор здесь меняет его везде.",
                    "Picking it here changes it everywhere."
                )
            ))
        };
        if response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            if self.stage != stage {
                self.go_stage(stage);
            }
            self.click_listed(ui, id, sound);
        }
    }

    // ------------------------------------------------------------ sounds

    fn sound_strip(&mut self, ui: &mut egui::Ui, stage: &StageDef) {
        if stage.sounds.is_empty() {
            return;
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.t("Звуки этапа", "Stage sounds")).font(condensed(22.0)));
            ui.add_space(8.0);
            if stage.scene == SceneKind::Play {
                let options: Vec<(usize, &str)> = stage
                    .sounds
                    .iter()
                    .take(HIT_SETS.len())
                    .enumerate()
                    .map(|(i, g)| (i, if self.ru() { g.ru } else { g.en }))
                    .collect();
                let mut set = self.hit_set;
                tabs(ui, &mut set, &options);
                self.hit_set = set;
                ui.label(caption(self.t(
                    "набор, который карта просит у скина",
                    "the set a map asks the skin for",
                )));
            }
        });
        ui.add_space(6.0);
        let groups: Vec<usize> = if stage.scene == SceneKind::Play {
            let mut groups = vec![self.hit_set.min(HIT_SETS.len() - 1)];
            groups.extend(HIT_SETS.len()..stage.sounds.len());
            groups
        } else {
            (0..stage.sounds.len()).collect()
        };
        for g in groups {
            let group = &stage.sounds[g];
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                if stage.sounds.len() > 1 {
                    let label = if self.ru() { group.ru } else { group.en };
                    if let Some(id) = group
                        .group
                        .and_then(|k| self.stage_index.id(k))
                        .map(str::to_owned)
                    {
                        // The group's name picks the whole set at once.
                        let focused = self.mix.focus.as_deref() == Some(id.as_str());
                        if btn(Kind::Ghost, label)
                            .small()
                            .selected(focused)
                            .show(ui)
                            .on_hover_text(
                                self.t("Выбрать весь набор сразу", "Pick the whole set at once"),
                            )
                            .clicked()
                        {
                            self.focus_node(&id);
                        }
                    } else {
                        ui.label(
                            RichText::new(label)
                                .font(font_strong(13.0))
                                .color(INK_MUTED),
                        );
                    }
                }
                for key in group.leaves {
                    if let Some(id) = self.stage_index.id(key).map(str::to_owned) {
                        self.sound_chip(ui, &id);
                    }
                }
            });
            ui.add_space(4.0);
        }
    }

    /// With a narrow window the tree shows only stage codes, so the stage's elements
    /// are offered as chips under the scene instead.
    fn element_strip(&mut self, ui: &mut egui::Ui, stage: &StageDef) {
        ui.label(RichText::new(self.t("Элементы этапа", "Stage elements")).font(condensed(22.0)));
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
            for key in stage.items {
                if let Some(id) = self.stage_index.id(key).map(str::to_owned) {
                    self.chip(ui, &id, false);
                }
            }
        });
    }

    fn sound_chip(&mut self, ui: &mut egui::Ui, id: &str) {
        self.chip(ui, id, true);
    }

    fn chip(&mut self, ui: &mut egui::Ui, id: &str, sound: bool) {
        let nodes = self.nodes.clone();
        let Some(node) = node_at(&nodes, id) else {
            return;
        };
        let label = node.label(self.ru()).to_owned();
        let galley = ui.painter().layout_no_wrap(label, font(13.0), INK);
        let lead = if sound { 30.0 } else { 12.0 };
        let size = vec2(lead + galley.size().x + 14.0, 30.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let focused = self.mix.focus.as_deref() == Some(id) || self.is_picked(id);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), focused, node.label(self.ru()))
        });
        self.mix.row_order.push(id.to_owned());
        let changed = self.changed(id);
        let fill = if focused {
            ACCENT_SOFT
        } else if response.hovered() {
            BG_300
        } else {
            BG_200
        };
        ui.painter().rect(
            rect,
            RADIUS_SM,
            fill,
            Stroke::new(1.0, if focused { ACCENT_LINE } else { LINE_100 }),
            egui::StrokeKind::Inside,
        );
        if sound {
            egui::Image::new(icons::play()).tint(INK).paint_at(
                ui,
                egui::Rect::from_center_size(
                    pos2(rect.left() + 16.0, rect.center().y),
                    Vec2::splat(13.0),
                ),
            );
        }
        ui.painter().galley(
            pos2(rect.left() + lead, rect.center().y - galley.size().y / 2.0),
            galley,
            INK,
        );
        if changed {
            ui.painter().circle_filled(
                pos2(rect.right() - 5.0, rect.top() + 5.0),
                3.0,
                ACCENT_LINE,
            );
        }
        let source = source_name(self.settings.language, &self.effective(id));
        focus_ring(ui, &response, rect.shrink(3.0));
        let hint = if sound {
            self.t("Слушать и выбрать", "Listen and choose")
        } else {
            self.t("Выбрать", "Choose")
        };
        if response
            .on_hover_text(format!("{hint} · {source}"))
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            self.click_listed(ui, id, sound);
        }
    }

    fn stage_steps(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.stage > 0 {
                let prev = &STAGES[self.stage - 1];
                let label = format!(
                    "{} {} · {}",
                    self.t("Назад:", "Back:"),
                    prev.code,
                    self.t(prev.ru, prev.en)
                );
                if btn(Kind::Ghost, &label).show(ui).clicked() {
                    self.go_stage(self.stage - 1);
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.stage + 1 < STAGES.len() {
                    let next = &STAGES[self.stage + 1];
                    let label = format!(
                        "{} {} · {}",
                        self.t("Дальше:", "Next:"),
                        next.code,
                        self.t(next.ru, next.en)
                    );
                    if btn(Kind::Secondary, &label).show(ui).clicked() {
                        self.go_stage(self.stage + 1);
                    }
                }
            });
        });
    }

    // ------------------------------------------------------------ variants

    fn kind_of(&self, id: &str, node: &OptionNode) -> Show {
        if let Some(look) = self.stage_index.look(id) {
            let own = |stems: &[&str]| stems.iter().map(|s| (*s).to_owned()).collect();
            return match look {
                Look::Stack(s) => Show::Images(own(s), Layout2::Stack),
                Look::Corner(s) => Show::Images(own(s), Layout2::Corner),
                Look::Row(s) => Show::Images(own(s), Layout2::Row),
                Look::Colors(s) => Show::Colors(own(s)),
            };
        }
        if has_audio(node) {
            fn first(node: &OptionNode) -> Option<String> {
                node.target
                    .as_ref()
                    .and_then(audio_stem)
                    .or_else(|| node.children.iter().find_map(first))
            }
            return first(node).map_or(Show::Nothing, Show::Sound);
        }
        match &node.target {
            Some(Target::Property { section, key }) if section.eq_ignore_ascii_case("Colours") => {
                Show::Colors(vec![key.clone()])
            }
            Some(Target::Property { section, key }) => Show::Value(section.clone(), key.clone()),
            Some(Target::Section { section, key, .. }) => Show::Value(section.clone(), key.clone()),
            _ => match representative(node) {
                Some((stem, _)) => Show::Images(vec![stem], Layout2::Stack),
                None => Show::Nothing,
            },
        }
    }

    /// A font digit of the skin's own prefix: `default-1` → `fonts/hc-1`.
    pub(crate) fn skin_stem(&self, origin: &Origin, stem: &str) -> String {
        let key = if stem.starts_with("default-") {
            "HitCirclePrefix"
        } else if stem.starts_with("score-") {
            "ScorePrefix"
        } else {
            return stem.to_owned();
        };
        if let Origin::Folder(Some(path)) = origin
            && let Some(prefix) = self
                .skin(path)
                .and_then(|s| s.ini.get("Fonts", key))
                .map(str::trim)
                .filter(|p| !p.is_empty())
            && let Some((_, digit)) = stem.split_once('-')
        {
            return format!("{}-{digit}", prefix.replace('\\', "/"));
        }
        stem.to_owned()
    }

    /// Whether a skin has its own file for an element, from the skin's file list (built off
    /// the UI thread). `None` while the list is being built. Answers are remembered until
    /// the library is rescanned.
    fn skin_has_file(
        &mut self,
        path: &std::path::Path,
        key: &str,
        check: impl Fn(&textures::FolderIndex) -> bool,
    ) -> Option<bool> {
        let memo = (path.to_path_buf(), key.to_owned());
        if let Some(&own) = self.own_files.get(&memo) {
            return Some(own);
        }
        let index = self.textures.index(path)?;
        let own = check(index.get()?);
        self.own_files.insert(memo, own);
        Some(own)
    }

    fn presence(&mut self, origin: &Origin, kind: &Show, max: u32) -> Presence {
        let path = match origin {
            Origin::Blank => return Presence::Missing,
            Origin::Folder(None) => return Presence::Own,
            Origin::Folder(Some(path)) => path.clone(),
        };
        match kind {
            Show::Images(stems, _) => {
                // Only the folder's file list is needed, not the image: tiles that are not on
                // screen never load anything (the list is built off the UI thread).
                let stem = self.skin_stem(origin, &stems[0]);
                let animated = self.stage_index.animated(&stem);
                let _ = max;
                let own = self.skin_has_file(&path, &stem, |i| i.has_image(&stem, animated));
                match own {
                    None => Presence::Loading,
                    Some(true) => Presence::Own,
                    Some(false) if textures::default_has_image(&stem, animated) => {
                        Presence::Fallback
                    }
                    Some(false) => Presence::Missing,
                }
            }
            Show::Colors(keys) => {
                let has = self
                    .skin(&path)
                    .is_some_and(|s| s.ini.get("Colours", &keys[0]).is_some());
                if has {
                    Presence::Own
                } else {
                    Presence::Fallback
                }
            }
            Show::Value(section, key) => {
                let has = self
                    .skin(&path)
                    .is_some_and(|s| s.ini.get(section, key).is_some());
                if has {
                    Presence::Own
                } else {
                    Presence::Fallback
                }
            }
            Show::Sound(stem) => {
                match self.skin_has_file(&path, &format!("snd:{stem}"), |i| i.has_sound(stem)) {
                    None => Presence::Loading,
                    Some(true) => Presence::Own,
                    Some(false) => Presence::Fallback,
                }
            }
            Show::Nothing => Presence::Missing,
        }
    }

    /// The preview panel in stage mode.
    pub(crate) fn variants_panel(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.mix.focus.clone() else {
            ui.add_space(8.0);
            hit_mark(ui, 40.0);
            ui.add_space(8.0);
            ui.label(
                RichText::new(self.t("Нажми на элемент сцены", "Click an element of the scene"))
                    .font(font_strong(16.0)),
            );
            ui.label(
                RichText::new(self.t(
                    "или выбери его в дереве этапов. Здесь появятся его варианты из всех твоих скинов — крупно, чтобы было видно каждую деталь.",
                    "or pick it in the stage tree. Its variants from all your skins will show up here, large enough to see every detail.",
                ))
                .color(INK_MUTED),
            );
            return;
        };
        let nodes = self.nodes.clone();
        let Some(node) = node_at(&nodes, &id) else {
            return;
        };
        let russian = self.ru();
        // Breadcrumb: every parent group is one click away.
        let parts: Vec<&str> = id.split('.').collect();
        if parts.len() > 1 {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for depth in 1..parts.len() {
                    let parent = parts[..depth].join(".");
                    if let Some(group) = node_at(&nodes, &parent) {
                        let link = ui.add(
                            egui::Label::new(caption(group.label(russian))).sense(Sense::click()),
                        );
                        if link
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .on_hover_text(self.t("Выбрать всю группу", "Pick the whole group"))
                            .clicked()
                        {
                            self.focus_node(&parent);
                        }
                        ui.label(caption("›"));
                    }
                }
            });
        }
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(0.0, 30.0), Sense::hover());
            let tab = code_tab(
                ui,
                pos2(r.left(), r.center().y - 9.0),
                &super::editor::element_code(&id),
                true,
            );
            ui.add_space(tab.width() + 4.0);
            ui.add(
                egui::Label::new(RichText::new(node.label(russian)).font(condensed(28.0)))
                    .truncate(),
            );
        });
        let prefix = format!("{id}.");
        let mixed = self.mix.choices.keys().any(|k| k.starts_with(&prefix));
        let current = self.effective(&id);
        ui.label(caption(format!(
            "{}: {}",
            self.t("Сейчас", "Now"),
            if mixed {
                self.t("разные источники", "mixed sources").to_owned()
            } else {
                source_name(self.settings.language, &current)
            }
        )));
        ui.add_space(10.0);
        self.picked_box(ui, &id);
        let kind = self.kind_of(&id, node);
        // Controls.
        let hint = self.t("Найти скин…", "Find a skin…");
        ui.add(
            egui::TextEdit::singleline(&mut self.variant_query)
                .prefix(
                    egui::Image::new(icons::search())
                        .tint(INK_SUBTLE)
                        .fit_to_exact_size(Vec2::splat(16.0)),
                )
                .hint_text(hint)
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(10, 7)),
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if !matches!(kind, Show::Sound(_)) {
                let options = [
                    (1usize, self.t("Крупно", "Large")),
                    (2, self.t("Средне", "Medium")),
                    (3, self.t("Мелко", "Small")),
                ];
                tabs(ui, &mut self.variant_columns, &options);
            }
        });
        ui.add_space(2.0);
        let label = self.t(
            "Только скины, где есть свой файл",
            "Only skins with their own file",
        );
        toggle(ui, &mut self.variant_own_only, label);
        ui.add_space(8.0);
        // Options: the default skin, blank, then every skin.
        let query = self.variant_query.to_lowercase();
        // (source, name, code tab, pinned): the default and blank first, then the skins
        // this element came from before, recent picks and the base, then everything else.
        let mut options: Vec<(Source, String, String, bool)> = vec![(
            Source::Default,
            self.t("Стандартный скин", "Default skin").to_owned(),
            String::new(),
            true,
        )];
        if !matches!(kind, Show::Colors(_) | Show::Value(..)) {
            options.push((
                Source::Blank,
                self.t("Пустой элемент", "Blank element").to_owned(),
                String::new(),
                true,
            ));
        }
        let pinned = self.pinned_skins(&id);
        let matches = |skin: &skinweaver::library::Skin| {
            query.is_empty()
                || skin.name.to_lowercase().contains(&query)
                || skin.author.to_lowercase().contains(&query)
        };
        let code_of = |app: &Self, path: &PathBuf| {
            app.skin_index
                .get(path)
                .and_then(|&i| app.skin_codes.get(i))
                .cloned()
                .unwrap_or_default()
        };
        for (path, why) in &pinned {
            if let Some(skin) = self.skin(path).filter(|s| matches(s)) {
                options.push((
                    Source::Skin(path.clone()),
                    skin.name.clone(),
                    format!("{} · {why}", code_of(self, path)),
                    true,
                ));
            }
        }
        for skin in &self.skins {
            if !matches(skin) || pinned.iter().any(|(p, _)| *p == skin.path) {
                continue;
            }
            options.push((
                Source::Skin(skin.path.clone()),
                skin.name.clone(),
                code_of(self, &skin.path),
                false,
            ));
        }
        let columns = self.variant_columns.clamp(1, 3);
        let max = match columns {
            1 => 512,
            2 => 320,
            _ => 192,
        };
        let total = options.len();
        let mut hidden = 0;
        let mut shown: Vec<(Source, String, String, Presence)> = vec![];
        for (source, name, code, pinned) in options {
            let origin = self.origin(&source);
            let presence = self.presence(&origin, &kind, max);
            if !pinned
                && self.variant_own_only
                && matches!(presence, Presence::Fallback | Presence::Missing)
            {
                hidden += 1;
                continue;
            }
            shown.push((source, name, code, presence));
        }
        let picked = self.picked_ids();
        let selected = if picked.len() > 1 {
            // A tile is checked only when every picked element already uses it.
            let first = self.effective(&picked[0]);
            picked
                .iter()
                .all(|p| {
                    self.effective(p) == first
                        && !self
                            .mix
                            .choices
                            .keys()
                            .any(|k| k.starts_with(&format!("{p}.")))
                })
                .then_some(first)
        } else {
            (!mixed).then_some(current)
        };
        let mut chosen = None;
        match &kind {
            Show::Sound(stem) => {
                for (source, name, code, presence) in &shown {
                    if self.sound_variant(
                        ui,
                        source,
                        name,
                        code,
                        *presence,
                        stem,
                        selected.as_ref(),
                    ) {
                        chosen = Some(source.clone());
                    }
                }
            }
            Show::Nothing => {
                ui.label(
                    RichText::new(self.t(
                        "Для этого элемента нечего показать — выбери его части ниже.",
                        "Nothing to show for this one — pick its parts below.",
                    ))
                    .color(INK_MUTED),
                );
            }
            _ => {
                let gap = 10.0;
                let width = ui.available_width();
                let tile = ((width - gap * (columns - 1) as f32) / columns as f32).floor();
                let height = (tile * if columns == 1 { 0.62 } else { 0.78 }).max(70.0);
                for row in shown.chunks(columns) {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for (source, name, code, presence) in row {
                            if self.variant_tile(
                                ui,
                                vec2(tile, height),
                                &kind,
                                source,
                                name,
                                code,
                                *presence,
                                selected.as_ref(),
                                max,
                            ) {
                                chosen = Some(source.clone());
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
            }
        }
        if hidden > 0 {
            ui.label(caption(format!(
                "{} {} {} — {}",
                self.t("Скрыто", "Hidden"),
                hidden,
                self.t(&format!("из {total}"), &format!("of {total}")),
                self.t(
                    "в них нет своего файла, osu! взял бы стандартный.",
                    "they have no file of their own, so osu! would use the default.",
                )
            )));
        }
        if let Some(source) = chosen {
            self.choose(&id, source);
        }
        // Parts of a group can still be picked one by one.
        if !node.children.is_empty() {
            ui.add_space(18.0);
            section_title(ui, self.t("По отдельности", "One by one"));
            ui.label(caption(self.t(
                "Каждой части можно выбрать свой источник; нажми на строку, чтобы увидеть её варианты.",
                "Each part can come from its own source; click a row to see its variants.",
            )));
            ui.add_space(6.0);
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (i, child) in node.children.iter().enumerate() {
                    self.tree_row(ui, child, &format!("{id}.{i}"), 1, "");
                }
            });
            ui.add_space(12.0);
        }
    }

    /// The Ctrl/Shift selection: what a chosen tile will be applied to.
    fn picked_box(&mut self, ui: &mut egui::Ui, focus: &str) {
        let picked = self.picked_ids();
        if picked.len() < 2 {
            return;
        }
        let nodes = self.nodes.clone();
        let russian = self.ru();
        let mut remove = None;
        let mut show = None;
        let mut clear = false;
        super::motion::glass(ui, ACCENT_SOFT, 10, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{}: {}",
                        self.t("Выбрано элементов", "Elements picked"),
                        picked.len()
                    ))
                    .font(font_strong(14.0))
                    .color(ACCENT_TEXT),
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if btn(Kind::Ghost, self.t("Снять выбор", "Clear"))
                        .small()
                        .show(ui)
                        .on_hover_text("Esc")
                        .clicked()
                    {
                        clear = true;
                    }
                });
            });
            ui.label(caption(self.t(
                    "Плитка, на которую нажмёшь, достанется всем им сразу. Показан тот, что выделен ниже.",
                    "The tile you click goes to all of them at once. The highlighted one is shown.",
                )));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 4.0);
                for id in &picked {
                    let Some(node) = node_at(&nodes, id) else {
                        continue;
                    };
                    let label = node.label(russian);
                    let response = btn(Kind::Secondary, label)
                        .small()
                        .selected(id == focus)
                        .show(ui)
                        .on_hover_text(self.t(
                            "Показать его плитки; Ctrl+клик — убрать из выбора",
                            "Show its tiles; Ctrl+click removes it",
                        ));
                    if response.clicked() {
                        if ui.input(|i| i.modifiers.command) {
                            remove = Some(id.clone());
                        } else {
                            show = Some(id.clone());
                        }
                    }
                }
            });
        });
        ui.add_space(10.0);
        if clear {
            self.clear_picked();
        } else if let Some(id) = remove {
            self.pick(&id, Pick::Toggle, &[]);
        } else if let Some(id) = show {
            self.mix.focus = Some(id);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn variant_tile(
        &mut self,
        ui: &mut egui::Ui,
        size: Vec2,
        kind: &Show,
        source: &Source,
        name: &str,
        code: &str,
        presence: Presence,
        selected: Option<&Source>,
        max: u32,
    ) -> bool {
        let (rect, response) = ui.allocate_exact_size(vec2(size.x, size.y + 36.0), Sense::click());
        if !ui.is_rect_visible(rect) {
            return false;
        }
        let image_rect = egui::Rect::from_min_size(rect.min, size);
        let is_selected = selected == Some(source);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), is_selected, name)
        });
        checkerboard(ui, image_rect, RADIUS_SM);
        let origin = self.origin(source);
        self.paint_kind(ui, image_rect.shrink(10.0), kind, &origin, max);
        if presence == Presence::Fallback && matches!(source, Source::Skin(_)) {
            ui.painter().rect_filled(
                image_rect.shrink(1.0),
                0,
                Color32::from_rgba_unmultiplied(22, 21, 26, 120),
            );
            badge_at(
                ui,
                image_rect.left_bottom() + vec2(6.0, -22.0),
                self.t("как в стандартном", "same as default"),
            );
        }
        let stroke = if is_selected {
            Stroke::new(3.0, ACCENT_LINE)
        } else if response.hovered() {
            Stroke::new(1.5, LINE_200)
        } else {
            Stroke::NONE
        };
        ui.painter()
            .rect_stroke(image_rect, 0, stroke, egui::StrokeKind::Inside);
        super::motion::approach(ui, response.id, image_rect, is_selected);
        if is_selected {
            let mark = egui::Rect::from_min_size(
                pos2(image_rect.right() - 26.0, image_rect.top() + 6.0),
                Vec2::splat(20.0),
            );
            ui.painter().rect_filled(mark, 2, ACCENT_LINE);
            egui::Image::new(icons::check())
                .tint(ON_INK)
                .paint_at(ui, mark.shrink(3.0));
        }
        if !code.is_empty() {
            code_tab(ui, image_rect.min, code, !is_selected);
        }
        let title = truncated(
            ui,
            name,
            if is_selected {
                font_strong(13.5)
            } else {
                font(13.5)
            },
            INK,
            size.x,
        );
        ui.painter()
            .galley(pos2(rect.left(), image_rect.bottom() + 4.0), title, INK);
        let note = match (source, presence) {
            (Source::Blank, _) => self.t("ничего не рисуется", "draws nothing"),
            (Source::Default, _) => self.t("встроенный набор", "built-in set"),
            (_, Presence::Own) => self.t("свой файл", "own file"),
            (_, Presence::Loading) => self.t("загружаю…", "loading…"),
            _ => self.t("нет файла", "no file"),
        };
        text_at(
            ui,
            pos2(rect.left(), image_rect.bottom() + 21.0),
            egui::Align2::LEFT_TOP,
            note,
            font(11.5),
            if presence == Presence::Own && matches!(source, Source::Skin(_)) {
                SUCCESS
            } else {
                INK_SUBTLE
            },
        );
        focus_ring(ui, &response, rect.shrink(3.0));
        response
            .on_hover_text(name)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
    }

    fn paint_kind(
        &mut self,
        ui: &egui::Ui,
        area: egui::Rect,
        kind: &Show,
        origin: &Origin,
        max: u32,
    ) {
        let painter = ui
            .painter()
            .with_clip_rect(area.expand(8.0).intersect(ui.clip_rect()));
        let empty = |ui: &egui::Ui, text: &str| {
            egui::Image::new(icons::image_off()).tint(BG_400).paint_at(
                ui,
                egui::Rect::from_center_size(area.center() - vec2(0.0, 8.0), Vec2::splat(22.0)),
            );
            text_at(
                ui,
                area.center() + vec2(0.0, 16.0),
                egui::Align2::CENTER_CENTER,
                text,
                font(12.0),
                BG_400,
            );
        };
        match kind {
            Show::Images(stems, layout) => {
                if matches!(origin, Origin::Blank) {
                    empty(ui, self.t("пусто", "blank"));
                    return;
                }
                let mut layers = vec![];
                let mut loading = false;
                for stem in stems {
                    let stem = self.skin_stem(origin, stem);
                    let animated = self.stage_index.animated(&stem);
                    match self.element(origin, &stem, animated, max) {
                        Some(State::Ready(e)) => {
                            let tint = self.combo_tint(origin, &stem);
                            layers.push((e, tint));
                        }
                        Some(State::Loading) => loading = true,
                        _ => {}
                    }
                }
                if layers.is_empty() {
                    if loading {
                        painter.circle_stroke(area.center(), 10.0, Stroke::new(2.0, BG_400));
                    } else {
                        empty(ui, self.t("нет файла", "no file"));
                    }
                    return;
                }
                let gap = 8.0;
                let bounds = match layout {
                    Layout2::Row => vec2(
                        layers.iter().map(|(e, _)| e.logical.x).sum::<f32>()
                            + gap * (layers.len() - 1) as f32,
                        layers.iter().map(|(e, _)| e.logical.y).fold(0.0, f32::max),
                    ),
                    _ => layers.iter().fold(Vec2::ZERO, |a, (e, _)| a.max(e.logical)),
                };
                if bounds.x <= 0.0 || bounds.y <= 0.0 {
                    return;
                }
                let fit = (area.width() / bounds.x)
                    .min(area.height() / bounds.y)
                    .min(3.0);
                let corner = area.center() - bounds * fit / 2.0;
                let mut x = corner.x;
                for (element, tint) in &layers {
                    let frame = self.animation_frame(ui, element.frames.len());
                    let size = element.logical * fit;
                    let rect = match layout {
                        Layout2::Stack => egui::Rect::from_center_size(area.center(), size),
                        Layout2::Corner => egui::Rect::from_min_size(corner, size),
                        Layout2::Row => {
                            let r = egui::Rect::from_min_size(
                                pos2(x, area.center().y - size.y / 2.0),
                                size,
                            );
                            x += size.x + gap * fit;
                            r
                        }
                    };
                    painter.image(
                        element.frames[frame].id(),
                        rect,
                        egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        *tint,
                    );
                }
            }
            Show::Colors(keys) => {
                let colors: Vec<[u8; 3]> = keys
                    .iter()
                    .filter_map(|k| {
                        self.ini_value(origin, "Colours", k)
                            .and_then(|v| parse_color(&v))
                    })
                    .collect();
                if colors.is_empty() {
                    empty(ui, self.t("не задано", "not set"));
                    return;
                }
                let n = colors.len() as f32;
                let side = (area.width() / n - 8.0).min(area.height() * 0.7).max(12.0);
                let start = area.center().x - (side * n + 8.0 * (n - 1.0)) / 2.0;
                for (i, color) in colors.iter().enumerate() {
                    let r = egui::Rect::from_min_size(
                        pos2(
                            start + i as f32 * (side + 8.0),
                            area.center().y - side / 2.0,
                        ),
                        Vec2::splat(side),
                    );
                    painter.rect(
                        r,
                        2,
                        rgb(*color),
                        Stroke::new(1.0, Color32::from_white_alpha(90)),
                        egui::StrokeKind::Inside,
                    );
                }
            }
            Show::Value(section, key) => {
                let value = self.ini_value(origin, section, key);
                text_at(
                    ui,
                    area.center(),
                    egui::Align2::CENTER_CENTER,
                    value.as_deref().unwrap_or(self.t("не задано", "not set")),
                    egui::FontId::monospace(15.0),
                    ON_INK,
                );
            }
            Show::Sound(_) | Show::Nothing => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sound_variant(
        &mut self,
        ui: &mut egui::Ui,
        source: &Source,
        name: &str,
        code: &str,
        presence: Presence,
        stem: &str,
        selected: Option<&Source>,
    ) -> bool {
        let (rect, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::click());
        if !ui.is_rect_visible(rect) {
            return false;
        }
        let is_selected = selected == Some(source);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), is_selected, name)
        });
        ui.painter().rect(
            rect,
            RADIUS_SM,
            if is_selected {
                ACCENT_SOFT
            } else if response.hovered() {
                BG_300
            } else {
                Color32::TRANSPARENT
            },
            Stroke::new(1.0, if is_selected { ACCENT_LINE } else { LINE_100 }),
            egui::StrokeKind::Inside,
        );
        let play = egui::Rect::from_min_size(rect.min + vec2(6.0, 7.0), Vec2::splat(30.0));
        let play_response = ui
            .interact(
                play,
                ui.make_persistent_id(("variant_play", name, code)),
                Sense::click(),
            )
            .on_hover_text(self.t("Слушать", "Play"));
        play_response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), format!("{}: {name}", self.t("Слушать", "Play")))
        });
        ui.painter().rect_filled(
            play,
            RADIUS_SM,
            if play_response.hovered() {
                BG_400
            } else {
                BG_300
            },
        );
        egui::Image::new(icons::play()).tint(INK).paint_at(
            ui,
            egui::Rect::from_center_size(play.center(), Vec2::splat(14.0)),
        );
        let mut x = play.right() + 10.0;
        if !code.is_empty() {
            let tab = code_tab(ui, pos2(x, rect.center().y - 9.0), code, !is_selected);
            x = tab.right() + 8.0;
        }
        let note = match (source, presence) {
            (Source::Blank, _) => self.t("тишина", "silence"),
            (Source::Default, _) => self.t("встроенный набор", "built-in set"),
            (_, Presence::Own) => self.t("свой файл", "own file"),
            _ => self.t("нет файла — стандартный", "no file — default"),
        };
        let note_galley = ui
            .painter()
            .layout_no_wrap(note.to_owned(), font(11.5), INK_SUBTLE);
        let title = truncated(
            ui,
            name,
            if is_selected {
                font_strong(13.5)
            } else {
                font(13.5)
            },
            INK,
            (rect.right() - x - note_galley.size().x - 16.0).max(40.0),
        );
        ui.painter()
            .galley(pos2(x, rect.center().y - title.size().y / 2.0), title, INK);
        ui.painter().galley(
            pos2(
                rect.right() - note_galley.size().x - 10.0,
                rect.center().y - note_galley.size().y / 2.0,
            ),
            note_galley,
            INK_SUBTLE,
        );
        ui.add_space(4.0);
        focus_ring(ui, &response, rect.shrink(3.0));
        focus_ring(ui, &play_response, play);
        if play_response.clicked() {
            match self.origin(source) {
                Origin::Folder(folder) => self.play(folder.as_deref(), stem),
                Origin::Blank => self.stop_audio(),
            }
            return false;
        }
        response
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
    }
}

fn badge_at(ui: &egui::Ui, at: egui::Pos2, text: &str) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font(11.0), INK);
    let rect = egui::Rect::from_min_size(at, galley.size() + vec2(8.0, 2.0));
    ui.painter().rect_filled(rect, 2, BG_200);
    ui.painter().galley(rect.min + vec2(4.0, 1.0), galley, INK);
}
