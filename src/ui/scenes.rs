//! Clickable scenes of the stage view, drawn from the elements the mix uses right now.
//!
//! Every scene is laid out in osu!'s 16:9 skin space (1366×768, the size SD elements
//! are drawn for) and scaled to the canvas. Each drawn element leaves a hotspot; a click
//! focuses its catalog node so the preview panel can offer variants of it.

use super::{
    editor::{Origin, Pick, audio_stem, node_at},
    icons,
    stages::{HIT_SETS, MOD_ROWS, SceneKind, related},
    textures::State,
    theme::*,
    *,
};
use egui::{Align2, Pos2, Rect};
use skinweaver::{catalog::Target, ini::SkinIni, mixer::Selection};
use std::f32::consts::{FRAC_PI_2, PI};

pub(crate) const W: f32 = 1366.0;
pub(crate) const H: f32 = 768.0;
/// Hit object sprites are drawn so 128 SD pixels match the circle diameter at CS 4.
const CIRCLE: f32 = 0.9;

pub(crate) struct Spot {
    pub rect: Rect,
    pub id: String,
    /// A speaker badge: a click plays the sound as well as focusing it.
    pub sound: bool,
}

pub(crate) struct Canvas {
    pub rect: Rect,
    pub s: f32,
    pub spots: Vec<Spot>,
    selections: std::rc::Rc<Vec<Selection>>,
    ini: SkinIni,
    time: f32,
}

impl Canvas {
    fn p(&self, (x, y): (f32, f32)) -> Pos2 {
        self.rect.min + vec2(x, y) * self.s
    }
    fn back(&self, p: Pos2) -> (f32, f32) {
        let v = (p - self.rect.min) / self.s;
        (v.x, v.y)
    }
    fn painter(&self, ui: &egui::Ui) -> egui::Painter {
        ui.painter()
            .with_clip_rect(self.rect.intersect(ui.clip_rect()))
    }
    fn color(&self, key: &str, fallback: [u8; 3]) -> Color32 {
        rgb(self
            .ini
            .get("Colours", key)
            .and_then(super::editor::parse_color)
            .unwrap_or(fallback))
    }
    fn combo(&self, i: usize) -> Color32 {
        let colors = self.ini.colors();
        rgb(colors[i % colors.len()])
    }
}

/// One element placed on a scene.
struct Put<'a> {
    key: &'a str,
    stem: &'a str,
    at: (f32, f32),
    anchor: Align2,
    scale: f32,
    tint: Color32,
    /// Size used for the hotspot and placeholder when there is no file at all.
    nominal: (f32, f32),
    crop: f32,
    angle: f32,
    spot: bool,
    optional: bool,
}

fn put<'a>(key: &'a str, stem: &'a str) -> Put<'a> {
    Put {
        key,
        stem,
        at: (0.0, 0.0),
        anchor: Align2::CENTER_CENTER,
        scale: 1.0,
        tint: Color32::WHITE,
        nominal: (64.0, 64.0),
        crop: 1.0,
        angle: 0.0,
        spot: true,
        optional: false,
    }
}

impl Put<'_> {
    fn at(mut self, (x, y): (f32, f32)) -> Self {
        self.at = (x, y);
        self
    }
    fn anchor(mut self, anchor: Align2) -> Self {
        self.anchor = anchor;
        self
    }
    fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
    fn tint(mut self, tint: Color32) -> Self {
        self.tint = tint;
        self
    }
    fn nominal(mut self, w: f32, h: f32) -> Self {
        self.nominal = (w, h);
        self
    }
    fn crop(mut self, crop: f32) -> Self {
        self.crop = crop;
        self
    }
    fn angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }
    /// Drawn without its own hotspot (another element already covers it).
    fn quiet(mut self) -> Self {
        self.spot = false;
        self
    }
    /// No placeholder when no file exists anywhere.
    fn optional(mut self) -> Self {
        self.optional = true;
        self
    }
}

fn image(painter: &egui::Painter, texture: egui::TextureId, rect: Rect, angle: f32, tint: Color32) {
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    image_uv(painter, texture, rect, uv, angle, tint);
}

fn image_uv(
    painter: &egui::Painter,
    texture: egui::TextureId,
    rect: Rect,
    uv: Rect,
    angle: f32,
    tint: Color32,
) {
    if angle == 0.0 {
        painter.image(texture, rect, uv, tint);
        return;
    }
    let mut mesh = egui::Mesh::with_texture(texture);
    mesh.add_rect_with_uv(rect, uv, tint);
    mesh.rotate(egui::emath::Rot2::from_angle(angle), rect.center());
    painter.add(mesh);
}

/// Bounding box of `rect` turned around its centre.
fn turned(rect: Rect, angle: f32) -> Rect {
    if angle == 0.0 {
        return rect;
    }
    let (sin, cos) = angle.sin_cos();
    let half = rect.size() / 2.0;
    let extent = vec2(
        (half.x * cos).abs() + (half.y * sin).abs(),
        (half.x * sin).abs() + (half.y * cos).abs(),
    );
    Rect::from_center_size(rect.center(), extent * 2.0)
}

/// Where an element would be if it existed: a quiet dashed frame.
fn placeholder(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, 2, Color32::from_white_alpha(12));
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
        rect.left_top(),
    ];
    painter.extend(egui::Shape::dashed_line(
        &corners,
        Stroke::new(1.0, Color32::from_white_alpha(80)),
        5.0,
        4.0,
    ));
}

fn bezier(a: (f32, f32), c: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    (
        u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
        u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
    )
}

