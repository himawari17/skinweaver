//! Design tokens and hand-drawn widgets.
//!
//! The look follows the kohu! site: a dark layered ground (000 backdrop → 400 pressed),
//! one accent spent only on the moment, ink primary buttons, the accent as a state mark
//! (ACCENT_LINE), tinted grounds for current items, 2px focus rings, book-like display
//! type. The palette is the original skin mixer's: plum-tinted greys, soft osu! pink
//! for the moment and the state marks, lavender violet for catalog codes and focus.
//! Type: Cormorant Garamond for titles and group letters, Onest for the interface,
//! JetBrains Mono for paths and keys.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontFamily, FontId, Rect, Response, Sense, Stroke,
    StrokeKind, Vec2, pos2, vec2,
};

// Layers follow kohu!'s depth order (000 backdrop → 400 pressed), on plum-tinted dark.
/// Backdrop layer: navigation pane, window ground.
pub const BG_000: Color32 = Color32::from_rgb(0x1c, 0x1a, 0x1f);
/// Page ground.
pub const BG_100: Color32 = Color32::from_rgb(0x22, 0x1e, 0x24);
/// Raised layer: preview column, dialogs, popups, notices.
pub const BG_200: Color32 = Color32::from_rgb(0x2b, 0x25, 0x2c);
/// Control rest fill and row hover.
pub const BG_300: Color32 = Color32::from_rgb(0x35, 0x2d, 0x36);
/// Control hover / pressed.
pub const BG_400: Color32 = Color32::from_rgb(0x40, 0x36, 0x3f);
/// Hairlines between rows and sections (decorative).
pub const LINE_100: Color32 = Color32::from_rgb(0x3a, 0x32, 0x3b);
/// Frames that identify a control (3:1 on every layer).
pub const LINE_200: Color32 = Color32::from_rgb(0x8d, 0x84, 0x94);

/// Main text: warm lavender white.
pub const INK: Color32 = Color32::from_rgb(0xf4, 0xee, 0xf3);
pub const INK_MUTED: Color32 = Color32::from_rgb(0xcb, 0xc3, 0xcf);
pub const INK_SUBTLE: Color32 = Color32::from_rgb(0xa5, 0x9c, 0xab);
/// Dark text on ink-filled (primary) buttons and inverse code tabs.
pub const ON_INK: Color32 = Color32::from_rgb(0x1c, 0x1a, 0x1f);

/// The original's soft pink: spent only on the moment (create the skin, apply changes).
pub const ACCENT: Color32 = Color32::from_rgb(0xef, 0xb8, 0xc8);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0xf5, 0xc8, 0xd5);
pub const ACCENT_PRESSED: Color32 = Color32::from_rgb(0xe0, 0xac, 0xc1);
/// Deep plum text on the pink moment button.
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x49, 0x25, 0x32);
/// Pink as a state mark: selected tab, toggle on, checked box, progress, focused field
/// (3:1 on every layer, like kohu!'s amber-line).
pub const ACCENT_LINE: Color32 = Color32::from_rgb(0xee, 0x86, 0xba);
/// Pink text and links.
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0xff, 0xb3, 0xd4);
/// Plum tinted ground: the current navigation item, selected rows, the demo badge.
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0x4a, 0x2c, 0x43);
/// Lavender violet: catalog codes and the focus ring.
pub const VIOLET: Color32 = Color32::from_rgb(0xc9, 0xa8, 0xff);
/// Violet tinted ground for code tabs.
pub const VIOLET_SOFT: Color32 = Color32::from_rgb(0x38, 0x2d, 0x4c);
/// Deep plum plate (kohu!'s resin): the specimen that carries a screen's moment.
pub const PLUM: Color32 = Color32::from_rgb(0x49, 0x25, 0x32);
/// Plum one step up: fields, key caps and hover on the plate.
pub const PLUM_RAISED: Color32 = Color32::from_rgb(0x5c, 0x30, 0x43);
/// Text on the plate (kohu!'s honey), 10:1.
pub const PLUM_TEXT: Color32 = Color32::from_rgb(0xff, 0xd8, 0xe4);
/// Secondary text and dim rings on the plate (kohu!'s honey-muted), 7:1.
pub const PLUM_MUTED: Color32 = Color32::from_rgb(0xe7, 0xb4, 0xc8);
/// Control frames on the plate, 4.5:1.
pub const PLUM_LINE: Color32 = Color32::from_rgb(0xb9, 0x8a, 0xa0);
/// Ground of the gameplay scene and every element cut.
pub const BERRY: Color32 = Color32::from_rgb(0x17, 0x14, 0x1a);
pub const CUT: Color32 = Color32::from_rgb(0x14, 0x12, 0x16);
pub const CUT_CHECK: Color32 = Color32::from_rgb(0x1d, 0x1a, 0x1f);

pub const SUCCESS: Color32 = Color32::from_rgb(0x5f, 0xd3, 0xc3);
pub const DANGER: Color32 = Color32::from_rgb(0xff, 0xb0, 0xa6);
pub const INFO: Color32 = Color32::from_rgb(0x9c, 0xc4, 0xff);
pub const WARN: Color32 = Color32::from_rgb(0xf0, 0xc2, 0x7a);
/// Keyboard focus ring: 2px with a 2px offset.
pub const FOCUS: Color32 = Color32::from_rgb(0xd6, 0xb8, 0xff);

