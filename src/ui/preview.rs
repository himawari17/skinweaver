//! Responsive preview: element before/after comparison, gameplay scene, skin gallery.

use super::{
    editor::{Origin, audio_stem, image_stem, node_at, parse_color, representative},
    icons,
    textures::State,
    theme::*,
    *,
};
use skinweaver::{catalog::Target, ini::SkinIni, mixer::Selection};
use std::{cell::RefCell, rc::Rc};

/// Scene inputs kept between frames; see `App::scene_inputs`.
pub(crate) struct SceneMemo {
    key: u64,
    selections: Rc<Vec<Selection>>,
    ini: SkinIni,
    /// File name drawn by the scene -> where it comes from.
    origins: RefCell<HashMap<String, Source>>,
}

/// Elements shown in the library gallery: (stem, animated, ru, en).
const GALLERY: &[(&str, bool, &str, &str)] = &[
    ("hitcircle", false, "Круг", "Hit circle"),
    ("hitcircleoverlay", false, "Обводка круга", "Circle overlay"),
    ("approachcircle", false, "Кольцо", "Approach circle"),
    ("sliderb", true, "Шар слайдера", "Slider ball"),
    ("sliderfollowcircle", true, "Зона слайдера", "Follow circle"),
    ("reversearrow", false, "Стрелка возврата", "Reverse arrow"),
    ("spinner-circle", false, "Спиннер", "Spinner"),
    (
        "spinner-approachcircle",
        false,
        "Кольцо спиннера",
        "Spinner ring",
    ),
    ("cursor", false, "Курсор", "Cursor"),
    ("cursortrail", false, "След курсора", "Cursor trail"),
    ("hit300", true, "300", "300"),
    ("hit100", true, "100", "100"),
    ("hit50", true, "50", "50"),
    ("hit0", true, "Промах", "Miss"),
    ("default-1", false, "Цифра", "Number"),
    ("score-7", false, "Цифра счёта", "Score digit"),
    ("ranking-s", false, "Оценка S", "S rank"),
    ("scorebar-colour", true, "Здоровье", "Health bar"),
    ("menu-back", true, "Назад", "Back button"),
    ("play-skip", true, "Пропуск", "Skip"),
];

const SAMPLES: [&str; 6] = [
    "normal-hitnormal",
    "soft-hitnormal",
    "drum-hitnormal",
    "normal-hitclap",
    "combobreak",
    "menuhit",
];

struct Side {
    title: &'static str,
    source: Source,
    origin: Origin,
}