impl App {
    /// Draws a stage scene `width` pixels wide and handles clicks on it.
    pub(crate) fn stage_scene(&mut self, ui: &mut egui::Ui, kind: SceneKind, width: f32) {
        let (selections, ini) = self.scene_inputs();
        let (rect, response) = ui.allocate_exact_size(vec2(width, width * H / W), Sense::click());
        let time = if self.settings.reduce_motion {
            0.35
        } else {
            ui.input(|i| i.time) as f32
        };
        let mut cv = Canvas {
            rect,
            s: width / W,
            spots: vec![],
            selections,
            ini,
            time,
        };
        cv.painter(ui)
            .rect_filled(rect, 0, Color32::from_rgb(16, 15, 20));
        match kind {
            SceneKind::Menu => self.scene_menu(ui, &mut cv),
            SceneKind::SongSelect => self.scene_song_select(ui, &mut cv),
            SceneKind::Mods => self.scene_mods(ui, &mut cv),
            SceneKind::Play => self.scene_play(ui, &mut cv),
            SceneKind::Spinner => self.scene_spinner(ui, &mut cv),
            SceneKind::Break => self.scene_break(ui, &mut cv),
            SceneKind::Pause => self.scene_pause(ui, &mut cv, false),
            SceneKind::Fail => self.scene_pause(ui, &mut cv, true),
            SceneKind::Results => self.scene_results(ui, &mut cv),
        }
        ui.painter().rect_stroke(
            rect,
            0,
            Stroke::new(1.0, LINE_100),
            egui::StrokeKind::Inside,
        );
        if !self.settings.reduce_motion {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
        self.scene_interaction(ui, &cv, &response);
    }

    // ------------------------------------------------------------ primitives

    fn add_spot(&self, cv: &mut Canvas, key: &str, rect: Rect, sound: bool) {
        if let Some(id) = self.stage_index.id(key) {
            // Tiny elements on a small canvas still get a target a finger-width wide.
            const MIN: f32 = 18.0;
            let mut rect = rect.intersect(cv.rect);
            if rect.width() < MIN || rect.height() < MIN {
                rect = Rect::from_center_size(rect.center(), rect.size().max(Vec2::splat(MIN)));
            }
            cv.spots.push(Spot {
                rect,
                id: id.to_owned(),
                sound,
            });
        }
    }

    fn draw(&mut self, ui: &egui::Ui, cv: &mut Canvas, p: Put) -> Rect {
        let origin = self.scene_origin(&cv.selections, p.stem, false);
        let animated = self.stage_index.animated(p.stem);
        let state = self.element(&origin, p.stem, animated, 512);
        let painter = cv.painter(ui);
        let rect = match &state {
            Some(State::Ready(e)) => {
                let size = e.logical * p.scale * cv.s;
                let rect = p.anchor.anchor_size(cv.p(p.at), size);
                let frame = self.animation_frame(ui, e.frames.len());
                if p.crop < 1.0 {
                    let shown =
                        Rect::from_min_size(rect.min, vec2(rect.width() * p.crop, rect.height()));
                    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(p.crop, 1.0));
                    image_uv(&painter, e.frames[frame].id(), shown, uv, p.angle, p.tint);
                } else {
                    image(&painter, e.frames[frame].id(), rect, p.angle, p.tint);
                }
                rect
            }
            other => {
                let size = vec2(p.nominal.0, p.nominal.1) * p.scale * cv.s;
                let rect = p.anchor.anchor_size(cv.p(p.at), size);
                if !p.optional && matches!(other, Some(State::Missing) | None) {
                    placeholder(&painter, turned(rect, p.angle));
                }
                rect
            }
        };
        let rect = turned(rect, p.angle);
        if p.spot {
            self.add_spot(cv, p.key, rect, false);
        }
        rect
    }

    /// Whether the mix takes this element from a skin that really has the file.
    fn has_own(&mut self, cv: &Canvas, stem: &str) -> bool {
        let origin = self.scene_origin(&cv.selections, stem, false);
        matches!(self.element(&origin, stem, false, 512), Some(State::Ready(e)) if !e.fallback)
    }

    /// A full-screen picture scaled to cover the canvas. Returns false when there is none.
    fn cover(&mut self, ui: &egui::Ui, cv: &mut Canvas, key: &str, stem: &str) -> bool {
        let origin = self.scene_origin(&cv.selections, stem, false);
        let state = self.element(&origin, stem, false, 1366);
        self.add_spot(cv, key, cv.rect, false);
        let Some(State::Ready(e)) = state else {
            return false;
        };
        let scale = (cv.rect.width() / e.logical.x).max(cv.rect.height() / e.logical.y);
        let rect = Rect::from_center_size(cv.rect.center(), e.logical * scale);
        image(&cv.painter(ui), e.frames[0].id(), rect, 0.0, Color32::WHITE);
        true
    }