pub const RADIUS_SM: u8 = 4;
pub const RADIUS_MD: u8 = 8;
pub const RADIUS_LG: u8 = 12;
pub const CONTROL: f32 = 32.0;

pub const NAV_WIDE: f32 = 208.0;
pub const NAV_NARROW: f32 = 60.0;

pub fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}
/// Cormorant Garamond SemiBold: page and section titles, group letters.
pub fn display() -> FontFamily {
    FontFamily::Name("display".into())
}
pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}
pub fn font_strong(size: f32) -> FontId {
    FontId::new(size, semibold())
}
/// Display type by size: from 20px up it is Cormorant Garamond (set a little larger,
/// its x-height is small); below that — codes, counters, badges — Onest SemiBold.
pub fn condensed(size: f32) -> FontId {
    if size >= 20.0 {
        FontId::new((size * 1.12).round(), display())
    } else {
        FontId::new((size * 0.9).round(), semibold())
    }
}
/// Cormorant Garamond at exactly `size`.
pub fn serif(size: f32) -> FontId {
    FontId::new(size, display())
}

pub fn install(ctx: &egui::Context, reduce_motion: bool, scale: f32) {
    let mut fonts = egui::FontDefinitions::default();
    let data = [
        (
            "onest",
            &include_bytes!("../../assets/fonts/Onest-Regular.ttf")[..],
        ),
        (
            "onest-semibold",
            &include_bytes!("../../assets/fonts/Onest-SemiBold.ttf")[..],
        ),
        (
            "cormorant",
            &include_bytes!("../../assets/fonts/CormorantGaramond-SemiBold.ttf")[..],
        ),
        (
            "jetbrains",
            &include_bytes!("../../assets/fonts/JetBrainsMono-Regular.ttf")[..],
        ),
    ];
    for (name, bytes) in data {
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_static(bytes).into());
    }
    let fallback: Vec<String> = fonts.families[&FontFamily::Proportional].clone();
    let family = |first: &str| {
        let mut list = vec![first.to_owned()];
        list.extend(fallback.iter().cloned());
        list
    };
    fonts
        .families
        .insert(FontFamily::Proportional, family("onest"));
    fonts.families.insert(semibold(), family("onest-semibold"));
    fonts.families.insert(display(), family("cormorant"));
    let mono = fonts.families.entry(FontFamily::Monospace).or_default();
    mono.insert(0, "jetbrains".into());
    ctx.set_fonts(fonts);
    ctx.set_zoom_factor(scale);
    ctx.set_theme(egui::Theme::Dark);

    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();
    use egui::TextStyle::*;
    style.text_styles = [
        (Heading, font_strong(18.0)),
        (Body, font(14.0)),
        (Button, font_strong(13.5)),
        (Small, font(12.5)),
        (Monospace, FontId::monospace(12.5)),
    ]
    .into();
    style.spacing.item_spacing = vec2(8.0, 8.0);
    style.spacing.button_padding = vec2(10.0, 5.0);
    style.spacing.interact_size.y = CONTROL;
    style.spacing.combo_height = 360.0;
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = true;
    style.spacing.scroll.foreground_color = false;
    style.spacing.scroll.dormant_handle_opacity = 0.0;
    style.spacing.scroll.active_handle_opacity = 0.6;
    // kohu! motion: base 120 ms, hover changes colour only, press moves 1px.
    style.animation_time = if reduce_motion { 0.0 } else { 0.12 };
    style.interaction.selectable_labels = false;

    let v = &mut style.visuals;
    v.dark_mode = true;
    v.panel_fill = BG_100;
    v.window_fill = BG_200;
    v.extreme_bg_color = BG_200;
    v.faint_bg_color = BG_300;
    v.code_bg_color = BG_300;
    v.text_edit_bg_color = Some(BG_300);
    v.override_text_color = Some(INK);
    v.weak_text_color = Some(INK_SUBTLE);
    v.hyperlink_color = ACCENT_TEXT;
    v.error_fg_color = DANGER;
    v.warn_fg_color = WARN;
    v.selection.bg_fill = ACCENT_SOFT;
    v.selection.stroke = Stroke::new(2.0, ACCENT_LINE);
    v.window_corner_radius = CornerRadius::same(RADIUS_MD);
    v.menu_corner_radius = CornerRadius::same(RADIUS_MD);
    v.window_stroke = Stroke::new(1.0, LINE_100);
    v.window_shadow = egui::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    v.popup_shadow = egui::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(90),
    };
    v.text_cursor.stroke = Stroke::new(2.0, ACCENT_LINE);
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };
    v.slider_trailing_fill = true;
    v.striped = false;
    let w = &mut v.widgets;
    for widget in [&mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open] {
        widget.corner_radius = CornerRadius::same(RADIUS_SM);
        widget.fg_stroke = Stroke::new(1.0, INK);
        widget.expansion = 0.0;
    }
    w.noninteractive.bg_fill = BG_200;
    w.noninteractive.weak_bg_fill = BG_200;
    w.noninteractive.bg_stroke = Stroke::new(1.0, LINE_100);
    w.noninteractive.fg_stroke = Stroke::new(1.0, INK);
    w.noninteractive.corner_radius = CornerRadius::same(RADIUS_SM);
    w.inactive.bg_fill = BG_300;
    w.inactive.weak_bg_fill = BG_300;
    w.inactive.bg_stroke = Stroke::new(1.0, LINE_200);
    w.hovered.bg_fill = BG_300;
    w.hovered.weak_bg_fill = BG_300;
    w.hovered.bg_stroke = Stroke::new(1.0, INK_SUBTLE);
    w.active.bg_fill = BG_400;
    w.active.weak_bg_fill = BG_400;
    w.active.bg_stroke = Stroke::new(1.0, INK_SUBTLE);
    w.open.bg_fill = BG_200;
    w.open.weak_bg_fill = BG_200;
    w.open.bg_stroke = Stroke::new(1.0, INK_SUBTLE);
    ctx.set_style_of(egui::Theme::Dark, style);
}