impl App {
    pub(crate) fn preview_panel(&mut self, ui: &mut egui::Ui, opacity: f32) {
        let compact = ui.ctx().content_rect().width() < 900.0;
        let panel = if compact {
            egui::Panel::bottom("preview_compact")
                .default_size((ui.ctx().content_rect().height() * 0.28).clamp(120.0, 200.0))
                .size_range(120.0..=320.0)
        } else {
            egui::Panel::right("preview")
            .default_size(
                if self.stage_mode() && ui.ctx().content_rect().width() >= 1400.0 {
                    420.0
                } else {
                    380.0
                },
            )
            .size_range(320.0..=760.0)
        };
        panel
            .resizable(true)
            .show_separator_line(false)
            .frame(
                egui::Frame::new()
                    .fill(BG_200)
                    .inner_margin(egui::Margin::symmetric(20, 20)),
            )
            .show(ui, |ui| {
                ui.multiply_opacity(opacity);
                // Only the taller side panel can keep the skin header pinned.
                if !compact && self.page == Page::Library && self.focus_skin.is_some() {
                    self.skin_header(ui);
                    let rect = ui.available_rect_before_wrap();
                    hairline(ui, rect.top(), rect.left()..=rect.right());
                    ui.add_space(12.0);
                }
                egui::ScrollArea::vertical()
                    .id_salt(("preview", self.page as u8))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        if compact && self.page == Page::Library && self.focus_skin.is_some() {
                            self.skin_header(ui);
                            ui.add_space(12.0);
                        }
                        match self.page {
                            Page::Library => self.skin_panel(ui),
                            _ => self.editor_preview(ui),
                        }
                    });
            });
    }

    fn editor_preview(&mut self, ui: &mut egui::Ui) {
        if self.stage_mode() {
            if self.preview_tab == PreviewTab::Scene {
                self.preview_tab = PreviewTab::Variants;
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new(self.t("Варианты", "Variants")).font(condensed(28.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let options = [
                        (PreviewTab::Variants, self.t("Выбор", "Pick")),
                        (PreviewTab::Element, self.t("Сравнить", "Compare")),
                    ];
                    tabs(ui, &mut self.preview_tab, &options);
                });
            });
            ui.add_space(12.0);
            match (self.preview_tab, self.mix.focus.clone()) {
                (PreviewTab::Element, Some(id)) => self.element_detail(ui, &id),
                _ => self.variants_panel(ui),
            }
            return;
        }
        if self.preview_tab == PreviewTab::Variants {
            self.preview_tab = PreviewTab::Element;
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.t("Предпросмотр", "Preview")).font(condensed(28.0)));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let options = [
                    (PreviewTab::Element, self.t("Элемент", "Element")),
                    (PreviewTab::Scene, self.t("Сцена", "Scene")),
                ];
                tabs(ui, &mut self.preview_tab, &options);
            });
        });
        ui.add_space(16.0);
        if self.page == Page::Modify && self.selected.is_empty() {
            ui.label(
                RichText::new(self.t(
                    "Выбери скины в библиотеке, чтобы увидеть, как они изменятся.",
                    "Pick skins in the library to see how they will change.",
                ))
                .color(INK_MUTED),
            );
            return;
        }
        let focus = self.editor_ref().focus.clone();
        match (self.preview_tab, focus) {
            (PreviewTab::Element, Some(id)) => self.element_detail(ui, &id),
            (PreviewTab::Element, None) => {
                ui.label(
                    RichText::new(self.t("Выбери элемент в списке", "Pick an element in the list"))
                        .font(font_strong(16.0)),
                );
                ui.label(
                    RichText::new(self.t(
                        "Здесь появится он сам и сравнение с тем, что было до твоего выбора.",
                        "It will show up here, compared with what it was before your choice.",
                    ))
                    .color(INK_MUTED),
                );
                ui.add_space(20.0);
                self.scene(ui);
            }
            (PreviewTab::Scene | PreviewTab::Variants, _) => self.scene(ui),
        }
    }

    // ------------------------------------------------------------ element detail

    fn element_detail(&mut self, ui: &mut egui::Ui, id: &str) {
        let nodes = self.nodes.clone();
        let Some(node) = node_at(&nodes, id) else {
            return;
        };
        let russian = self.ru();
        // Breadcrumb of parent groups.
        let mut trail = vec![];
        let mut prefix = String::new();
        for part in id
            .split('.')
            .collect::<Vec<_>>()
            .iter()
            .take(id.split('.').count() - 1)
        {
            prefix = if prefix.is_empty() {
                (*part).to_owned()
            } else {
                format!("{prefix}.{part}")
            };
            if let Some(parent) = node_at(&nodes, &prefix) {
                trail.push(parent.label(russian).to_owned());
            }
        }
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(0.0, 26.0), Sense::hover());
            let tab = code_tab(
                ui,
                pos2(r.left(), r.center().y - 9.0),
                &super::editor::element_code(id),
                true,
            );
            ui.add_space(tab.width() + 4.0);
            ui.label(RichText::new(node.label(russian)).font(condensed(26.0)));
        });
        let mut line = trail.join(" › ");
        if node.label(russian) != node.name {
            if !line.is_empty() {
                line.push_str("  ·  ");
            }
            line.push_str(&node.name);
        }
        if !line.is_empty() {
            ui.label(caption(line));
        }
        ui.add_space(12.0);
        let after = self.effective(id);
        let before = self.previous(id);
        let before_side = Side {
            title: if self.page == Page::Modify {
                self.t("Сейчас", "Now")
            } else {
                self.t("Было", "Before")
            },
            origin: self.origin(&before),
            source: before,
        };
        let after_side = Side {
            title: self.t("Станет", "After"),
            origin: self.origin(&after),
            source: after,
        };
        self.compare_source_picker(ui);
        ui.add_space(12.0);
        match &node.target {
            Some(target) => {
                if let Some((stem, animated)) = image_stem(target) {
                    self.compare_images(ui, &stem, animated, before_side, after_side);
                } else if let Some(stem) = audio_stem(target) {
                    self.compare_audio(ui, &stem, before_side, after_side);
                } else if let Target::Property { section, key } = target {
                    self.compare_property(ui, section, key, before_side, after_side);
                } else if let Target::Section {
                    section,
                    key,
                    value,
                } = target
                {
                    ui.label(RichText::new(format!("[{section}] {key}: {value}")).monospace());
                    ui.label(caption(self.t(
                        "Этот раздел skin.ini переносится целиком из выбранного источника.",
                        "This skin.ini section is copied as a whole from the chosen source.",
                    )));
                }
            }
            None => self.group_grid(ui, node, id, before_side, after_side),
        }
    }

    fn compare_source_picker(&mut self, ui: &mut egui::Ui) {
        if self.page == Page::Modify {
            let target = self
                .target_skin()
                .map(|p| source_name(self.settings.language, &Source::Skin(p)));
            ui.label(caption(format!(
                "{} {}",
                self.t("Сравнение со скином", "Compared with skin"),
                target.unwrap_or_default()
            )));
            return;
        }
        let language = self.settings.language;
        let current = self.mix.compare.clone();
        let label = match &current {
            None => language
                .text("с прошлым выбором", "the previous choice")
                .to_owned(),
            Some(source) => source_name(language, source),
        };
        let mut chosen: Option<Option<Source>> = None;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(language.text("Сравнить", "Compare with"))
                    .color(INK_MUTED)
                    .size(13.0),
            );
            egui::ComboBox::from_id_salt("compare_with")
                .icon(combo_chevron)
                .width((ui.available_width() - 8.0).max(120.0))
                .truncate()
                .selected_text(RichText::new(label).size(13.0))
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(
                            current.is_none(),
                            language.text("С прошлым выбором", "Previous choice"),
                        )
                        .clicked()
                    {
                        chosen = Some(None);
                    }
                    if ui
                        .selectable_label(
                            current == Some(Source::Default),
                            language.text("Со стандартным скином", "Default skin"),
                        )
                        .clicked()
                    {
                        chosen = Some(Some(Source::Default));
                    }
                    ui.separator();
                    for skin in &self.skins {
                        let option = Source::Skin(skin.path.clone());
                        if ui
                            .selectable_label(current.as_ref() == Some(&option), &skin.name)
                            .clicked()
                        {
                            chosen = Some(Some(option));
                        }
                    }
                });
        });
        if let Some(choice) = chosen {
            self.mix.compare = choice;
        }
    }

    fn compare_images(
        &mut self,
        ui: &mut egui::Ui,
        stem: &str,
        animated: bool,
        before: Side,
        after: Side,
    ) {
        let before_state = self.element(&before.origin, stem, animated, 512);
        let after_state = self.element(&after.origin, stem, animated, 512);
        let same = before.origin == after.origin;
        let before_tint = self.combo_tint(&before.origin, stem);
        let after_tint = self.combo_tint(&after.origin, stem);
        if !same {
            ui.horizontal(|ui| {
                let options = [
                    (CompareMode::Side, self.t("Рядом", "Side by side")),
                    (CompareMode::Wipe, self.t("Шторка", "Wipe")),
                ];
                tabs(ui, &mut self.compare_mode, &options);
            });
            ui.add_space(10.0);
        }
        let width = ui.available_width();
        if same || self.compare_mode == CompareMode::Side {
            let gap = 10.0;
            let tile = if same { width } else { (width - gap) / 2.0 };
            let height = if same {
                (width * 0.7).min(320.0)
            } else {
                tile.max(120.0)
            };
            let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
            if same {
                let fit = shared_fit(&after_state, &None, rect.shrink(12.0));
                self.tile(ui, rect, &after_state, fit, after_tint);
                code_tab(ui, rect.min, after.title, false);
            } else {
                let left = egui::Rect::from_min_size(rect.min, vec2(tile, height));
                let right = egui::Rect::from_min_size(
                    pos2(rect.min.x + tile + gap, rect.min.y),
                    vec2(tile, height),
                );
                // One scale for both tiles keeps real sizes comparable.
                let fit = shared_fit(&before_state, &after_state, left.shrink(12.0));
                self.tile(ui, left, &before_state, fit, before_tint);
                self.tile(ui, right, &after_state, fit, after_tint);
                code_tab(ui, left.min, before.title, true);
                code_tab(ui, right.min, after.title, false);
            }
            ui.add_space(8.0);
            if same {
                self.side_caption(ui, &after, &after_state, width);
                ui.label(caption(self.t(
                    "Не отличается от того, с чем сравниваем.",
                    "Same as what we compare with.",
                )));
            } else {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    ui.vertical(|ui| {
                        ui.set_width(tile);
                        self.side_caption(ui, &before, &before_state, tile);
                    });
                    ui.vertical(|ui| {
                        ui.set_width(tile);
                        self.side_caption(ui, &after, &after_state, tile);
                    });
                });
            }
        } else {
            let height = (width * 0.8).min(360.0);
            let (rect, response) =
                ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
            if (response.dragged() || response.clicked())
                && let Some(pos) = response.interact_pointer_pos()
            {
                self.wipe = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
            }
            checkerboard(ui, rect, RADIUS_MD);
            let split = rect.left() + rect.width() * self.wipe;
            let left_clip = egui::Rect::from_min_max(rect.min, pos2(split, rect.max.y));
            let right_clip = egui::Rect::from_min_max(pos2(split, rect.min.y), rect.max);
            let fit = shared_fit(&before_state, &after_state, rect.shrink(16.0));
            self.paint_state(ui, rect, &before_state, Some(left_clip), fit, before_tint);
            self.paint_state(ui, rect, &after_state, Some(right_clip), fit, after_tint);
            ui.painter()
                .vline(split, rect.y_range(), Stroke::new(2.0, ACCENT_LINE));
            ui.painter()
                .circle_filled(pos2(split, rect.center().y), 9.0, ACCENT_LINE);
            egui::Image::new(icons::columns_2()).tint(BG_200).paint_at(
                ui,
                egui::Rect::from_center_size(pos2(split, rect.center().y), Vec2::splat(12.0)),
            );
            code_tab(ui, rect.min, before.title, true);
            let galley = ui
                .painter()
                .layout_no_wrap(after.title.to_owned(), condensed(13.0), INK);
            code_tab(
                ui,
                pos2(rect.right() - galley.size().x - 8.0, rect.top()),
                after.title,
                false,
            );
            let response = response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            focus_ring(ui, &response, rect.shrink(3.0));
            let _ = response;
            ui.add_space(8.0);
            ui.add(
                egui::Slider::new(&mut self.wipe, 0.0..=1.0)
                    .step_by(0.01)
                    .show_value(false)
                    .text(self.settings.language.text("Положение шторки", "Wipe position")),
            );
            ui.horizontal_top(|ui| {
                let half = (width - 10.0) / 2.0;
                ui.vertical(|ui| {
                    ui.set_width(half);
                    self.side_caption(ui, &before, &before_state, half);
                });
                ui.vertical(|ui| {
                    ui.set_width(half);
                    self.side_caption(ui, &after, &after_state, half);
                });
            });
        }
    }

    fn tile(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        state: &Option<State>,
        fit: f32,
        tint: Color32,
    ) {
        checkerboard(ui, rect, RADIUS_MD);
        self.paint_state(ui, rect, state, None, fit, tint);
    }

    /// Draws an element's current animation frame (or a placeholder) inside `rect`.
    fn paint_state(
        &self,
        ui: &egui::Ui,
        rect: egui::Rect,
        state: &Option<State>,
        clip: Option<egui::Rect>,
        scale: f32,
        tint: Color32,
    ) {
        let painter = ui
            .painter()
            .with_clip_rect(clip.unwrap_or(rect).intersect(ui.clip_rect()));
        match state {
            None => {
                let r = egui::Rect::from_center_size(rect.center(), Vec2::splat(24.0));
                egui::Image::new(icons::image_off())
                    .tint(BG_400)
                    .paint_at(ui, r);
                if clip.is_none() {
                    text_at(
                        ui,
                        rect.center() + vec2(0.0, 26.0),
                        egui::Align2::CENTER_CENTER,
                        self.t("пусто", "blank"),
                        font(12.5),
                        BG_300,
                    );
                }
            }
            Some(State::Loading) => {
                painter.circle_stroke(rect.center(), 10.0, Stroke::new(2.0, BG_400));
            }
            Some(State::Missing) => {
                if clip.is_none() {
                    let r = egui::Rect::from_center_size(
                        rect.center() - vec2(0.0, 8.0),
                        Vec2::splat(24.0),
                    );
                    egui::Image::new(icons::image_off())
                        .tint(BG_400)
                        .paint_at(ui, r);
                    text_at(
                        ui,
                        rect.center() + vec2(0.0, 18.0),
                        egui::Align2::CENTER_CENTER,
                        self.t("нет файла", "no file"),
                        font(12.5),
                        BG_300,
                    );
                }
            }
            Some(State::Ready(element)) => {
                let frame = self.animation_frame(ui, element.frames.len());
                let texture = &element.frames[frame];
                let size = element.logical * scale;
                painter.image(
                    texture.id(),
                    egui::Rect::from_center_size(rect.center(), size),
                    egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    tint,
                );
            }
        }
    }

    /// osu! tints circles with the combo colour; previews do the same so white sprites read.
    pub(crate) fn combo_tint(&self, origin: &Origin, stem: &str) -> Color32 {
        const TINTED: [&str; 4] = [
            "hitcircle",
            "sliderstartcircle",
            "sliderendcircle",
            "approachcircle",
        ];
        if !TINTED.contains(&stem) {
            return Color32::WHITE;
        }
        let colors = match origin {
            Origin::Folder(Some(path)) => self.skin(path).map(|s| s.ini.colors()),
            _ => None,
        }
        .unwrap_or_else(|| SkinIni::default().colors());
        rgb(colors[0])
    }

    pub(crate) fn animation_frame(&self, ui: &egui::Ui, frames: usize) -> usize {
        if frames <= 1 || self.settings.reduce_motion {
            return 0;
        }
        ui.ctx().request_repaint_after(Duration::from_millis(60));
        (ui.input(|i| i.time) * 16.0) as usize % frames
    }

    fn side_caption(&self, ui: &mut egui::Ui, side: &Side, state: &Option<State>, width: f32) {
        ui.add(
            egui::Label::new(
                RichText::new(source_name(self.settings.language, &side.source))
                    .font(font_strong(14.0)),
            )
            .truncate(),
        )
        .on_hover_text(source_name(self.settings.language, &side.source));
        let _ = width;
        if let Some(State::Ready(element)) = state {
            let mut meta = format!("{}×{}", element.size[0], element.size[1]);
            if element.frame_count > 1 {
                meta.push_str(&format!(
                    " · {}",
                    self.settings.language.count(
                        element.frame_count,
                        ["кадр", "кадра", "кадров"],
                        ["frame", "frames"]
                    )
                ));
            }
            ui.add_space(-4.0);
            ui.label(RichText::new(meta).font(condensed(18.0)).color(INK));
            ui.add_space(-4.0);
            ui.label(caption(&element.file));
            if element.fallback {
                badge(ui, self.t("из стандартного", "from default"), false).on_hover_text(self.t(
                    "В этом скине нет файла — osu! возьмёт стандартный.",
                    "This skin has no such file, so osu! uses the default one.",
                ));
            }
        }
    }

    fn compare_audio(&mut self, ui: &mut egui::Ui, stem: &str, before: Side, after: Side) {
        for side in [before, after] {
            super::motion::glass(ui, BG_200, 12, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let folder = match &side.origin {
                        Origin::Folder(f) => Some(f.clone()),
                        Origin::Blank => None,
                    };
                    let play = btn(Kind::Secondary, "")
                        .icon(icons::play())
                        .enabled(folder.is_some())
                        .show(ui)
                        .accessible_name(self.t("Слушать", "Play"))
                        .on_hover_text(self.t("Слушать", "Play"));
                    if play.clicked()
                        && let Some(folder) = &folder
                    {
                        self.play(folder.as_deref(), stem);
                    }
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(side.title)
                                .font(font_strong(12.0))
                                .color(INK_SUBTLE),
                        );
                        ui.add(
                            egui::Label::new(source_name(self.settings.language, &side.source))
                                .truncate(),
                        );
                    });
                });
            });
            ui.add_space(8.0);
        }
        ui.label(caption(self.t(
            "Если звука нет в скине, osu! играет стандартный.",
            "If the skin has no such sound, osu! plays the default one.",
        )));
    }

    fn compare_property(
        &mut self,
        ui: &mut egui::Ui,
        section: &str,
        key: &str,
        before: Side,
        after: Side,
    ) {
        ui.horizontal_top(|ui| {
            let half = (ui.available_width() - 10.0) / 2.0;
            for side in [before, after] {
                ui.vertical(|ui| {
                    ui.set_width(half);
                    let value = match &side.origin {
                        Origin::Blank => None,
                        origin => self.ini_value(origin, section, key),
                    };
                    let (rect, _) = ui.allocate_exact_size(vec2(half, 72.0), Sense::hover());
                    ui.painter().rect_filled(rect, RADIUS_MD, BG_200);
                    match value.as_deref().and_then(parse_color) {
                        Some(color) if section.eq_ignore_ascii_case("Colours") => {
                            ui.painter()
                                .rect_filled(rect.shrink(10.0), RADIUS_SM, rgb(color));
                        }
                        _ => {
                            text_at(
                                ui,
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                value.as_deref().unwrap_or(self.t("не задано", "not set")),
                                egui::FontId::monospace(13.0),
                                INK,
                            );
                        }
                    }
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(side.title)
                            .font(font_strong(12.0))
                            .color(INK_SUBTLE),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(source_name(self.settings.language, &side.source))
                                .size(13.0),
                        )
                        .truncate(),
                    );
                    if let Some(value) = value {
                        ui.label(caption(value).family(egui::FontFamily::Monospace));
                    }
                });
            }
        });
    }

    fn group_grid(
        &mut self,
        ui: &mut egui::Ui,
        node: &OptionNode,
        id: &str,
        before: Side,
        after: Side,
    ) {
        let russian = self.ru();
        let mut leaves = vec![];
        collect_leaves(node, id, &mut leaves);
        let images: Vec<_> = leaves
            .into_iter()
            .filter_map(|(leaf_id, leaf)| {
                representative(leaf).map(|stem| (leaf_id, leaf.label(russian).to_owned(), stem))
            })
            .take(48)
            .collect();
        if images.is_empty() {
            let mut leaves = vec![];
            collect_leaves(node, id, &mut leaves);
            let sounds: Vec<_> = leaves
                .into_iter()
                .filter_map(|(leaf_id, leaf)| {
                    leaf.target
                        .as_ref()
                        .and_then(audio_stem)
                        .map(|stem| (leaf_id, leaf.label(russian).to_owned(), stem))
                })
                .take(40)
                .collect();
            if sounds.is_empty() {
                ui.label(
                    RichText::new(self.t(
                        "В этой группе настройки skin.ini — раскрой её и выбери элемент.",
                        "This group holds skin.ini settings — expand it and pick an element.",
                    ))
                    .color(INK_MUTED),
                );
                return;
            }
            for (leaf_id, label, stem) in sounds {
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 2.0 * (CONTROL + 8.0)).max(60.0);
                    let (rect, _) = ui.allocate_exact_size(vec2(width, CONTROL), Sense::hover());
                    // The catalog code tells same-named sounds of different sets apart.
                    let code = text_at(
                        ui,
                        pos2(rect.left(), rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        &super::editor::element_code(&leaf_id),
                        condensed(15.0),
                        INK_SUBTLE,
                    );
                    let x = rect.left() + code.width().max(36.0) + 8.0;
                    let galley = truncated(ui, &label, font(13.5), INK, rect.right() - x);
                    ui.painter().galley(
                        pos2(x, rect.center().y - galley.size().y / 2.0),
                        galley,
                        INK,
                    );
                    for (side, source) in [
                        (before.title, self.previous(&leaf_id)),
                        (after.title, self.effective(&leaf_id)),
                    ] {
                        let origin = self.origin(&source);
                        let enabled = !matches!(origin, Origin::Blank);
                        let tip =
                            format!("{side}: {}", source_name(self.settings.language, &source));
                        let kind = if side == after.title {
                            Kind::Secondary
                        } else {
                            Kind::Ghost
                        };
                        if btn(kind, "")
                            .icon(icons::play())
                            .enabled(enabled)
                            .show(ui)
                            .on_hover_text(tip)
                            .clicked()
                            && let Origin::Folder(folder) = origin
                        {
                            self.play(folder.as_deref(), &stem);
                        }
                    }
                });
            }
            ui.add_space(4.0);
            ui.label(caption(format!(
                "{}: {} · {}: {}",
                before.title,
                self.t("левая кнопка", "left button"),
                after.title,
                self.t("правая", "right button")
            )));
            return;
        }
        ui.horizontal(|ui| {
            let options = [(true, after.title), (false, before.title)];
            tabs(ui, &mut self.group_after, &options);
            ui.label(caption(source_name(
                self.settings.language,
                if self.group_after {
                    &after.source
                } else {
                    &before.source
                },
            )));
        });
        ui.add_space(10.0);
        let columns = 3;
        let gap = 8.0;
        let width = ui.available_width();
        let tile = (width - gap * (columns - 1) as f32) / columns as f32;
        for row in images.chunks(columns) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (leaf_id, label, (stem, animated)) in row {
                    let source = if self.group_after {
                        self.effective(leaf_id)
                    } else {
                        self.previous(leaf_id)
                    };
                    let origin = self.origin(&source);
                    let state = self.element(&origin, stem, *animated, 160);
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(tile, tile + 22.0), Sense::click());
                    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label));
                    let image_rect = egui::Rect::from_min_size(rect.min, Vec2::splat(tile));
                    checkerboard(ui, image_rect, RADIUS_SM);
                    let fit = shared_fit(&state, &None, image_rect.shrink(8.0));
                    let tint = self.combo_tint(&origin, stem);
                    self.paint_state(ui, image_rect, &state, None, fit, tint);
                    if response.hovered() {
                        ui.painter().rect_stroke(
                            image_rect,
                            RADIUS_SM,
                            Stroke::new(1.0, LINE_200),
                            egui::StrokeKind::Inside,
                        );
                    }
                    code_tab(
                        ui,
                        image_rect.min,
                        &super::editor::element_code(leaf_id),
                        true,
                    );
                    let galley = truncated(ui, label, font(12.5), INK_MUTED, tile);
                    ui.painter().galley(
                        pos2(rect.min.x, image_rect.max.y + 4.0),
                        galley,
                        INK_MUTED,
                    );
                    focus_ring(ui, &response, rect.shrink(3.0));
                    if response
                        .on_hover_text(label.as_str())
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        let leaf_id = leaf_id.clone();
                        let editor = self.editor_mut();
                        let mut prefix = String::new();
                        for part in leaf_id.split('.') {
                            prefix = if prefix.is_empty() {
                                part.to_owned()
                            } else {
                                format!("{prefix}.{part}")
                            };
                            if prefix != leaf_id {
                                editor.expanded.insert(prefix.clone());
                            }
                        }
                        editor.focus = Some(leaf_id);
                    }
                }
            });
            ui.add_space(4.0);
        }
    }

    // ------------------------------------------------------------ scene

    /// Everything a scene's look depends on, hashed: the picks, the base, the target skins,
    /// the colour edits and the library itself.
    fn scene_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        fn source(s: &Source, h: &mut impl Hasher) {
            std::mem::discriminant(s).hash(h);
            if let Source::Skin(p) = s {
                p.hash(h);
            }
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.page as u8).hash(&mut h);
        let mut choices: Vec<_> = self.editor_ref().choices.iter().collect();
        choices.sort_unstable_by(|a, b| a.0.cmp(b.0));
        for (id, s) in choices {
            id.hash(&mut h);
            source(s, &mut h);
        }
        self.mix_base.hash(&mut h);
        self.focus_skin.hash(&mut h);
        self.target_skin().hash(&mut h);
        self.edit_colors.hash(&mut h);
        self.colors.hash(&mut h);
        self.skins_generation.hash(&mut h);
        h.finish()
    }

    /// The selections and skin.ini a scene draws, rebuilt only when the mix changes
    /// instead of on every animation frame.
    pub(crate) fn scene_inputs(&self) -> (Rc<Vec<Selection>>, SkinIni) {
        let key = self.scene_key();
        if let Some(memo) = &*self.scene_memo.borrow()
            && memo.key == key
        {
            return (memo.selections.clone(), memo.ini.clone());
        }
        let selections = Rc::new(self.selections());
        let ini = self.scene_ini(&selections);
        *self.scene_memo.borrow_mut() = Some(SceneMemo {
            key,
            selections: selections.clone(),
            ini: ini.clone(),
            origins: Default::default(),
        });
        (selections, ini)
    }

    pub(crate) fn scene_origin(&self, selections: &[Selection], stem: &str, audio: bool) -> Origin {
        if self.page == Page::Library {
            return Origin::Folder(self.focus_skin.clone());
        }
        let file = format!("{stem}.{}", if audio { "wav" } else { "png" });
        let find = || {
            selections
                .iter()
                .find(|s| s.target.matches(Path::new(&file)))
                .map(|s| s.source.clone())
                .unwrap_or_else(|| self.base_source())
        };
        // Sprites of the memoized selections remember their source between frames.
        let memo = self.scene_memo.borrow();
        let source = match &*memo {
            Some(memo) if std::ptr::eq(memo.selections.as_ptr(), selections.as_ptr()) => {
                let known = memo.origins.borrow().get(&file).cloned();
                known.unwrap_or_else(|| {
                    let source = find();
                    memo.origins
                        .borrow_mut()
                        .insert(file.clone(), source.clone());
                    source
                })
            }
            _ => find(),
        };
        drop(memo);
        self.origin(&source)
    }

    pub(crate) fn scene_ini(&self, selections: &[Selection]) -> SkinIni {
        if self.page == Page::Library {
            return self
                .focus_skin
                .as_ref()
                .and_then(|p| self.skin(p))
                .map(|s| s.ini.clone())
                .unwrap_or_default();
        }
        let mut ini = if self.page == Page::Modify {
            self.target_skin()
                .and_then(|p| self.skin(&p).map(|s| s.ini.clone()))
                .unwrap_or_default()
        } else {
            SkinIni::default()
        };
        for selection in selections {
            if let Target::Property { section, key } = &selection.target {
                if !section.eq_ignore_ascii_case("Colours")
                    && !section.eq_ignore_ascii_case("Fonts")
                {
                    continue;
                }
                match &selection.source {
                    Source::Skin(path) => {
                        ini.section_mut(section).remove(key);
                        if let Some(value) = self.skin(path).and_then(|s| s.ini.get(section, key)) {
                            ini.set(section, key, value);
                        }
                    }
                    Source::Default | Source::Blank => ini.section_mut(section).remove(key),
                    Source::Unchanged => {}
                }
            }
        }
        if self.page == Page::Modify && self.edit_colors {
            let _ = skinweaver::modify::set_combo_colors(&mut ini, &self.colors);
        }
        ini
    }

    fn scene(&mut self, ui: &mut egui::Ui) {
        let (selections, ini) = if self.page == Page::Library {
            let none = Vec::new();
            let ini = self.scene_ini(&none);
            (Rc::new(none), ini)
        } else {
            self.scene_inputs()
        };
        let colors = ini.colors();
        let width = ui.available_width();
        let (rect, response) = ui.allocate_exact_size(vec2(width, width * 0.78), Sense::click());
        ui.painter().rect(
            rect,
            0,
            BERRY,
            Stroke::new(1.0, LINE_100),
            egui::StrokeKind::Inside,
        );
        let origin = |s: &str| self.scene_origin(&selections, s, false);
        let (circle_o, overlay_o, approach_o, cursor_o, trail_o) = (
            origin("hitcircle"),
            origin("hitcircleoverlay"),
            origin("approachcircle"),
            origin("cursor"),
            origin("cursortrail"),
        );
        let font_o = if self.page == Page::Library {
            Origin::Folder(self.focus_skin.clone())
        } else {
            let source = selections
                .iter()
                .find(|s| matches!(&s.target, Target::Property { section, key } if section == "Fonts" && key == "HitCirclePrefix"))
                .map(|s| s.source.clone())
                .unwrap_or_else(|| self.base_source());
            self.origin(&source)
        };
        let prefix = match &font_o {
            Origin::Folder(Some(p)) => self
                .skin(p)
                .and_then(|s| s.ini.get("Fonts", "HitCirclePrefix"))
                .unwrap_or("default")
                .replace('\\', "/"),
            _ => "default".into(),
        };
        let circle = self.element(&circle_o, "hitcircle", false, 256);
        let overlay = self.element(&overlay_o, "hitcircleoverlay", false, 256);
        let approach = self.element(&approach_o, "approachcircle", false, 256);
        let size = width * 0.24;
        let positions = [(0.24, 0.62), (0.52, 0.30), (0.78, 0.62)];
        let t = if self.settings.reduce_motion {
            0.5
        } else {
            (ui.input(|i| i.time) * 0.6).fract() as f32
        };
        for (i, (x, y)) in positions.iter().enumerate().rev() {
            let center = rect.min + vec2(width * x, rect.height() * y);
            let color = colors[i % colors.len()];
            let tint = rgb(color);
            if i == 0
                && let Some(State::Ready(e)) = &approach
            {
                let scale = 1.0 + 1.6 * (1.0 - t);
                paint_centered(
                    ui,
                    &e.frames[0],
                    center,
                    size * scale,
                    tint.gamma_multiply(t.max(0.25)),
                );
            }
            if let Some(State::Ready(e)) = &circle {
                paint_centered(ui, &e.frames[0], center, size, tint);
            }
            if let Some(State::Ready(e)) = &overlay {
                paint_centered(ui, &e.frames[0], center, size, Color32::WHITE);
            }
            let digit = format!("{prefix}-{}", i + 1);
            match self.element(&font_o, &digit, false, 128) {
                Some(State::Ready(e)) => {
                    paint_centered(ui, &e.frames[0], center, size * 0.36, Color32::WHITE)
                }
                _ => {
                    text_at(
                        ui,
                        center,
                        egui::Align2::CENTER_CENTER,
                        &(i + 1).to_string(),
                        condensed(size * 0.5),
                        Color32::WHITE,
                    );
                }
            }
        }
        if !self.settings.reduce_motion {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
        let hide_cursor = self.page == Page::Modify && self.flags.invisible_cursor;
        if !hide_cursor {
            let tint = if self.page == Page::Modify && self.edit_cursor {
                rgb(self.cursor_color)
            } else {
                Color32::WHITE
            };
            let cursor_pos = rect.min + vec2(width * 0.44, rect.height() * 0.74);
            if let Some(State::Ready(e)) = self.element(&trail_o, "cursortrail", false, 128) {
                for k in 1..=5 {
                    let p =
                        cursor_pos - vec2(k as f32 * width * 0.022, -(k as f32) * width * 0.008);
                    paint_centered(
                        ui,
                        &e.frames[0],
                        p,
                        width * 0.07,
                        tint.gamma_multiply(1.0 - k as f32 / 6.0),
                    );
                }
            }
            if let Some(State::Ready(e)) = self.element(&cursor_o, "cursor", false, 256) {
                paint_centered(ui, &e.frames[0], cursor_pos, width * 0.13, tint);
            }
        }
        if response.clicked() {
            let origin = self.scene_origin(&selections, SAMPLES[self.sound_sample], true);
            if let Origin::Folder(folder) = origin {
                self.play(folder.as_deref(), SAMPLES[self.sound_sample]);
            }
        }
        response.on_hover_text(self.t("Нажми, чтобы услышать звук", "Click to hear the sound"));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            for color in &colors {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 7.0, rgb(*color));
            }
            ui.label(caption(self.t("цвета комбо", "combo colours")));
        });
        ui.add_space(14.0);
        ui.label(RichText::new(self.t("Звук", "Sound")).font(font_strong(13.0)));
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("sound_sample")
                .icon(combo_chevron)
                .width((ui.available_width() - 90.0).max(100.0))
                .selected_text(RichText::new(SAMPLES[self.sound_sample]).size(13.5))
                .show_ui(ui, |ui| {
                    for (i, label) in SAMPLES.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.sound_sample,
                            i,
                            RichText::new(*label).size(13.5),
                        );
                    }
                });
            if btn(Kind::Secondary, "")
                .icon(icons::play())
                .show(ui)
                .accessible_name(self.t("Слушать", "Play"))
                .on_hover_text(self.t("Слушать", "Play"))
                .clicked()
            {
                let origin = self.scene_origin(&selections, SAMPLES[self.sound_sample], true);
                match origin {
                    Origin::Folder(folder) => {
                        self.play(folder.as_deref(), SAMPLES[self.sound_sample])
                    }
                    Origin::Blank => self.stop_audio(),
                }
            }
            if btn(Kind::Secondary, "")
                .icon(icons::square())
                .show(ui)
                .accessible_name(self.t("Стоп", "Stop"))
                .on_hover_text(self.t("Стоп", "Stop"))
                .clicked()
            {
                self.stop_audio();
            }
        });
    }

    // ------------------------------------------------------------ library

    fn skin_panel(&mut self, ui: &mut egui::Ui) {
        let Some(path) = self.focus_skin.clone() else {
            ui.add_space(8.0);
            hit_mark(ui, 40.0);
            ui.add_space(8.0);
            ui.label(RichText::new(self.t("Выбери скин", "Pick a skin")).font(condensed(30.0)));
            ui.label(
                RichText::new(self.t(
                    "Здесь будут его элементы, сцена и действия.",
                    "Its elements, a scene and actions will show up here.",
                ))
                .color(INK_MUTED),
            );
            return;
        };
        if self.skin(&path).is_none() {
            return;
        }
        self.scene(ui);
        ui.add_space(20.0);
        self.skin_gallery(ui, &path);
    }

    fn skin_header(&mut self, ui: &mut egui::Ui) {
        let Some(path) = self.focus_skin.clone() else {
            return;
        };
        let Some(skin) = self.skin(&path).cloned() else {
            return;
        };
        let code = self
            .skin_index
            .get(&path)
            .and_then(|&i| self.skin_codes.get(i))
            .cloned()
            .unwrap_or_default();
        // The skin as a specimen: its name on the plum plate, rings in its own combo colours.
        let colors: Vec<Color32> = skin.ini.colors().into_iter().map(rgb).collect();
        let language = self.settings.language;
        let mut open = false;
        plate(ui, Rings::Ready, &colors, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(0.0, 34.0), Sense::hover());
                let tab = code_tab(ui, pos2(r.left(), r.center().y - 9.0), &code, true);
                ui.add_space(tab.width() + 4.0);
                ui.add(egui::Label::new(RichText::new(&skin.name).font(serif(32.0))).truncate());
            });
            ui.add_space(-4.0);
            ui.add(
                egui::Label::new(
                    RichText::new(if skin.author.is_empty() {
                        language.text("Автор не указан", "Unknown author")
                    } else {
                        &skin.author
                    })
                    .color(PLUM_MUTED),
                )
                .truncate(),
            );
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        elements_label(language, skin.elements),
                        megabytes(language, skin.bytes)
                    ))
                    .size(12.5)
                    .color(PLUM_MUTED),
                );
                if skin.hidden {
                    ui.label(
                        RichText::new(language.text("· скрыт", "· hidden"))
                            .size(12.5)
                            .color(PLUM_MUTED),
                    );
                }
            });
            ui.add_space(8.0);
            open = btn(
                Kind::OnPlate,
            language.text("Открыть в редакторе", "Open in editor"),
            )
            .icon(icons::wand_sparkles())
            .show(ui)
            .on_hover_text(language.text(
                "Откроет скин в «Собрать скин»: каждый экран osu! целиком, любой элемент можно заменить.",
                "Opens the skin in Mix skins: every osu! screen in full, any element can be replaced.",
            ))
            .clicked();
        });
        if open {
            self.page = Page::Mix;
            self.stage_view = true;
            self.mix.choices.clear();
            self.mix.previous.clear();
            self.mix.focus = None;
            self.set_mix_base(Some(path));
            self.preview_tab = PreviewTab::Variants;
        }
        ui.add_space(12.0);
    }

    fn skin_gallery(&mut self, ui: &mut egui::Ui, path: &Path) {
        let path = path.to_path_buf();
        let Some(skin) = self.skin(&path).cloned() else {
            return;
        };
        section_title(ui, self.t("Элементы", "Elements"));
        ui.label(caption(self.t(
            "Нажми на элемент, чтобы сравнить его со стандартным.",
            "Click an element to compare it with the default one.",
        )));
        ui.add_space(8.0);
        let origin = Origin::Folder(Some(path.clone()));
        // The skin's own files first; elements it lacks collapse into one line.
        let mut present = vec![];
        let mut missing = vec![];
        for (index, &(stem, animated, ru, en)) in GALLERY.iter().enumerate() {
            let state = self.element(&origin, stem, animated, 128);
            match &state {
                Some(State::Ready(e)) if !e.fallback => present.push((index, state)),
                Some(State::Loading) => present.push((index, state)),
                _ => missing.push(self.t(ru, en)),
            }
        }
        let columns = 3;
        let gap = 8.0;
        let tile = (ui.available_width() - gap * (columns - 1) as f32) / columns as f32;
        for row in present.chunks(columns) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for (index, state) in row {
                    let (stem, _, ru, en) = GALLERY[*index];
                    let (rect, response) =
                        ui.allocate_exact_size(vec2(tile, tile + 22.0), Sense::click());
                    let image_rect = egui::Rect::from_min_size(rect.min, Vec2::splat(tile));
                    let selected = self.focus_element == Some(*index);
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(egui::Role::Button, ui.is_enabled(), selected, self.t(ru, en))
                    });
                    checkerboard(ui, image_rect, RADIUS_SM);
                    let fit = shared_fit(state, &None, image_rect.shrink(8.0));
                    let tint = self.combo_tint(&origin, stem);
                    self.paint_state(ui, image_rect, state, None, fit, tint);
                    code_tab(ui, image_rect.min, &format!("{:02}", index + 1), !selected);
                    let stroke = if selected {
                        Stroke::new(2.0, ACCENT_LINE)
                    } else if response.hovered() {
                        Stroke::new(1.0, LINE_200)
                    } else {
                        Stroke::NONE
                    };
                    ui.painter()
                        .rect_stroke(image_rect, 0, stroke, egui::StrokeKind::Inside);
                    let label = self.t(ru, en);
                    let galley = truncated(
                        ui,
                        label,
                        if selected {
                            font_strong(12.5)
                        } else {
                            font(12.5)
                        },
                        INK,
                        tile,
                    );
                    ui.painter()
                        .galley(pos2(rect.min.x, image_rect.max.y + 4.0), galley, INK);
                    focus_ring(ui, &response, rect.shrink(3.0));
                    if response
                        .on_hover_text(label)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.focus_element = if selected { None } else { Some(*index) };
                    }
                }
            });
            ui.add_space(2.0);
        }
        if present.is_empty() {
            ui.label(
                RichText::new(self.t(
                    "В этом скине нет ни одного из ключевых элементов — osu! покажет стандартные.",
                    "This skin has none of the key elements, so osu! shows the default ones.",
                ))
                .color(INK_MUTED),
            );
        }
        if !missing.is_empty() && !present.is_empty() {
            ui.add_space(4.0);
            ui.label(caption(format!(
                "{} {} — {}",
                self.t("Нет в скине:", "Not in this skin:"),
                missing.join(", "),
                self.t("osu! возьмёт стандартные.", "osu! uses the default ones.")
            )));
        }
        ui.add_space(8.0);
        if let Some(index) = self.focus_element
            && let Some(&(stem, animated, ru, en)) = GALLERY.get(index)
        {
            ui.label(RichText::new(self.t(ru, en)).font(font_strong(14.0)));
            ui.add_space(6.0);
            let before = Side {
                title: self.t("Стандартный", "Default"),
                source: Source::Default,
                origin: Origin::Folder(None),
            };
            let after = Side {
                title: self.t("Этот скин", "This skin"),
                source: Source::Skin(path.clone()),
                origin: Origin::Folder(Some(path.clone())),
            };
            self.compare_images(ui, stem, animated, before, after);
            ui.add_space(14.0);
        }
        ui.add_space(20.0);
        section_title(ui, self.t("Действия", "Actions"));
        ui.add_space(4.0);
        if btn(
            Kind::Secondary,
            self.t("Взять весь скин в микс", "Use whole skin in a mix"),
        )
        .icon(icons::blend())
        .show(ui)
        .clicked()
        {
            self.page = Page::Mix;
            for i in 0..self.nodes.len() {
                self.set_choice(&i.to_string(), Source::Skin(path.clone()));
            }
            self.mix.focus = None;
            self.preview_tab = PreviewTab::Scene;
        }
        ui.horizontal_wrapped(|ui| {
            if btn(Kind::Secondary, self.t("Изменить", "Modify"))
                .icon(icons::wand_sparkles())
                .show(ui)
                .clicked()
            {
                self.selected.clear();
                self.selected.insert(path.clone());
                self.page = Page::Modify;
            }
            if btn(Kind::Secondary, self.t("Папка", "Folder"))
                .icon(icons::folder_open())
                .show(ui)
                .clicked()
            {
                self.report(open::that(&path).map_err(anyhow::Error::from));
            }
            if btn(Kind::Secondary, self.t("Открыть в osu!", "Open in osu!"))
                .icon(icons::gamepad_2())
                .show(ui)
                .clicked()
            {
                self.open_in_osu(ui.ctx(), path.clone());
            }
        });
        ui.add_space(12.0);
        egui::CollapsingHeader::new(
            RichText::new(self.t("skin.ini и авторы", "skin.ini and credits"))
                .font(font_strong(13.0)),
        )
        .id_salt("ini")
        .show(ui, |ui| {
            ui.label(
                RichText::new(skin.ini.to_string())
                    .monospace()
                    .size(12.0)
                    .color(INK_MUTED),
            );
            if let Ok(credits) = fs::read_to_string(skin.path.join("credits.ini")) {
                ui.separator();
                ui.label(
                    RichText::new(credits)
                        .monospace()
                        .size(12.0)
                        .color(INK_MUTED),
                );
            }
        });
    }

    fn open_in_osu(&mut self, ctx: &egui::Context, path: PathBuf) {
        let Some(library) = self.library.clone() else {
            return;
        };
        let data = self.data.clone();
        self.spawn(ctx, move |_, _| {
            library.validate_skin(&path)?;
            anyhow::ensure!(
                path.parent() == Some(library.root.join("Skins").as_path()),
                "Сначала покажи скрытый скин / Unhide this skin first"
            );
            let executable = library.root.join("osu!.exe");
            anyhow::ensure!(
                executable.is_file(),
                "osu!.exe нет в выбранной папке / osu!.exe was not found in the selected folder"
            );
            let folder = data.join("exports");
            fs::create_dir_all(&folder)?;
            let destination = folder.join(format!(
                "{}.osk",
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ));
            // osu! selects an installed skin when importing an empty archive of the same name.
            let archive = zip::ZipWriter::new(fs::File::create(&destination)?);
            archive.finish()?;
            let mut command = std::process::Command::new(executable);
            command.arg(&destination);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            command.spawn()?;
            Ok(Outcome::Exported(destination))
        });
    }
}

fn collect_leaves<'a>(node: &'a OptionNode, id: &str, out: &mut Vec<(String, &'a OptionNode)>) {
    if node.target.is_some() {
        out.push((id.to_owned(), node));
    }
    for (i, child) in node.children.iter().enumerate() {
        collect_leaves(child, &format!("{id}.{i}"), out);
    }
}

/// One scale for both sides so their real sizes stay comparable.
fn shared_fit(a: &Option<State>, b: &Option<State>, area: egui::Rect) -> f32 {
    let size = |s: &Option<State>| match s {
        Some(State::Ready(e)) => e.logical,
        _ => Vec2::ZERO,
    };
    let largest = size(a).max(size(b));
    if largest.x <= 0.0 || largest.y <= 0.0 {
        return 1.0;
    }
    (area.width() / largest.x)
        .min(area.height() / largest.y)
        .min(3.0)
}