    /// A speaker badge that plays a sound of the stage.
    fn speaker(&self, ui: &egui::Ui, cv: &mut Canvas, key: &str, at: Pos2) {
        let at = pos2(
            at.x.clamp(cv.rect.left() + 14.0, cv.rect.right() - 14.0),
            at.y.clamp(cv.rect.top() + 14.0, cv.rect.bottom() - 14.0),
        );
        // Badges shrink with small canvases so they never hide what they belong to.
        let radius = (26.0 * cv.s).clamp(8.0, 12.0);
        let painter = cv.painter(ui);
        painter.circle_filled(at, radius, Color32::from_rgba_unmultiplied(27, 26, 31, 225));
        painter.circle_stroke(at, radius, Stroke::new(1.0, Color32::from_white_alpha(150)));
        egui::Image::new(icons::audio_lines())
            .tint(INK)
            .paint_at(ui, Rect::from_center_size(at, Vec2::splat(radius * 1.15)));
        self.add_spot(
            cv,
            key,
            Rect::from_center_size(at, Vec2::splat(radius * 2.0 + 2.0)),
            true,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn text(
        &self,
        ui: &egui::Ui,
        cv: &Canvas,
        at: (f32, f32),
        align: Align2,
        text: &str,
        size: f32,
        color: Color32,
    ) -> Rect {
        cv.painter(ui)
            .text(cv.p(at), align, text, font(size * cv.s), color)
    }

    /// Where the digits of a font come from: a skin with its own prefix brings its
    /// font folder; otherwise the digit files themselves decide.
    fn font_source(&self, cv: &Canvas, prefix_key: &str, sample: &str) -> (Origin, String) {
        let default = sample.rsplit_once('-').map_or("score", |(p, _)| p);
        let source = cv
            .selections
            .iter()
            .find(|s| {
                matches!(&s.target, Target::Property { section, key }
                    if section == "Fonts" && key == prefix_key)
            })
            .map(|s| s.source.clone())
            .unwrap_or_else(|| self.base_source());
        let origin = self.origin(&source);
        if let Origin::Folder(Some(path)) = &origin
            && let Some(prefix) = self
                .skin(path)
                .and_then(|s| s.ini.get("Fonts", prefix_key))
                .map(str::trim)
                .filter(|p| !p.is_empty())
        {
            return (origin.clone(), prefix.replace('\\', "/"));
        }
        (
            self.scene_origin(&cv.selections, sample, false),
            default.to_owned(),
        )
    }

    /// A number set in a skin font; returns its bounding box.
    #[allow(clippy::too_many_arguments)]
    fn digits(
        &mut self,
        ui: &egui::Ui,
        cv: &mut Canvas,
        key: &str,
        prefix_key: &str,
        sample: &str,
        text: &str,
        at: (f32, f32),
        anchor: Align2,
        scale: f32,
    ) -> Rect {
        let (origin, prefix) = self.font_source(cv, prefix_key, sample);
        let glyphs: Vec<(char, Option<State>)> = text
            .chars()
            .map(|c| {
                let name = match c {
                    '.' => "dot".to_owned(),
                    '%' => "percent".to_owned(),
                    ',' => "comma".to_owned(),
                    other => other.to_string(),
                };
                (
                    c,
                    self.element(&origin, &format!("{prefix}-{name}"), false, 256),
                )
            })
            .collect();
        let size = |state: &Option<State>| match state {
            Some(State::Ready(e)) => e.logical,
            _ => vec2(26.0, 38.0),
        };
        let total = glyphs.iter().fold(Vec2::ZERO, |acc, (_, state)| {
            let s = size(state);
            vec2(acc.x + s.x, acc.y.max(s.y))
        }) * scale
            * cv.s;
        let rect = anchor.anchor_size(cv.p(at), total);
        let painter = cv.painter(ui);
        let mut x = rect.left();
        for (c, state) in &glyphs {
            let glyph = size(state) * scale * cv.s;
            let r = Rect::from_min_size(pos2(x, rect.bottom() - glyph.y), glyph);
            match state {
                Some(State::Ready(e)) => image(&painter, e.frames[0].id(), r, 0.0, Color32::WHITE),
                Some(State::Loading) => {}
                _ => {
                    painter.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        c.to_string(),
                        condensed(glyph.y * 0.9),
                        Color32::WHITE,
                    );
                }
            }
            x += glyph.x;
        }
        self.add_spot(cv, key, rect, false);
        rect
    }

    /// A hit circle (or a slider head) with its overlay and number.
    fn circle(
        &mut self,
        ui: &egui::Ui,
        cv: &mut Canvas,
        at: (f32, f32),
        color: Color32,
        number: u32,
        head: Option<&str>,
    ) -> Rect {
        const KEY: &str = "grp:Gameplay/osu!/Hitcircles";
        let (stem, key) = match head {
            Some(stem) if self.has_own(cv, stem) => (stem, "grp:Gameplay/osu!/Sliders"),
            _ => ("hitcircle", KEY),
        };
        let rect = self.draw(
            ui,
            cv,
            put(key, stem)
                .at(at)
                .scale(CIRCLE)
                .tint(color)
                .nominal(128.0, 128.0),
        );
        let above = cv
            .ini
            .get("General", "HitCircleOverlayAboveNumber")
            .is_none_or(|v| v.trim() != "0");
        let overlay = |app: &mut Self, cv: &mut Canvas| {
            app.draw(
                ui,
                cv,
                put(KEY, "hitcircleoverlay")
                    .at(at)
                    .scale(CIRCLE)
                    .nominal(128.0, 128.0)
                    .optional()
                    .quiet(),
            );
        };
        if !above {
            overlay(self, cv);
        }
        self.digits(
            ui,
            cv,
            KEY,
            "HitCirclePrefix",
            "default-1",
            &number.to_string(),
            at,
            Align2::CENTER_CENTER,
            CIRCLE * 0.85,
        );
        if above {
            overlay(self, cv);
        }
        rect
    }

    fn corner(rect: Rect) -> Pos2 {
        pos2(rect.right() - 6.0, rect.top() + 6.0)
    }

    // ------------------------------------------------------------ scenes

    fn scene_menu(&mut self, ui: &egui::Ui, cv: &mut Canvas) {
        if !self.cover(ui, cv, "menu-background", "menu-background") {
            cv.painter(ui)
                .rect_filled(cv.rect, 0, Color32::from_rgb(44, 40, 58));
        }
        let flakes = [
            (120.0, 80.0),
            (300.0, 420.0),
            (470.0, 160.0),
            (860.0, 90.0),
            (1010.0, 300.0),
            (1220.0, 520.0),
            (1300.0, 140.0),
            (640.0, 660.0),
        ];
        for (i, (x, y)) in flakes.into_iter().enumerate() {
            let fall = (y + cv.time * (14.0 + 6.0 * (i % 3) as f32)) % H;
            let flake = put("menu-snow", "menu-snow")
                .at((x, fall))
                .scale(0.35 + 0.1 * (i % 3) as f32)
                .tint(Color32::from_white_alpha(210))
                .nominal(60.0, 60.0);
            // One placeholder is enough to show where snow would fall.
            self.draw(ui, cv, if i == 0 { flake } else { flake.optional() });
        }
        let glow = cv.color("MenuGlow", [0, 78, 155]);
        let painter = cv.painter(ui);
        let center = cv.p((440.0, 384.0));
        let radius = 210.0 * cv.s;
        painter.circle_filled(center, radius * 1.1, glow.gamma_multiply(0.45));
        painter.circle_filled(center, radius, Color32::from_rgb(232, 98, 160));
        painter.circle_stroke(
            center,
            radius * 0.92,
            Stroke::new(9.0 * cv.s, Color32::WHITE),
        );
        painter.text(
            center,
            Align2::CENTER_CENTER,
            "osu!",
            font_strong(120.0 * cv.s),
            Color32::WHITE,
        );
        self.speaker(
            ui,
            cv,
            "snd:heartbeat",
            center + vec2(radius * 0.7, -radius * 0.7),
        );
        let buttons = [
            ("Играть", "Play", "snd:menu-play-*"),
            ("Редактор", "Edit", "snd:menu-edit-*"),
            ("Настройки", "Options", "snd:menu-options-*"),
            ("Выход", "Exit", "snd:menu-exit-*"),
        ];
        for (i, (ru, en, sound)) in buttons.into_iter().enumerate() {
            let y = 200.0 + i as f32 * 96.0;
            let rect = Rect::from_min_size(cv.p((700.0, y)), vec2(520.0, 76.0) * cv.s);
            let painter = cv.painter(ui);
            painter.rect_filled(rect, 4.0 * cv.s, glow.gamma_multiply(0.55));
            painter.rect_stroke(
                rect,
                4.0 * cv.s,
                Stroke::new((2.0 * cv.s).max(1.0), glow),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.left_center() + vec2(30.0 * cv.s, 0.0),
                Align2::LEFT_CENTER,
                self.t(ru, en),
                font_strong(32.0 * cv.s),
                Color32::WHITE,
            );
            self.add_spot(cv, "ini:Colours/MenuGlow", rect, false);
            self.speaker(ui, cv, sound, rect.right_center() - vec2(24.0, 0.0));
        }
        let welcome = self.draw(
            ui,
            cv,
            put("welcome_text", "welcome_text")
                .at((683.0, 28.0))
                .anchor(Align2::CENTER_TOP)
                .nominal(640.0, 64.0),
        );
        self.speaker(
            ui,
            cv,
            "snd:welcome",
            welcome.right_center() + vec2(20.0, 0.0),
        );
    }

    fn scene_song_select(&mut self, ui: &egui::Ui, cv: &mut Canvas) {
        cv.painter(ui)
            .rect_filled(cv.rect, 0, Color32::from_rgb(38, 34, 48));
        self.draw(
            ui,
            cv,
            put("mode-osu", "mode-osu")
                .at((560.0, 420.0))
                .tint(Color32::from_white_alpha(70))
                .nominal(300.0, 300.0),
        );
        self.leaderboard(ui, cv);
        let active = cv.color("SongSelectActiveText", [0, 0, 0]);
        let inactive = cv.color("SongSelectInactiveText", [255, 255, 255]);
        let cards: [(&str, &str, Option<&str>, u8); 6] = [
            (
                "Camellia — Exit This Earth's Atomosphere",
                "[Evolution]",
                None,
                0,
            ),
            ("xi — Blue Zenith", "[Easy]", Some("ranking-s-small"), 1),
            (
                "xi — Blue Zenith",
                "[FOUR DIMENSIONS]",
                Some("ranking-a-small"),
                2,
            ),
            ("xi — Blue Zenith", "[Insane]", None, 1),
            ("Halozy — Genryuu Kaiko", "[Higan Torrent]", None, 0),
            ("t+pazolite — Oshama Scramble!", "[Extreme]", None, 0),
        ];
        for (i, (title, version, rank, kind)) in cards.into_iter().enumerate() {
            let y = 150.0 + i as f32 * 92.0;
            let x = [710.0, 660.0, 610.0][kind as usize];
            let tint = match kind {
                2 => Color32::WHITE,
                1 => Color32::from_rgb(0, 150, 236),
                _ => Color32::from_rgb(235, 73, 153),
            };
            let card = self.draw(
                ui,
                cv,
                put("menu-button-background", "menu-button-background")
                    .at((x, y))
                    .anchor(Align2::LEFT_CENTER)
                    .tint(tint)
                    .nominal(699.0, 103.0),
            );
            let selected = kind == 2;
            let (color, key) = if selected {
                (active, "ini:Colours/SongSelectActiveText")
            } else {
                (inactive, "ini:Colours/SongSelectInactiveText")
            };
            let left = cv.back(card.left_center()).0;
            let text_x = left + 80.0;
            let mut text = self.text(
                ui,
                cv,
                (text_x, y - 30.0),
                Align2::LEFT_TOP,
                title,
                21.0,
                color,
            );
            text = text.union(self.text(
                ui,
                cv,
                (text_x, y - 4.0),
                Align2::LEFT_TOP,
                version,
                17.0,
                color,
            ));
            self.add_spot(cv, key, text, false);
            if let Some(rank) = rank {
                self.draw(
                    ui,
                    cv,
                    put("grp:Interface/Song select/Ranking icons", rank)
                        .at((left + 42.0, y))
                        .nominal(34.0, 40.0),
                );
            }
            if selected {
                for k in 0..5 {
                    self.draw(
                        ui,
                        cv,
                        put("star", "star")
                            .at((text_x + 12.0 + k as f32 * 28.0, y + 32.0))
                            .scale(0.45)
                            .nominal(56.0, 56.0),
                    );
                }
                self.speaker(
                    ui,
                    cv,
                    "snd:select-difficulty",
                    pos2(card.right().min(cv.rect.right()) - 26.0, card.center().y),
                );
            }
        }
        self.draw(
            ui,
            cv,
            put("songselect-top", "songselect-top")
                .anchor(Align2::LEFT_TOP)
                .nominal(1366.0, 92.0),
        );
        let tabs = [
            ("Без группы", "No grouping"),
            ("Недавние", "Recently played"),
            ("Коллекции", "Collections"),
        ];
        for (k, (ru, en)) in tabs.into_iter().enumerate() {
            let tab = self.draw(
                ui,
                cv,
                put("selection-tab", "selection-tab")
                    .at((790.0 + k as f32 * 170.0, 96.0))
                    .anchor(Align2::LEFT_BOTTOM)
                    .tint(if k == 0 {
                        Color32::WHITE
                    } else {
                        Color32::from_gray(170)
                    })
                    .nominal(160.0, 30.0),
            );
            cv.painter(ui).text(
                tab.center(),
                Align2::CENTER_CENTER,
                self.t(ru, en),
                font(14.0 * cv.s),
                Color32::WHITE,
            );
        }
        self.text(
            ui,
            cv,
            (18.0, 10.0),
            Align2::LEFT_TOP,
            "xi — Blue Zenith [FOUR DIMENSIONS]",
            24.0,
            Color32::WHITE,
        );
        self.text(
            ui,
            cv,
            (18.0, 42.0),
            Align2::LEFT_TOP,
            self.t(
                "Длина 3:58  BPM 200  Объекты 1544",
                "Length 3:58  BPM 200  Objects 1544",
            ),
            15.0,
            Color32::from_gray(220),
        );
        self.draw(
            ui,
            cv,
            put("songselect-bottom", "songselect-bottom")
                .at((0.0, H))
                .anchor(Align2::LEFT_BOTTOM)
                .nominal(1366.0, 96.0),
        );
        let group = "grp:Interface/Song select/Bottom bar buttons";
        for (stem, x) in [
            ("selection-mode", 210.0),
            ("selection-mods", 296.0),
            ("selection-random", 382.0),
            ("selection-options", 468.0),
        ] {
            let button = self.draw(
                ui,
                cv,
                put(group, stem)
                    .at((x, H))
                    .anchor(Align2::LEFT_BOTTOM)
                    .nominal(78.0, 88.0),
            );
            if stem == "selection-mode" {
                self.draw(
                    ui,
                    cv,
                    put("mode-osu-small", "mode-osu-small")
                        .at(cv.back(button.center()))
                        .nominal(32.0, 32.0),
                );
            }
        }
        for (k, stem) in [
            "selection-mod-hidden",
            "selection-mod-hardrock",
            "selection-mod-doubletime",
        ]
        .into_iter()
        .enumerate()
        {
            self.draw(
                ui,
                cv,
                put("grp:Interface/Mod icons", stem)
                    .at((330.0 + k as f32 * 52.0, 630.0))
                    .scale(0.8)
                    .nominal(68.0, 66.0),
            );
        }
        let back = self.draw(
            ui,
            cv,
            put("menu-back", "menu-back")
                .at((0.0, H))
                .anchor(Align2::LEFT_BOTTOM)
                .nominal(190.0, 86.0),
        );
        self.speaker(ui, cv, "snd:back-button-click", Self::corner(back));
    }

    /// The mod select panel: every mod icon, each one picked on its own.
    fn scene_mods(&mut self, ui: &egui::Ui, cv: &mut Canvas) {
        cv.painter(ui)
            .rect_filled(cv.rect, 0, Color32::from_rgb(30, 28, 38));
        let title = self.text(
            ui,
            cv,
            (60.0, 34.0),
            Align2::LEFT_TOP,
            self.t("Выбор модов", "Mods"),
            40.0,
            Color32::WHITE,
        );
        self.speaker(
            ui,
            cv,
            "snd:check-on",
            title.right_center() + vec2(26.0, 0.0),
        );
        self.speaker(
            ui,
            cv,
            "snd:check-off",
            title.right_center() + vec2(58.0, 0.0),
        );
        for (row, (ru, en, mods)) in MOD_ROWS.iter().enumerate() {
            let y = 136.0 + row as f32 * 158.0;
            self.text(
                ui,
                cv,
                (60.0, y),
                Align2::LEFT_TOP,
                self.t(ru, en),
                20.0,
                Color32::from_gray(200),
            );
            for (k, (name, short)) in mods.iter().enumerate() {
                let stem = format!("selection-mod-{name}");
                let x = 110.0 + k as f32 * 106.0;
                let icon = self.draw(
                    ui,
                    cv,
                    put(&stem, &stem)
                        .at((x, y + 78.0))
                        .scale(1.35)
                        .nominal(68.0, 66.0),
                );
                cv.painter(ui).text(
                    pos2(icon.center().x, icon.bottom() + 4.0 * cv.s),
                    Align2::CENTER_TOP,
                    *short,
                    font_strong(21.0 * cv.s),
                    Color32::from_gray(190),
                );
            }
        }
    }

    /// Local scores at the left of song select: the beatmap plate, rank icons and mod
    /// icons appear here again, with numbers in the score-entry font.
    fn leaderboard(&mut self, ui: &egui::Ui, cv: &mut Canvas) {
        let entries = [
            (
                "Kirin",
                "ranking-x-small",
                "1,024,377",
                Some("selection-mod-hidden"),
            ),
            ("Hoshimi", "ranking-s-small", "987,120", None),
            (
                "Mochi",
                "ranking-a-small",
                "912,004",
                Some("selection-mod-hardrock"),
            ),
            (
                "Yuzu",
                "ranking-b-small",
                "640,512",
                Some("selection-mod-doubletime"),
            ),
        ];
        for (i, (name, rank, score, mod_icon)) in entries.into_iter().enumerate() {
            let y = 168.0 + i as f32 * 74.0;
            self.draw(
                ui,
                cv,
                put("menu-button-background", "menu-button-background")
                    .at((450.0, y))
                    .anchor(Align2::RIGHT_CENTER)
                    .scale(0.68)
                    .tint(Color32::from_rgba_unmultiplied(70, 70, 92, 225))
                    .nominal(699.0, 103.0),
            );
            self.draw(
                ui,
                cv,
                put("grp:Interface/Song select/Ranking icons", rank)
                    .at((36.0, y))
                    .nominal(34.0, 40.0),
            );
            self.text(
                ui,
                cv,
                (66.0, y - 28.0),
                Align2::LEFT_TOP,
                name,
                17.0,
                Color32::WHITE,
            );
            self.digits(
                ui,
                cv,
                "scoreentry-*",
                "",
                "scoreentry-0",
                score,
                (66.0, y + 22.0),
                Align2::LEFT_BOTTOM,
                0.8,
            );
            if let Some(stem) = mod_icon {
                self.draw(
                    ui,
                    cv,
                    put("grp:Interface/Mod icons", stem)
                        .at((410.0, y))
                        .scale(0.5)
                        .nominal(68.0, 66.0),
                );
            }
        }
    }

    /// The in-game scoreboard at the left of the playfield, also in the score-entry font.
    fn ingame_board(&mut self, ui: &egui::Ui, cv: &mut Canvas) {
        let rows = [
            ("Kirin", "1,203,881", false),
            (self.t("Ты", "You"), "727,411", true),
            ("Hoshimi", "650,002", false),
        ];
        for (i, (name, score, me)) in rows.into_iter().enumerate() {
            let y = 300.0 + i as f32 * 58.0;
            let rect = Rect::from_min_size(cv.p((0.0, y)), vec2(230.0, 52.0) * cv.s);
            cv.painter(ui).rect_filled(
                rect,
                0,
                if me {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 60)
                } else {
                    Color32::from_black_alpha(130)
                },
            );
            self.text(
                ui,
                cv,
                (10.0, y + 4.0),
                Align2::LEFT_TOP,
                name,
                15.0,
                Color32::WHITE,
            );
            self.digits(
                ui,
                cv,
                "scoreentry-*",
                "",
                "scoreentry-0",
                score,
                (10.0, y + 48.0),
                Align2::LEFT_BOTTOM,
                0.6,
            );
        }
    }