pub fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

// ---------------------------------------------------------------- widgets

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    /// Black ink fill: the main action of a section.
    Primary,
    /// Pink spot fill: the one moment of a screen (create the skin, apply changes).
    Accent,
    Secondary,
    Ghost,
    Danger,
    /// Outline button on the plum plate (kohu!'s on-resin).
    OnPlate,
}

pub struct Btn<'a> {
    pub kind: Kind,
    pub icon: Option<egui::ImageSource<'a>>,
    pub label: &'a str,
    pub enabled: bool,
    pub large: bool,
    pub small: bool,
    pub min_width: f32,
    pub selected: bool,
}

pub fn btn<'a>(kind: Kind, label: &'a str) -> Btn<'a> {
    Btn {
        kind,
        icon: None,
        label,
        enabled: true,
        large: false,
        small: false,
        min_width: 0.0,
        selected: false,
    }
}

impl<'a> Btn<'a> {
    pub fn icon(mut self, icon: egui::ImageSource<'a>) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn large(mut self) -> Self {
        self.large = true;
        self
    }
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }
    pub fn selected(mut self, s: bool) -> Self {
        self.selected = s;
        self
    }
    pub fn show(self, ui: &mut egui::Ui) -> Response {
        let enabled = self.enabled && ui.is_enabled();
        let height = if self.large {
            44.0
        } else if self.small {
            26.0
        } else {
            CONTROL
        };
        let font = if self.large {
            font_strong(15.0)
        } else {
            font_strong(13.5)
        };
        let icon_size = if self.large { 18.0 } else { 15.0 };
        let pad = if self.small { 8.0 } else { 12.0 };
        let has_text = !self.label.is_empty();
        let galley = ui
            .painter()
            .layout_no_wrap(self.label.to_owned(), font.clone(), INK);
        let mut width = pad * 2.0 + galley.size().x;
        if self.icon.is_some() {
            width += icon_size + if has_text { 7.0 } else { 0.0 };
        }
        if !has_text {
            width = height;
        }
        let size = vec2(width.max(self.min_width), height);
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Button, enabled, self.selected, self.label)
        });
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let hovered = enabled && response.hovered();
        let pressed = enabled && response.is_pointer_button_down_on();
        // Hover changes colour only, over kohu!'s 120 ms.
        let h = ui
            .ctx()
            .animate_bool_with_time(response.id.with("hover"), hovered || self.selected, ui.style().animation_time);
        let fade = |rest: Color32, hover: Color32| rest.lerp_to_gamma(hover, h);
        let (fill, text, stroke) = if !enabled {
            (
                if self.kind == Kind::Ghost {
                    Color32::TRANSPARENT
                } else {
                    BG_300
                },
                INK_SUBTLE,
                Stroke::NONE,
            )
        } else {
            match self.kind {
                Kind::Primary => (fade(INK, INK_MUTED), ON_INK, Stroke::NONE),
                Kind::Accent => (
                    if pressed {
                        ACCENT_PRESSED
                    } else {
                        fade(ACCENT, ACCENT_HOVER)
                    },
                    ON_ACCENT,
                    Stroke::NONE,
                ),
                Kind::Secondary => (
                    if pressed { BG_400 } else { fade(BG_300, BG_400) },
                    INK,
                    Stroke::new(1.0, if self.selected { ACCENT_LINE } else { LINE_200 }),
                ),
                Kind::Ghost => (
                    if pressed {
                        BG_400
                    } else {
                        fade(Color32::TRANSPARENT, BG_300)
                    },
                    INK,
                    Stroke::NONE,
                ),
                Kind::OnPlate => (
                    fade(Color32::TRANSPARENT, PLUM_RAISED),
                    PLUM_TEXT,
                    Stroke::new(1.0, PLUM_LINE),
                ),
                Kind::Danger => (
                    fade(Color32::TRANSPARENT, BG_300),
                    DANGER,
                    Stroke::new(1.0, DANGER),
                ),
            }
        };
        let rect = if pressed {
            rect.translate(vec2(0.0, 1.0))
        } else {
            rect
        };
        ui.painter()
            .rect(rect, RADIUS_SM, fill, stroke, StrokeKind::Inside);
        let content = (width.min(size.x) - pad * 2.0).max(0.0);
        let mut x = rect.center().x - content / 2.0;
        if !has_text {
            x = rect.center().x - icon_size / 2.0;
        }
        if let Some(icon) = self.icon {
            let icon_rect = Rect::from_min_size(
                pos2(x, rect.center().y - icon_size / 2.0),
                Vec2::splat(icon_size),
            );
            egui::Image::new(icon).tint(text).paint_at(ui, icon_rect);
            x += icon_size + 7.0;
        }
        if has_text {
            let galley = ui
                .painter()
                .layout_no_wrap(self.label.to_owned(), font, text);
            ui.painter().galley(
                pos2(x, rect.center().y - galley.size().y / 2.0),
                galley,
                text,
            );
        }
        focus_ring(ui, &response, rect);
        if self.kind == Kind::Accent {
            super::motion::moment(ui, response.id, rect, enabled, response.clicked());
        }
        if enabled {
            response.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            response
        }
    }
}