    fn scene_play(&mut self, ui: &egui::Ui, cv: &mut Canvas) {
        let set = HIT_SETS[self.hit_set.min(HIT_SETS.len() - 1)];
        cv.painter(ui)
            .rect_filled(cv.rect, 0, Color32::from_rgb(14, 13, 18));
        let c1: (f32, f32) = (540.0, 590.0);
        let c2: (f32, f32) = (690.0, 350.0);
        let head: (f32, f32) = (880.0, 560.0);
        let control = (1010.0, 300.0);
        let tail = (1170.0, 440.0);
        // Follow points lead from one object to the next.
        for (a, b) in [(c1, c2), (c2, head)] {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let length = (dx * dx + dy * dy).sqrt();
            let angle = dy.atan2(dx);
            let mut d = 80.0;
            while d < length - 80.0 {
                let f = d / length;
                self.draw(
                    ui,
                    cv,
                    put("followpoint", "followpoint")
                        .at((a.0 + dx * f, a.1 + dy * f))
                        .angle(angle)
                        .nominal(24.0, 8.0)
                        .optional(),
                );
                d += 40.0;
            }
        }
        // Slider body: border, then track.
        let sliders = "grp:Gameplay/osu!/Sliders";
        let points: Vec<Pos2> = (0..=36)
            .map(|j| cv.p(bezier(head, control, tail, j as f32 / 36.0)))
            .collect();
        let diameter = 128.0 * CIRCLE * cv.s;
        let border = cv.color("SliderBorder", [255, 255, 255]);
        let track = cv
            .ini
            .get("Colours", "SliderTrackOverride")
            .and_then(super::editor::parse_color)
            .map(rgb)
            .unwrap_or_else(|| cv.combo(2).gamma_multiply(0.35))
            .to_opaque();
        let painter = cv.painter(ui);
        for (width, color) in [(0.92, border), (0.8, track)] {
            painter.add(egui::Shape::line(
                points.clone(),
                Stroke::new(diameter * width, color),
            ));
            for end in [points[0], points[points.len() - 1]] {
                painter.circle_filled(end, diameter * width / 2.0, color);
            }
        }
        let body = Rect::from_points(&points).expand(diameter * 0.46);
        self.add_spot(cv, sliders, body, false);
        for t in [0.3, 0.45] {
            self.draw(
                ui,
                cv,
                put(sliders, "sliderscorepoint")
                    .at(bezier(head, control, tail, t))
                    .scale(CIRCLE)
                    .nominal(16.0, 16.0)
                    .optional(),
            );
        }
        let end_stem = if self.has_own(cv, "sliderendcircle") {
            "sliderendcircle"
        } else {
            "hitcircle"
        };
        self.draw(
            ui,
            cv,
            put(sliders, end_stem)
                .at(tail)
                .scale(CIRCLE)
                .tint(cv.combo(2))
                .nominal(128.0, 128.0)
                .quiet(),
        );
        let back = bezier(head, control, tail, 0.9);
        self.draw(
            ui,
            cv,
            put(sliders, "reversearrow")
                .at(tail)
                .scale(CIRCLE)
                .angle((back.1 - tail.1).atan2(back.0 - tail.0))
                .nominal(64.0, 64.0)
                .optional(),
        );
        let ball = bezier(head, control, tail, 0.66);
        self.draw(
            ui,
            cv,
            put(sliders, "sliderfollowcircle")
                .at(ball)
                .scale(CIRCLE)
                .nominal(256.0, 256.0)
                .optional()
                .quiet(),
        );
        self.draw(
            ui,
            cv,
            put(sliders, "sliderb")
                .at(ball)
                .scale(CIRCLE)
                .nominal(128.0, 128.0)
                .optional(),
        );
        self.circle(ui, cv, head, cv.combo(2), 3, Some("sliderstartcircle"));
        let second = self.circle(ui, cv, c2, cv.combo(1), 2, None);
        let first = self.circle(ui, cv, c1, cv.combo(0), 1, None);
        let t = (cv.time * 0.55).fract();
        self.draw(
            ui,
            cv,
            put("approachcircle", "approachcircle")
                .at(c1)
                .scale(CIRCLE * (1.0 + 2.2 * (1.0 - t)))
                .tint(cv.combo(0).gamma_multiply(t.max(0.3)))
                .nominal(128.0, 128.0)
                .optional()
                .quiet(),
        );
        // Judgements of the notes already played.
        let judgements = "grp:Gameplay/osu!/Hit judgements";
        self.draw(
            ui,
            cv,
            put(judgements, "hit300")
                .at((380.0, 270.0))
                .scale(0.8)
                .nominal(128.0, 80.0),
        );
        self.draw(
            ui,
            cv,
            put(judgements, "hit100")
                .at((470.0, 170.0))
                .scale(0.8)
                .nominal(128.0, 80.0),
        );
        let miss = self.draw(
            ui,
            cv,
            put(judgements, "hit0")
                .at((330.0, 470.0))
                .scale(0.8)
                .nominal(128.0, 80.0),
        );
        self.ingame_board(ui, cv);
        // Cursor with its trail.
        let cursor = (800.0, 470.0);
        let from = (620.0, 560.0);
        for j in (1..=8).rev() {
            let f = j as f32 / 9.0;
            self.draw(
                ui,
                cv,
                put("grp:Cursor/Trail", "cursortrail")
                    .at((
                        cursor.0 + (from.0 - cursor.0) * f,
                        cursor.1 + (from.1 - cursor.1) * f,
                    ))
                    .tint(Color32::from_white_alpha(((1.0 - f) * 230.0) as u8))
                    .nominal(32.0, 32.0)
                    .optional(),
            );
        }
        self.draw(
            ui,
            cv,
            put("grp:Cursor/Head", "cursor")
                .at(cursor)
                .nominal(64.0, 64.0),
        );
        // HUD.
        let health = "grp:Interface/In-game/Health bar";
        self.draw(
            ui,
            cv,
            put(health, "scorebar-bg")
                .anchor(Align2::LEFT_TOP)
                .nominal(690.0, 44.0),
        );
        let fill = self.draw(
            ui,
            cv,
            put(health, "scorebar-colour")
                .at((8.0, 13.0))
                .anchor(Align2::LEFT_TOP)
                .crop(0.72)
                .nominal(640.0, 20.0),
        );
        let marker = pos2(fill.left() + fill.width() * 0.72, fill.center().y);
        self.draw(
            ui,
            cv,
            put(health, "scorebar-ki")
                .at(cv.back(marker))
                .nominal(60.0, 60.0)
                .optional(),
        );
        let fonts = "grp:Interface/In-game/Fonts";
        let score = self.digits(
            ui,
            cv,
            fonts,
            "ScorePrefix",
            "score-0",
            "00727411",
            (1356.0, 4.0),
            Align2::RIGHT_TOP,
            0.8,
        );
        let below = cv.back(score.right_bottom());
        self.digits(
            ui,
            cv,
            fonts,
            "ScorePrefix",
            "score-0",
            "98.76%",
            (1356.0, below.1 + 4.0),
            Align2::RIGHT_TOP,
            0.5,
        );
        self.digits(
            ui,
            cv,
            fonts,
            "ComboPrefix",
            "score-0",
            "247x",
            (10.0, 758.0),
            Align2::LEFT_BOTTOM,
            1.0,
        );
        let keys = "grp:Interface/In-game/Key overlay";
        self.draw(
            ui,
            cv,
            put(keys, "inputoverlay-background")
                .at((1342.0, 384.0))
                .angle(FRAC_PI_2)
                .nominal(193.0, 46.0),
        );
        let ink = cv.color("InputOverlayText", [0, 0, 0]);
        for (j, count) in ["412", "398", "0", "0"].into_iter().enumerate() {
            let pressed = j == (cv.time * 1.5) as usize % 2;
            let key = self.draw(
                ui,
                cv,
                put(keys, "inputoverlay-key")
                    .at((1342.0, 316.0 + j as f32 * 46.0))
                    .scale(if pressed { 0.88 } else { 1.0 })
                    .tint(if pressed {
                        Color32::from_rgb(255, 222, 0)
                    } else {
                        Color32::WHITE
                    })
                    .nominal(46.0, 46.0),
            );
            cv.painter(ui).text(
                key.center(),
                Align2::CENTER_CENTER,
                count,
                font_strong(13.0 * cv.s),
                ink,
            );
        }
        self.draw(
            ui,
            cv,
            put("play-skip", "play-skip")
                .at((W, H))
                .anchor(Align2::RIGHT_BOTTOM)
                .nominal(200.0, 90.0),
        );
        // Which sounds the notes make in the chosen hitsound set.
        self.speaker(ui, cv, &format!("snd:{set}-hitnormal"), Self::corner(first));
        self.speaker(ui, cv, &format!("snd:{set}-hitclap"), Self::corner(second));
        self.speaker(
            ui,
            cv,
            &format!("snd:{set}-sliderslide"),
            cv.p(bezier(head, control, tail, 0.5)) + vec2(0.0, -diameter * 0.55),
        );
        self.speaker(ui, cv, "snd:combobreak", Self::corner(miss));
        let label = format!("{}: {}", self.t("Набор звуков", "Sound set"), set);
        cv.painter(ui).text(
            cv.p((12.0, 60.0)),
            Align2::LEFT_TOP,
            label,
            font_strong(13.0),
            Color32::from_white_alpha(200),
        );
    }

    fn scene_spinner(&mut self, ui: &egui::Ui, cv: &mut Canvas) {
        cv.painter(ui)
            .rect_filled(cv.rect, 0, Color32::from_rgb(14, 13, 18));
        let center = (683.0, 400.0);
        let spin = cv.time * 2.4;
        let old = self.has_own(cv, "spinner-background") || self.has_own(cv, "spinner-circle");
        if old {
            let background = cv.color("SpinnerBackground", [100, 100, 100]);
            self.draw(
                ui,
                cv,
                put("spinner-background", "spinner-background")
                    .at((683.0, 430.0))
                    .tint(background)
                    .nominal(1024.0, 692.0),
            );
            self.draw(
                ui,
                cv,
                put("spinner-circle", "spinner-circle")
                    .at(center)
                    .angle(spin)
                    .nominal(500.0, 500.0),
            );
        } else {
            self.draw(
                ui,
                cv,
                put("spinner-glow", "spinner-glow")
                    .at(center)
                    .tint(Color32::from_rgb(2, 170, 255))
                    .nominal(500.0, 500.0)
                    .optional(),
            );
            self.draw(
                ui,
                cv,
                put("spinner-bottom", "spinner-bottom")
                    .at(center)
                    .angle(spin * 0.25)
                    .nominal(480.0, 480.0),
            );
            self.draw(
                ui,
                cv,
                put("spinner-top", "spinner-top")
                    .at(center)
                    .angle(spin)
                    .nominal(400.0, 400.0)
                    .optional(),
            );
            self.draw(
                ui,
                cv,
                put("spinner-middle2", "spinner-middle2")
                    .at(center)
                    .nominal(120.0, 120.0)
                    .optional(),
            );
            self.draw(
                ui,
                cv,
                put("spinner-middle", "spinner-middle")
                    .at(center)
                    .tint(Color32::from_rgb(255, 120, 120))
                    .nominal(48.0, 48.0),
            );
        }
        let t = (cv.time * 0.3).fract();
        self.draw(
            ui,
            cv,
            put("spinner-approachcircle", "spinner-approachcircle")
                .at(center)
                .scale(0.2 + 0.8 * (1.0 - t))
                .nominal(500.0, 500.0)
                .optional()
                .quiet(),
        );
        let clear = self.draw(
            ui,
            cv,
            put("spinner-clear", "spinner-clear")
                .at((683.0, 190.0))
                .nominal(400.0, 80.0),
        );
        self.draw(
            ui,
            cv,
            put("spinner-spin", "spinner-spin")
                .at((683.0, 650.0))
                .nominal(300.0, 60.0),
        );
        let rpm = self.draw(
            ui,
            cv,
            put("spinner-rpm", "spinner-rpm")
                .at((683.0, H))
                .anchor(Align2::CENTER_BOTTOM)
                .nominal(280.0, 56.0),
        );
        cv.painter(ui).text(
            rpm.right_center() - vec2(14.0 * cv.s, 0.0),
            Align2::RIGHT_CENTER,
            "477",
            font_strong(22.0 * cv.s),
            Color32::WHITE,
        );
        self.speaker(ui, cv, "snd:spinnerspin", cv.p(center) + vec2(60.0, -60.0));
        self.speaker(ui, cv, "snd:spinnerbonus", Self::corner(clear));
    }