pub fn focus_ring(ui: &egui::Ui, response: &Response, rect: Rect) {
    if response.has_focus() && ui.memory(|m| m.focused().is_some()) {
        ui.painter().rect_stroke(
            rect.expand(4.0),
            RADIUS_SM + 2,
            Stroke::new(2.0, FOCUS),
            StrokeKind::Inside,
        );
    }
}

/// Icon-only ghost button with a tooltip.
pub fn icon_button(ui: &mut egui::Ui, icon: egui::ImageSource<'_>, tooltip: &str) -> Response {
    btn(Kind::Ghost, "")
        .icon(icon)
        .show(ui)
        .accessible_name(tooltip)
        .on_hover_text(tooltip)
}

/// Switch for settings that apply at once.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, label: &str) -> Response {
    let galley = ui.painter().layout(
        label.to_owned(),
        font(14.5),
        INK,
        (ui.available_width() - 50.0).max(80.0),
    );
    let height = galley.size().y.max(24.0);
    let (rect, mut response) =
        ui.allocate_exact_size(vec2(50.0 + galley.size().x, height), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::Switch, ui.is_enabled(), *on, label)
    });
    let t = ui.ctx().animate_bool(response.id, *on);
    let track = Rect::from_min_size(pos2(rect.min.x, rect.center().y - 9.0), vec2(36.0, 18.0));
    let painter = ui.painter();
    if *on {
        painter.rect_filled(track, 9, ACCENT_LINE);
    } else {
        painter.rect(
            track,
            9,
            if response.hovered() {
                BG_300
            } else {
                Color32::TRANSPARENT
            },
            Stroke::new(1.0, LINE_200),
            StrokeKind::Inside,
        );
    }
    let knob_x = egui::lerp(track.left() + 9.0..=track.right() - 9.0, t);
    painter.circle_filled(
        pos2(knob_x, track.center().y),
        if *on { 5.5 } else { 4.5 },
        if *on { BG_200 } else { INK_MUTED },
    );
    painter.galley(
        pos2(rect.min.x + 50.0, rect.center().y - galley.size().y / 2.0),
        galley,
        INK,
    );
    focus_ring(ui, &response, track);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Square checkbox for multi-selection in lists.
pub fn check(ui: &mut egui::Ui, checked: bool, label: &str) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), checked, label)
    });
    let boxr = Rect::from_center_size(rect.center(), Vec2::splat(16.0));
    let painter = ui.painter();
    if checked {
        painter.rect_filled(boxr, RADIUS_SM, ACCENT_LINE);
    } else {
        painter.rect(
            boxr,
            RADIUS_SM,
            if response.hovered() { BG_300 } else { BG_200 },
            Stroke::new(1.0, LINE_200),
            StrokeKind::Inside,
        );
    }
    if checked {
        let s = Stroke::new(2.0, BG_200);
        let c = boxr.center();
        painter.line_segment([c + vec2(-4.0, 0.0), c + vec2(-1.0, 3.2)], s);
        painter.line_segment([c + vec2(-1.0, 3.2), c + vec2(4.4, -3.4)], s);
    }
    focus_ring(ui, &response, boxr);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Segmented switch (kohu! Tabs): the selected tab sits on the page ground with a