    fn scene_break(&mut self, ui: &egui::Ui, cv: &mut Canvas) {
        cv.painter(ui)
            .rect_filled(cv.rect, 0, Color32::from_rgb(22, 20, 28));
        let ready = self.draw(
            ui,
            cv,
            put("ready", "ready")
                .at((683.0, 150.0))
                .nominal(420.0, 120.0),
        );
        self.speaker(ui, cv, "snd:readys", Self::corner(ready));
        for (k, (stem, sound)) in [
            ("count3", "snd:count3s"),
            ("count2", "snd:count2s"),
            ("count1", "snd:count1s"),
            ("go", "snd:gos"),
        ]
        .into_iter()
        .enumerate()
        {
            let rect = self.draw(
                ui,
                cv,
                put(stem, stem)
                    .at((260.0 + k as f32 * 280.0, 390.0))
                    .scale(0.55)
                    .nominal(360.0, 300.0),
            );
            self.speaker(ui, cv, sound, Self::corner(rect));
        }
        for (k, (stem, sound)) in [
            ("section-pass", "snd:sectionpass"),
            ("section-fail", "snd:sectionfail"),
        ]
        .into_iter()
        .enumerate()
        {
            let rect = self.draw(
                ui,
                cv,
                put(stem, stem)
                    .at((480.0 + k as f32 * 400.0, 630.0))
                    .scale(0.5)
                    .nominal(300.0, 200.0),
            );
            self.speaker(ui, cv, sound, Self::corner(rect));
        }
        for (x, angle) in [(70.0, 0.0), (W - 70.0, PI)] {
            self.draw(
                ui,
                cv,
                put("arrow-warning", "arrow-warning")
                    .at((x, 560.0))
                    .angle(angle)
                    .nominal(100.0, 80.0),
            );
            self.draw(
                ui,
                cv,
                put("play-warningarrow", "play-warningarrow")
                    .at((x, 700.0))
                    .angle(angle)
                    .nominal(100.0, 80.0),
            );
        }
    }

    fn scene_pause(&mut self, ui: &egui::Ui, cv: &mut Canvas, fail: bool) {
        let (key, music) = if fail {
            ("fail-background", "snd:failsound")
        } else {
            ("pause-overlay", "snd:pause-loop")
        };
        if !self.cover(ui, cv, key, key) {
            cv.painter(ui).rect_filled(
                cv.rect,
                0,
                if fail {
                    Color32::from_rgb(48, 20, 26)
                } else {
                    Color32::from_rgb(30, 28, 38)
                },
            );
            self.text(
                ui,
                cv,
                (683.0, 90.0),
                Align2::CENTER_CENTER,
                if fail {
                    self.t(
                        "Фон неудачной попытки — нет файла",
                        "Fail background — no file",
                    )
                } else {
                    self.t("Фон паузы — нет файла", "Pause overlay — no file")
                },
                18.0,
                Color32::from_white_alpha(120),
            );
        }
        self.speaker(ui, cv, music, cv.p((1320.0, 50.0)));
        let buttons: &[(&str, f32, &str)] = if fail {
            &[
                ("pause-retry", 330.0, "snd:pause-retry-*"),
                ("pause-back", 490.0, "snd:pause-back-*"),
            ]
        } else {
            &[
                ("pause-continue", 250.0, "snd:pause-continue-*"),
                ("pause-retry", 400.0, "snd:pause-retry-*"),
                ("pause-back", 550.0, "snd:pause-back-*"),
            ]
        };
        let mut first = None;
        for &(stem, y, sound) in buttons {
            let rect = self.draw(ui, cv, put(stem, stem).at((683.0, y)).nominal(360.0, 100.0));
            first.get_or_insert(rect);
            self.speaker(ui, cv, sound, rect.right_center() + vec2(22.0, 0.0));
        }
        if let Some(first) = first {
            let (x, y) = cv.back(first.left_center());
            let bob = (cv.time * 4.0).sin() * 6.0;
            self.draw(
                ui,
                cv,
                put("arrow-pause", "arrow-pause")
                    .at((x - 50.0 + bob, y))
                    .nominal(64.0, 48.0),
            );
        }
    }