/// 2px pink state line under it.
pub fn tabs<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    let galleys: Vec<_> = options
        .iter()
        .map(|(_, l)| {
            ui.painter()
                .layout_no_wrap((*l).to_owned(), font_strong(13.0), INK)
        })
        .collect();
    let width: f32 = galleys.iter().map(|g| g.size().x + 24.0).sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, CONTROL), Sense::hover());
    ui.painter().rect(
        rect,
        RADIUS_SM,
        BG_300,
        Stroke::new(1.0, LINE_100),
        StrokeKind::Inside,
    );
    // The selected segment slides to its place instead of jumping.
    let mut x = rect.min.x + 2.0;
    let mut target = None;
    for ((option, _), galley) in options.iter().zip(&galleys) {
        let w = galley.size().x + 24.0;
        if *value == *option {
            target = Some((x, w));
        }
        x += w;
    }
    if let Some((tx, tw)) = target {
        let id = ui.make_persistent_id(("tabs", options.len(), options[0].1));
        let ctx = ui.ctx();
        let time = if super::motion::reduced(ctx) { 0.0 } else { 0.18 };
        let sx = ctx.animate_value_with_time(id.with("x"), tx - rect.min.x, time) + rect.min.x;
        let sw = ctx.animate_value_with_time(id.with("w"), tw, time);
        let seg = Rect::from_min_size(pos2(sx, rect.min.y + 2.0), vec2(sw, rect.height() - 4.0));
        ui.painter().rect_filled(seg, 3, BG_100);
        ui.painter().rect_filled(
            Rect::from_min_max(
                pos2(seg.min.x + 8.0, seg.max.y - 2.0),
                pos2(seg.max.x - 8.0, seg.max.y),
            ),
            1,
            ACCENT_LINE,
        );
    }
    let mut x = rect.min.x + 2.0;
    for ((option, label), galley) in options.iter().zip(galleys) {
        let w = galley.size().x + 24.0;
        let tab = Rect::from_min_size(pos2(x, rect.min.y + 2.0), vec2(w, rect.height() - 4.0));
        let response = ui
            .interact(tab, ui.make_persistent_id(("tab", *label)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let selected = *value == *option;
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::Role::Tab, ui.is_enabled(), selected, *label)
        });
        let h = ui
            .ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered() && !selected, ui.style().animation_time);
        if h > 0.0 {
            ui.painter()
                .rect_filled(tab, 3, BG_400.gamma_multiply(h * 0.8));
        }
        let color = if selected { INK } else { INK_MUTED };
        let text = ui
            .painter()
            .layout_no_wrap((*label).to_owned(), font_strong(13.0), color);
        ui.painter()
            .galley(tab.center() - text.size() / 2.0, text, color);
        if response.clicked() && !selected {
            *value = *option;
            changed = true;
        }
        focus_ring(ui, &response, tab);
        x += w;
    }
    changed
}

/// Page title set in the condensed face, with an optional lead line.
pub fn page_title(ui: &mut egui::Ui, title: &str, lead: &str) {
    ui.label(egui::RichText::new(title).font(serif(40.0)).color(INK));
    if !lead.is_empty() {
        ui.add_space(-4.0);
        ui.label(egui::RichText::new(lead).size(14.0).color(INK_MUTED));
    }
    ui.add_space(18.0);
}

pub fn section_title(ui: &mut egui::Ui, title: &str) {
    ui.label(egui::RichText::new(title).font(condensed(22.0)).color(INK));
}

pub fn caption(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text).size(12.5).color(INK_SUBTLE)
}

pub fn hairline(ui: &egui::Ui, y: f32, x: std::ops::RangeInclusive<f32>) {
    ui.painter().hline(x, y, Stroke::new(1.0, LINE_100));
}

/// A catalog code tab, e.g. `B1-3`: violet on a violet tint, or inverse (ink fill).
pub fn code_tab(ui: &egui::Ui, at: egui::Pos2, code: &str, inverse: bool) -> Rect {
    let (fill, color) = if inverse {
        (VIOLET, ON_INK)
    } else {
        (VIOLET_SOFT, VIOLET)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(code.to_owned(), condensed(13.0), color);
    let rect = Rect::from_min_size(at, galley.size() + vec2(8.0, 2.0));
    ui.painter().rect_filled(rect, RADIUS_SM, fill);
    ui.painter()
        .galley(rect.min + vec2(4.0, 1.0), galley, color);
    rect
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Success,
    Danger,
    Info,
}

pub struct InfobarResult {
    pub dismissed: bool,
    pub action: Option<usize>,
}

/// Inline notice under the page title, with optional action buttons.
pub fn infobar(
    ui: &mut egui::Ui,
    tone: Tone,
    title: &str,
    text: &str,
    dismiss: &str,
    actions: &[&str],
) -> InfobarResult {
    let (icon, color) = match tone {
        Tone::Success => (super::icons::circle_check(), SUCCESS),
        Tone::Danger => (super::icons::circle_alert(), DANGER),
        Tone::Info => (super::icons::info(), INFO),
    };
    let mut result = InfobarResult {
        dismissed: false,
        action: None,
    };
    let danger_edge = tone == Tone::Danger;
    let inner = super::motion::glass(ui, BG_200, egui::Margin::symmetric(12, 9), |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.add(
                    egui::Image::new(icon)
                        .tint(color)
                        .fit_to_exact_size(Vec2::splat(18.0)),
                );
                let width = (ui.available_width() - 44.0).max(80.0);
                ui.vertical(|ui| {
                    ui.set_width(width);
                    if !title.is_empty() {
                        ui.label(egui::RichText::new(title).font(font_strong(14.5)));
                    }
                    if !text.is_empty() {
                        egui::ScrollArea::vertical()
                            .max_height(96.0)
                            .id_salt("infobar_text")
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(text).color(INK_MUTED));
                            });
                    }
                    if !actions.is_empty() {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            for (i, label) in actions.iter().enumerate() {
                                let kind = if i == 0 { Kind::Secondary } else { Kind::Ghost };
                                if btn(kind, label).small().show(ui).clicked() {
                                    result.action = Some(i);
                                }
                            }
                        });
                    }
                });
                if icon_button(ui, super::icons::x(), dismiss).clicked() {
                    result.dismissed = true;
                }
            });
        });
    if danger_edge {
        ui.painter().rect_stroke(
            inner.response.rect,
            RADIUS_MD,
            Stroke::new(1.0, DANGER),
            StrokeKind::Inside,
        );
    }
    result
}