    fn scene_results(&mut self, ui: &egui::Ui, cv: &mut Canvas) {
        let painter = cv.painter(ui);
        painter.rect_filled(cv.rect, 0, Color32::from_rgb(24, 22, 30));
        painter.rect_filled(
            Rect::from_min_size(cv.rect.min, vec2(cv.rect.width(), 96.0 * cv.s)),
            0,
            Color32::from_black_alpha(170),
        );
        self.text(
            ui,
            cv,
            (18.0, 12.0),
            Align2::LEFT_TOP,
            "xi — Blue Zenith [FOUR DIMENSIONS]",
            24.0,
            Color32::WHITE,
        );
        self.text(
            ui,
            cv,
            (18.0, 46.0),
            Align2::LEFT_TOP,
            self.t("Сыграно сегодня", "Played today"),
            16.0,
            Color32::from_gray(210),
        );
        self.draw(
            ui,
            cv,
            put("ranking-title", "ranking-title")
                .at((1346.0, 4.0))
                .anchor(Align2::RIGHT_TOP)
                .nominal(340.0, 84.0),
        );
        self.draw(
            ui,
            cv,
            put("ranking-panel", "ranking-panel")
                .at((0.0, 100.0))
                .anchor(Align2::LEFT_TOP)
                .nominal(622.0, 480.0),
        );
        let fonts = "grp:Interface/In-game/Fonts";
        self.digits(
            ui,
            cv,
            fonts,
            "ScorePrefix",
            "score-0",
            "07274110",
            (190.0, 120.0),
            Align2::LEFT_TOP,
            1.1,
        );
        let judgements = "grp:Gameplay/osu!/Hit judgements";
        for (stem, at, count) in [
            ("hit300", (80.0, 250.0), "412x"),
            ("hit100", (80.0, 335.0), "23x"),
            ("hit50", (350.0, 250.0), "2x"),
            ("hit0", (350.0, 335.0), "5x"),
        ] {
            self.draw(
                ui,
                cv,
                put(judgements, stem)
                    .at(at)
                    .scale(0.55)
                    .nominal(128.0, 80.0),
            );
            self.digits(
                ui,
                cv,
                fonts,
                "ScorePrefix",
                "score-0",
                count,
                (at.0 + 60.0, at.1),
                Align2::LEFT_CENTER,
                0.8,
            );
        }
        self.draw(
            ui,
            cv,
            put("ranking-maxcombo", "ranking-maxcombo")
                .at((16.0, 400.0))
                .anchor(Align2::LEFT_TOP)
                .nominal(220.0, 40.0),
        );
        self.digits(
            ui,
            cv,
            fonts,
            "ScorePrefix",
            "score-0",
            "247x",
            (30.0, 450.0),
            Align2::LEFT_TOP,
            0.9,
        );
        self.draw(
            ui,
            cv,
            put("ranking-accuracy", "ranking-accuracy")
                .at((300.0, 400.0))
                .anchor(Align2::LEFT_TOP)
                .nominal(220.0, 40.0),
        );
        self.digits(
            ui,
            cv,
            fonts,
            "ScorePrefix",
            "score-0",
            "98.76%",
            (314.0, 450.0),
            Align2::LEFT_TOP,
            0.9,
        );
        self.draw(
            ui,
            cv,
            put("ranking-graph", "ranking-graph")
                .at((10.0, 505.0))
                .anchor(Align2::LEFT_TOP)
                .nominal(330.0, 160.0),
        );
        self.draw(
            ui,
            cv,
            put("ranking-perfect", "ranking-perfect")
                .at((520.0, 640.0))
                .nominal(300.0, 90.0),
        );
        let grade = self.draw(
            ui,
            cv,
            put("grp:Interface/Results screen/Grades", "ranking-s")
                .at((1040.0, 320.0))
                .nominal(360.0, 400.0),
        );
        self.speaker(ui, cv, "snd:applause", Self::corner(grade));
        for (k, stem) in ["selection-mod-hidden", "selection-mod-hardrock"]
            .into_iter()
            .enumerate()
        {
            self.draw(
                ui,
                cv,
                put("grp:Interface/Mod icons", stem)
                    .at((700.0 + k as f32 * 70.0, 540.0))
                    .nominal(68.0, 66.0),
            );
        }
        self.draw(
            ui,
            cv,
            put("ranking-retry", "ranking-retry")
                .at((W, 520.0))
                .anchor(Align2::RIGHT_TOP)
                .nominal(380.0, 90.0),
        );
        self.draw(
            ui,
            cv,
            put("ranking-replay", "ranking-replay")
                .at((W, 620.0))
                .anchor(Align2::RIGHT_TOP)
                .nominal(380.0, 90.0),
        );
        let back = self.draw(
            ui,
            cv,
            put("menu-back", "menu-back")
                .at((0.0, H))
                .anchor(Align2::LEFT_BOTTOM)
                .nominal(190.0, 86.0),
        );
        self.speaker(ui, cv, "snd:menuback", Self::corner(back));
    }

    // ------------------------------------------------------------ interaction

    fn scene_interaction(&mut self, ui: &egui::Ui, cv: &Canvas, response: &egui::Response) {
        let painter = cv.painter(ui);
        let picked = self.picked_ids();
        if !picked.is_empty() {
            let mut outlined: Vec<Rect> = vec![];
            for spot in cv
                .spots
                .iter()
                .filter(|s| !s.sound && picked.iter().any(|p| related(&s.id, p)))
            {
                if outlined.iter().any(|r| r.contains_rect(spot.rect)) {
                    continue;
                }
                outlined.push(spot.rect);
                let rect = if spot.rect == cv.rect {
                    spot.rect.shrink(3.0)
                } else {
                    spot.rect.expand(3.0)
                };
                painter.rect_stroke(
                    rect,
                    2,
                    Stroke::new(2.0, ACCENT_LINE),
                    egui::StrokeKind::Middle,
                );
                // A picked element is landed on like a hit: a frame closes in on it.
                super::motion::approach(ui, ui.make_persistent_id(("spot", &spot.id)), rect, true);
            }
        }
        let hovered = response
            .hover_pos()
            .and_then(|p| cv.spots.iter().rev().find(|s| s.rect.contains(p)));
        let Some(spot) = hovered else {
            return;
        };
        let nodes = self.nodes.clone();
        let Some(node) = node_at(&nodes, &spot.id) else {
            return;
        };
        if !spot.sound {
            let rect = if spot.rect == cv.rect {
                spot.rect.shrink(1.0)
            } else {
                spot.rect.expand(1.0)
            };
            painter.rect_stroke(
                rect,
                2,
                Stroke::new(1.5, Color32::WHITE),
                egui::StrokeKind::Middle,
            );
        }
        let source = self.effective(&spot.id);
        let label = format!(
            "{}{}  ·  {}",
            if spot.sound {
                self.t("Звук: ", "Sound: ")
            } else {
                ""
            },
            node.label(self.ru()),
            source_name(self.settings.language, &source)
        );
        let galley = painter.layout_no_wrap(label, font(12.5), ON_INK);
        let mut at = pos2(spot.rect.left(), spot.rect.top() - galley.size().y - 8.0);
        if at.y < cv.rect.top() + 2.0 {
            at.y = spot.rect.bottom() + 4.0;
        }
        if spot.rect.height() > cv.rect.height() * 0.8 {
            at = response.hover_pos().unwrap_or(at) + vec2(14.0, 14.0);
        }
        // A tag wider than the canvas starts at its left edge instead of failing the clamp.
        let right = (cv.rect.right() - galley.size().x - 10.0).max(cv.rect.left() + 2.0);
        at.x = at.x.clamp(cv.rect.left() + 2.0, right);
        at.y = at.y.min(cv.rect.bottom() - galley.size().y - 6.0);
        let tag = Rect::from_min_size(at, galley.size() + vec2(10.0, 4.0));
        painter.rect_filled(tag, 2, INK);
        painter.galley(tag.min + vec2(5.0, 2.0), galley, ON_INK);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        if response.clicked() {
            let id = spot.id.clone();
            if spot.sound {
                self.play_node(&id);
            }
            // The scene has no order, so Shift works like Ctrl here.
            let how = match Pick::from(ui) {
                Pick::Range => Pick::Toggle,
                how => how,
            };
            self.pick(&id, how, &[]);
        }
    }

    /// Plays what the mix uses for a sound node (a group plays its first sound).
    pub(crate) fn play_node(&mut self, id: &str) {
        let nodes = self.nodes.clone();
        let Some(node) = node_at(&nodes, id) else {
            return;
        };
        fn first_sound(node: &OptionNode, id: &str) -> Option<(String, String)> {
            if let Some(stem) = node.target.as_ref().and_then(audio_stem) {
                return Some((id.to_owned(), stem));
            }
            node.children
                .iter()
                .enumerate()
                .find_map(|(i, c)| first_sound(c, &format!("{id}.{i}")))
        }
        let Some((leaf, stem)) = first_sound(node, id) else {
            return;
        };
        match self.origin(&self.effective(&leaf)) {
            Origin::Folder(folder) => self.play(folder.as_deref(), &stem),
            Origin::Blank => self.stop_audio(),
        }
    }
}