/// Chevron for every drop-down, from the same icon set as the rest of the app.
pub fn combo_chevron(
    ui: &egui::Ui,
    rect: Rect,
    _visuals: &egui::style::WidgetVisuals,
    _open: bool,
) {
    let r = Rect::from_center_size(rect.center(), Vec2::splat(rect.height().min(16.0)));
    egui::Image::new(super::icons::chevron_down())
        .tint(INK_MUTED)
        .paint_at(ui, r);
}

/// Determinate progress: a framed bar filled with the spot ink.
pub fn progress(ui: &mut egui::Ui, value: f32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, 3, BG_400);
    let mut fill = rect;
    fill.set_width(fill.width() * value.clamp(0.0, 1.0));
    ui.painter().rect_filled(fill, 3, ACCENT_LINE);
}

/// Spinner for work of unknown length: a black arc around a pink dot.
pub fn spinner(ui: &mut egui::Ui, size: f32, reduce_motion: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    let r = size / 2.0 - 2.0;
    painter.circle_stroke(rect.center(), r, Stroke::new(2.0, BG_400));
    let time = ui.input(|i| i.time) as f32;
    let start = if reduce_motion { 0.0 } else { time * 5.0 };
    let points: Vec<_> = (0..=16)
        .map(|i| {
            let a = start + i as f32 / 16.0 * 1.6;
            rect.center() + vec2(a.cos(), a.sin()) * r
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(2.0, ACCENT_LINE)));
    painter.circle_filled(rect.center(), 2.5, ACCENT_LINE);
    if !reduce_motion {
        ui.ctx().request_repaint();
    }
}

/// Small label on a tinted ground.
pub fn badge(ui: &mut egui::Ui, text: &str, accent: bool) -> Response {
    let color = if accent { ACCENT_TEXT } else { INK_MUTED };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), condensed(14.0), color);
    let (rect, response) = ui.allocate_exact_size(galley.size() + vec2(10.0, 2.0), Sense::hover());
    ui.painter()
        .rect_filled(rect, RADIUS_SM, if accent { ACCENT_SOFT } else { BG_300 });
    ui.painter()
        .galley(rect.min + vec2(5.0, 1.0), galley, color);
    response
}

/// Dark checkerboard: the inside of a cut, so light skin art stays readable.
pub fn checkerboard(ui: &egui::Ui, rect: Rect, _radius: u8) {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, 0, CUT);
    let cell = 8.0;
    let cols = (rect.width() / cell).ceil() as i32;
    let rows = (rect.height() / cell).ceil() as i32;
    if cols * rows <= 8000 {
        for y in 0..rows {
            for x in 0..cols {
                if (x + y) % 2 == 0 {
                    let r = Rect::from_min_size(
                        rect.min + vec2(x as f32 * cell, y as f32 * cell),
                        Vec2::splat(cell),
                    )
                    .intersect(rect);
                    painter.rect_filled(r, 0, CUT_CHECK);
                }
            }
        }
    }
    ui.painter()
        .rect_stroke(rect, 0, Stroke::new(1.0, LINE_100), StrokeKind::Inside);
}

/// Draws a texture fitted inside `rect`, keeping its aspect ratio.
pub fn paint_fit(ui: &egui::Ui, texture: &egui::TextureHandle, rect: Rect, tint: Color32) -> Rect {
    let size = texture.size_vec2();
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    let fitted = Rect::from_center_size(rect.center(), size * scale);
    ui.painter().image(
        texture.id(),
        fitted,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        tint,
    );
    fitted
}

/// Draws a texture at `center` scaled so its longest side is `size`.
pub fn paint_centered(
    ui: &egui::Ui,
    texture: &egui::TextureHandle,
    center: egui::Pos2,
    size: f32,
    tint: Color32,
) {
    let dims = texture.size_vec2();
    let scale = size / dims.x.max(dims.y);
    ui.painter().image(
        texture.id(),
        Rect::from_center_size(center, dims * scale),
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        tint,
    );
}

pub fn text_at(
    ui: &egui::Ui,
    pos: egui::Pos2,
    align: Align2,
    text: &str,
    font: FontId,
    color: Color32,
) -> Rect {
    ui.painter().text(pos, align, text, font, color)
}

/// Single-line text that ends with "…" when it does not fit.
pub fn truncated(
    ui: &egui::Ui,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping {
        max_width: width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    ui.painter().layout_job(job)
}

/// kohu!'s hit circle for empty states (see `motion::orb`).
pub fn hit_mark(ui: &mut egui::Ui, size: f32) {
    super::motion::orb(ui, size * 2.2);
}

/// A key cap, e.g. `Ctrl` `Enter` (kohu!'s Kbd).
pub fn kbd(ui: &mut egui::Ui, key: &str, on_plate: bool) -> Response {
    let color = if on_plate { PLUM_MUTED } else { INK_MUTED };
    let galley = ui
        .painter()
        .layout_no_wrap(key.to_owned(), font_strong(11.0), color);
    let (rect, response) =
        ui.allocate_exact_size(vec2(galley.size().x + 12.0, 18.0), Sense::hover());
    if on_plate {
        ui.painter().rect_filled(rect, 3, PLUM_RAISED);
    } else {
        ui.painter()
            .rect(rect, 3, BG_200, Stroke::new(1.0, LINE_100), StrokeKind::Inside);
    }
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    response
}

/// What the specimen's rings say (kohu!'s StatusSpecimen states, read without text).
#[derive(Clone, Copy, PartialEq)]
pub enum Rings {
    /// Rings converge on a full core: ready for the moment.
    Ready,
    /// Rings closed in on the core, which breathes: done / working.
    Running,
    /// Dashed outer ring turning: busy.
    Busy,
    /// Dim rings around an empty core: nothing yet.
    Idle,
    /// Dim rings, dashed core: something is wrong.
    Error,
}

fn dashed_circle(center: egui::Pos2, r: f32, dash: f32, gap: f32, phase: f32, stroke: Stroke) -> Vec<egui::Shape> {
    let circumference = std::f32::consts::TAU * r;
    let n = (circumference / (dash + gap)).floor().max(4.0) as usize;
    let step = std::f32::consts::TAU / n as f32;
    let dash_angle = step * dash / (dash + gap);
    (0..n)
        .map(|i| {
            let a0 = phase + i as f32 * step;
            let pts: Vec<_> = (0..=6)
                .map(|k| {
                    let a = a0 + dash_angle * k as f32 / 6.0;
                    center + vec2(a.cos(), a.sin()) * r
                })
                .collect();
            egui::Shape::line(pts, stroke)
        })
        .collect()
}

/// Paints specimen rings (300-unit artboard, like the kohu! SVG) of diameter `d`.
/// `colors` replaces the pink of the converging rings and the core (a skin's combo colours).
pub fn rings(ui: &egui::Ui, painter: &egui::Painter, center: egui::Pos2, d: f32, state: Rings, colors: &[Color32], live: bool) -> Vec<egui::Shape> {
    let k = d / 300.0;
    let w = (4.0 * k).max(1.5);
    let ctx = ui.ctx();
    let still = super::motion::reduced(ctx);
    let time = ctx.input(|i| i.time) as f32;
    let pick = |i: usize| colors.get(i % colors.len().max(1)).copied().unwrap_or(ACCENT);
    let mut shapes = vec![];
    let soft = Stroke::new(w, PLUM_TEXT.gamma_multiply(0.55));
    let dim = Stroke::new(w, PLUM_MUTED.gamma_multiply(0.8));
    let mut animate = false;
    match state {
        Rings::Busy => {
            let phase = if still { 0.0 } else { time * std::f32::consts::TAU / 6.0 };
            shapes.extend(dashed_circle(center, 140.0 * k, 40.0 * k, 30.0 * k, phase, soft));
            animate = !still;
        }
        _ => shapes.push(egui::Shape::circle_stroke(center, 140.0 * k, soft)),
    }
    match state {
        Rings::Ready => {
            shapes.push(egui::Shape::circle_stroke(center, 104.0 * k, Stroke::new(w, pick(0))));
            shapes.push(egui::Shape::circle_stroke(center, 70.0 * k, Stroke::new(w, pick(1))));
            shapes.push(egui::Shape::circle_filled(center, 36.0 * k, pick(2)));
            // kh-approach: a ring closes in from outside and fades on the core.
            if live && !still {
                let u = (time / 2.4).fract();
                let u = super::motion::ease(u);
                let scale = 1.35 - 0.93 * u;
                let alpha = if u < 0.3 { u / 0.3 * 0.9 } else { 0.9 * (1.0 - (u - 0.3) / 0.7) };
                shapes.push(egui::Shape::circle_stroke(
                    center,
                    104.0 * k * scale,
                    Stroke::new(w, pick(0).gamma_multiply(alpha)),
                ));
                animate = true;
            }
        }
        Rings::Running => {
            shapes.push(egui::Shape::circle_stroke(center, 58.0 * k, Stroke::new(w, pick(0))));
            shapes.push(egui::Shape::circle_stroke(center, 48.0 * k, Stroke::new(w, pick(1))));
            let pulse = if still || !live {
                1.0
            } else {
                animate = true;
                let u = (time / 1.6) % 2.0;
                let u = if u > 1.0 { 2.0 - u } else { u };
                1.0 + 0.12 * super::motion::ease(u)
            };
            shapes.push(egui::Shape::circle_filled(center, 36.0 * k * pulse, pick(2)));
        }
        Rings::Busy | Rings::Idle | Rings::Error => {
            shapes.push(egui::Shape::circle_stroke(center, 104.0 * k, dim));
            shapes.push(egui::Shape::circle_stroke(center, 70.0 * k, dim));
            if state == Rings::Error {
                shapes.extend(dashed_circle(center, 36.0 * k, 6.0 * k, 8.0 * k, 0.0, dim));
            } else {
                shapes.push(egui::Shape::circle_stroke(center, 36.0 * k, dim));
            }
        }
    }
    let _ = painter;
    if animate {
        ctx.request_repaint();
    }
    shapes
}

/// kohu!'s StatusSpecimen: a plum plate that carries the screen's moment, with rings on the
/// right that tell its state. Content is laid out on the left in plate colours.
pub fn plate<R>(
    ui: &mut egui::Ui,
    state: Rings,
    colors: &[Color32],
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let pad = 18.0;
    let full = ui.available_rect_before_wrap();
    let bg = ui.painter().add(egui::Shape::Noop);
    let show_rings = full.width() >= 560.0;
    let rings_space = if show_rings { (full.width() * 0.36).clamp(110.0, 200.0) } else { pad };
    let inner_rect = Rect::from_min_max(
        full.min + vec2(pad, pad),
        pos2(full.right() - rings_space, full.bottom() - pad),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    {
        let v = child.visuals_mut();
        v.override_text_color = Some(PLUM_TEXT);
        v.weak_text_color = Some(PLUM_MUTED);
        v.text_edit_bg_color = Some(PLUM_RAISED);
        v.extreme_bg_color = PLUM_RAISED;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, PLUM_LINE);
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, PLUM_MUTED);
        v.widgets.active.bg_stroke = Stroke::new(1.0, PLUM_MUTED);
        v.selection.bg_fill = PLUM_RAISED.lerp_to_gamma(ACCENT_LINE, 0.35);
    }
    let inner = add_contents(&mut child);
    let used = child.min_rect();
    let rect = Rect::from_min_max(full.min, pos2(full.right(), used.bottom() + pad));
    let response = ui.allocate_rect(rect, Sense::hover());
    let live = response.contains_pointer();
    let mut shapes = vec![egui::Shape::rect_filled(rect, RADIUS_LG, PLUM)];
    if show_rings {
        let d = (rect.height() * 1.3).clamp(110.0, rings_space * 1.05);
        let center = pos2(rect.right() - d * 0.44, rect.center().y);
        shapes.extend(rings(
            ui,
            ui.painter(),
            center,
            d,
            state,
            colors,
            live || matches!(state, Rings::Busy | Rings::Running),
        ));
    }
    ui.painter().with_clip_rect(rect.intersect(ui.clip_rect())).set(bg, egui::Shape::Vec(shapes));
    egui::InnerResponse { inner, response }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_controls_publish_accessible_names_and_states() {
        let ctx = egui::Context::default();
        install(&ctx, true, 1.0);
        ctx.enable_accesskit();
        let output = ctx.run_ui(Default::default(), |ui| {
            btn(Kind::Primary, "Create skin").show(ui);
            toggle(ui, &mut true, "Reduce motion");
            check(ui, true, "Select skin");
            tabs(ui, &mut 0, &[(0, "Pick"), (1, "Compare")]);
            ui.add_enabled_ui(false, |ui| {
                btn(Kind::Primary, "Apply changes").show(ui);
            });
        });
        let update = output.platform_output.accesskit_update.as_ref().unwrap();
        for (label, role) in [
            ("Create skin", egui::Role::Button),
            ("Reduce motion", egui::Role::Switch),
            ("Select skin", egui::Role::CheckBox),
            ("Pick", egui::Role::Tab),
        ] {
            let (_, node) = update.nodes.iter().find(|(_, node)| node.label() == Some(label)).unwrap();
            assert_eq!(node.role(), role, "{label}");
        }
        let (_, disabled) = update.nodes.iter().find(|(_, node)| node.label() == Some("Apply changes")).unwrap();
        assert!(disabled.is_disabled());
        output.drop_without_applying_deltas();
    }

    #[test]
    fn compact_plates_keep_actions_inside_the_content_area() {
        let ctx = egui::Context::default();
        install(&ctx, true, 1.0);
        for width in [280.0, 340.0, 480.0] {
            ctx.run_ui(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 700.0))),
                ..Default::default()
            }, |ui| {
                plate(ui, Rings::Ready, &[], |ui| {
                    let bounds = ui.max_rect();
                    let action = btn(Kind::OnPlate, "Open in editor").min_width(180.0).show(ui);
                    assert!(bounds.contains_rect(action.rect), "Action clipped at {width} px");
                });
            }).drop_without_applying_deltas();
        }
    }

    #[test]
    fn reused_text_tokens_keep_readable_contrast() {
        let luminance = |color: Color32| {
            let channel = |value: u8| {
                let v = f64::from(value) / 255.0;
                if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            };
            0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
        };
        for (text, background) in [
            (INK_SUBTLE, BG_300), (INK_MUTED, BG_400),
            (PLUM_TEXT, PLUM), (PLUM_MUTED, PLUM),
            (ON_ACCENT, ACCENT), (ON_INK, INK),
        ] {
            let a = luminance(text);
            let b = luminance(background);
            assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
        }
    }
}
