//! Motion from the kohu! site, drawn by hand: the hit circle (orb) with its approach
//! ring, the soft glow that follows the cursor, glass sheen on cards, the run around the
//! moment button and the burst when it is pressed, approach frames when something is
//! picked. "Reduce motion" (animation time 0) turns every one of them off.

use super::theme::*;
use eframe::egui::{self, Color32, Id, Mesh, Pos2, Rect, Sense, Shape, Stroke, Vec2, pos2, vec2};

pub fn reduced(ctx: &egui::Context) -> bool {
    ctx.global_style().animation_time == 0.0
}

/// kohu!'s `--kh-ease`, cubic-bezier(.2, 0, 0, 1), close enough.
pub fn ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(4)
}

fn ease_in(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

fn now(ctx: &egui::Context) -> f64 {
    ctx.input(|i| i.time)
}

/// Seconds since `id` was triggered, if it ever was.
pub fn since(ctx: &egui::Context, id: Id) -> Option<f32> {
    ctx.data(|d| d.get_temp::<f64>(id))
        .map(|t| (now(ctx) - t) as f32)
}

pub fn trigger(ctx: &egui::Context, id: Id) {
    let t = now(ctx);
    ctx.data_mut(|d| d.insert_temp(id, t));
}

/// True once when `value` turns from false to true (also on the first frame it is true
/// when `first_counts`). Something not drawn last frame counts as false.
pub fn rose(ctx: &egui::Context, id: Id, value: bool, first_counts: bool) -> bool {
    let pass = ctx.cumulative_pass_nr();
    let before = ctx.data_mut(|d| {
        let before = d.get_temp::<(bool, u64)>(id);
        d.insert_temp(id, (value, pass));
        before
    });
    let before = before.map(|(b, seen)| b && seen + 2 >= pass);
    value && before.map_or(first_counts, |b| !b)
}

/// A disc fading from `color` in the middle to nothing at `radius`.
pub fn radial(center: Pos2, radius: f32, color: Color32) -> Mesh {
    const STOPS: [(f32, f32); 6] = [
        (0.0, 1.0),
        (0.2, 0.86),
        (0.4, 0.58),
        (0.6, 0.3),
        (0.8, 0.1),
        (1.0, 0.0),
    ];
    let segments = 48;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, color);
    for &(r, a) in &STOPS[1..] {
        for i in 0..segments {
            let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
            mesh.colored_vertex(
                center + vec2(angle.cos(), angle.sin()) * radius * r,
                color.gamma_multiply(a),
            );
        }
    }
    for i in 0..segments as u32 {
        let j = (i + 1) % segments as u32;
        mesh.add_triangle(0, 1 + i, 1 + j);
        for ring in 0..(STOPS.len() as u32 - 2) {
            let a = 1 + ring * segments as u32;
            let b = a + segments as u32;
            mesh.add_triangle(a + i, a + j, b + i);
            mesh.add_triangle(a + j, b + j, b + i);
        }
    }
    mesh
}

/// A filled circle shaded from `top` to `bottom`.
fn shaded_disc(center: Pos2, radius: f32, top: Color32, bottom: Color32) -> Mesh {
    let segments = 64;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, top.lerp_to_gamma(bottom, 0.5));
    for i in 0..segments {
        let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
        let p = center + vec2(angle.cos(), angle.sin()) * radius;
        let t = (p.y - (center.y - radius)) / (2.0 * radius);
        mesh.colored_vertex(p, top.lerp_to_gamma(bottom, t));
    }
    for i in 0..segments as u32 {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % segments as u32);
    }
    mesh
}

fn arc(center: Pos2, radius: f32, from: f32, to: f32) -> Vec<Pos2> {
    (0..=20)
        .map(|i| {
            let a = (from + (to - from) * i as f32 / 20.0).to_radians();
            center + vec2(a.cos(), a.sin()) * radius
        })
        .collect()
}

/// The lower half of the orb's core: pink running into lavender.
const ORB_LOW: Color32 = Color32::from_rgb(0xd2, 0xa6, 0xde);

/// The site's hit circle. It plays one hit when it first appears — the approach ring
/// closes in, the circle takes the hit, "300" floats up — then rests and leans a little
/// toward the cursor. Clicking it plays the hit again.
pub fn orb(ui: &mut egui::Ui, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let ctx = ui.ctx().clone();
    let id = response.id;
    let still = reduced(&ctx);
    if response.clicked() || since(&ctx, id).is_none() {
        trigger(&ctx, id);
    }
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let c = rect.center();
    let core = size * 0.3;
    // Parallax: the core leans toward the cursor, the rings away from it.
    let lean = ctx
        .pointer_hover_pos()
        .filter(|_| !still)
        .map(|p| {
            let d = p - c;
            d / d.length().max(size * 1.5)
        })
        .unwrap_or(Vec2::ZERO);
    let lx = ctx.animate_value_with_time(id.with("lx"), lean.x, 0.9);
    let ly = ctx.animate_value_with_time(id.with("ly"), lean.y, 0.9);
    let lean = vec2(lx, ly);

    // One hit: 0.25 s rest, then 1.5 s of approach, take and score.
    let t = if still {
        f32::INFINITY
    } else {
        since(&ctx, id).unwrap_or(0.0) - 0.25
    };
    let u = (t / 1.5).clamp(0.0, 1.0);
    let take = if (0.82..0.88).contains(&u) {
        1.0 - 0.025 * (u - 0.82) / 0.06
    } else if (0.88..0.94).contains(&u) {
        0.975 + 0.037 * (u - 0.88) / 0.06
    } else if (0.94..1.0).contains(&u) {
        1.012 - 0.012 * (u - 0.94) / 0.06
    } else {
        1.0
    };

    let painter = ui.painter();
    let rings = c - lean * size * 0.02;
    painter.circle_stroke(rings, size * 0.49, Stroke::new(1.5, LINE_100));
    let track = size * 0.4;
    let dots = ((std::f32::consts::TAU * track) / 11.0).floor().max(12.0) as usize;
    for i in 0..dots {
        let a = i as f32 / dots as f32 * std::f32::consts::TAU;
        painter.circle_filled(rings + vec2(a.cos(), a.sin()) * track, 0.9, VIOLET.gamma_multiply(0.7));
    }
    let hit = c + lean * size * 0.035;
    let r = core * take;
    painter.circle_filled(hit + vec2(0.0, r * 0.1), r * 1.04, Color32::from_black_alpha(70));
    painter.add(shaded_disc(hit, r, ACCENT_HOVER, ORB_LOW));
    painter.add(radial(
        hit + vec2(-r * 0.32, -r * 0.38),
        r * 0.75,
        PLUM_TEXT.gamma_multiply(0.45),
    ));
    painter.circle_stroke(hit, r, Stroke::new((r * 0.09).max(1.5), PLUM_TEXT));
    painter.circle_filled(hit, r * 0.2, ON_ACCENT);
    painter.add(Shape::line(
        arc(hit, r * 0.72, 200.0, 250.0),
        Stroke::new((r * 0.07).max(1.2), PLUM_TEXT.gamma_multiply(0.75)),
    ));

    if t.is_finite() && t > 0.0 && t < 2.6 {
        // Approach ring: accelerates in, holds on the rim, fades as it is taken.
        let (scale, alpha) = if u < 0.84 {
            let k = u / 0.84;
            (1.7 - 0.7 * k.powf(1.6), 0.55 + 0.15 * ease_in(k))
        } else {
            let k = (u - 0.84) / 0.16;
            (1.0 + 0.05 * k, 0.7 * (1.0 - k))
        };
        let alpha = alpha * ((u / 0.2).min(1.0));
        if u < 1.0 {
            painter.circle_stroke(
                hit,
                core * scale,
                Stroke::new((size * 0.015).max(2.0), ACCENT_LINE.gamma_multiply(alpha)),
            );
        }
        // "300" floats up after the hit.
        let s = (t - 1.26) / 0.9;
        if (0.0..1.0).contains(&s) {
            let fade = if s < 0.25 { s / 0.25 } else { 1.0 - (s - 0.25) / 0.75 };
            painter.text(
                hit + vec2(core * 1.05, -core * 0.9 - 10.0 * ease(s)),
                egui::Align2::LEFT_BOTTOM,
                "300",
                serif((size * 0.24).max(18.0)),
                ACCENT_TEXT.gamma_multiply(fade),
            );
        }
        ctx.request_repaint();
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The site's stage glow: a wide, faint pink light under the cursor.
pub fn cursor_glow(ui: &egui::Ui, rect: Rect) {
    let ctx = ui.ctx();
    if reduced(ctx) {
        return;
    }
    let pointer = ctx.pointer_hover_pos().filter(|p| rect.contains(*p));
    let id = ui.scope_id().with("cursor_glow");
    let shown = ctx.animate_bool_with_time(id, pointer.is_some(), 0.3);
    if shown <= 0.0 {
        return;
    }
    let last = id.with("at");
    let at = match pointer {
        Some(p) => {
            ctx.data_mut(|d| d.insert_temp(last, p));
            p
        }
        None => ctx.data(|d| d.get_temp::<Pos2>(last)).unwrap_or(rect.center()),
    };
    ui.painter()
        .with_clip_rect(rect)
        .add(radial(at, 560.0, ACCENT.gamma_multiply(0.075 * shown)));
}

/// Glass card: a raised fill with a faint sheen that follows the cursor, a light edge and
/// a one-pixel highlight along the top.
pub fn glass<R>(
    ui: &mut egui::Ui,
    fill: Color32,
    margin: impl Into<egui::Margin>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let mut sheen_idx = None;
    let inner = egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, INK.gamma_multiply(0.13)))
        .corner_radius(RADIUS_MD)
        .inner_margin(margin)
        .show(ui, |ui| {
            sheen_idx = Some(ui.painter().add(Shape::Noop));
            add_contents(ui)
        });
    if let Some(idx) = sheen_idx {
        let rect = inner.response.rect;
        let mut shapes = vec![];
        let ctx = ui.ctx();
        if !reduced(ctx) {
            if let Some(p) = ctx.pointer_hover_pos() {
                if rect.expand(160.0).contains(p) {
                    let clip = rect.shrink(2.0);
                    let mut mesh = radial(p, 420.0, INK.gamma_multiply(0.07));
                    // Clip by hand: a mesh has no clip of its own inside a shape list.
                    for v in &mut mesh.vertices {
                        v.pos = v.pos.clamp(clip.min, clip.max);
                    }
                    shapes.push(Shape::mesh(mesh));
                }
            }
        }
        let r = RADIUS_MD as f32;
        shapes.push(Shape::line_segment(
            [
                pos2(rect.left() + r, rect.top() + 1.0),
                pos2(rect.right() - r, rect.top() + 1.0),
            ],
            Stroke::new(1.0, INK.gamma_multiply(0.09)),
        ));
        ui.painter().set(idx, Shape::Vec(shapes));
    }
    inner
}

fn perimeter_point(rect: Rect, s: f32) -> Pos2 {
    let (w, h) = (rect.width(), rect.height());
    let total = 2.0 * (w + h);
    let mut d = s.rem_euclid(1.0) * total;
    if d < w {
        return pos2(rect.left() + d, rect.top());
    }
    d -= w;
    if d < h {
        return pos2(rect.right(), rect.top() + d);
    }
    d -= h;
    if d < w {
        return pos2(rect.right() - d, rect.bottom());
    }
    d -= w;
    pos2(rect.left(), rect.bottom() - d)
}

/// The moment: a light runs once around the button when it becomes available, and a
/// ring bursts out of it when pressed.
pub fn moment(ui: &egui::Ui, id: Id, rect: Rect, enabled: bool, clicked: bool) {
    let ctx = ui.ctx();
    if reduced(ctx) {
        return;
    }
    let run = id.with("run");
    let burst = id.with("burst");
    if rose(ctx, id.with("enabled"), enabled, true) {
        trigger(ctx, run);
    }
    if clicked {
        trigger(ctx, burst);
    }
    let painter = ui.painter();
    let mut busy = false;
    if let Some(t) = since(ctx, run) {
        let u = t / 1.8;
        if u < 1.0 {
            busy = true;
            let k = 0.5 - 0.5 * (std::f32::consts::PI * u).cos();
            let alpha = (u / 0.12).min(1.0) * ((1.0 - u) / 0.2).min(1.0);
            let path = rect.expand(3.0);
            let points: Vec<Pos2> = (0..=24)
                .map(|i| perimeter_point(path, k * 1.25 - 0.25 + i as f32 / 24.0 * 0.25))
                .collect();
            painter.add(Shape::line(
                points,
                Stroke::new(2.0, ACCENT_LINE.gamma_multiply(alpha)),
            ));
        }
    }
    if let Some(t) = since(ctx, burst) {
        let u = t / 0.45;
        if u < 1.0 {
            busy = true;
            painter.rect_stroke(
                rect.expand(1.0 + 7.0 * ease(u)),
                RADIUS_SM + 3,
                Stroke::new(2.0, ACCENT_LINE.gamma_multiply(1.0 - u)),
                egui::StrokeKind::Outside,
            );
        }
    }
    if busy {
        ctx.request_repaint();
    }
}

/// An approach frame: when `active` turns on, a frame closes in on `rect` like an osu!
/// approach circle and fades as it lands.
pub fn approach(ui: &egui::Ui, id: Id, rect: Rect, active: bool) {
    let ctx = ui.ctx();
    if reduced(ctx) {
        return;
    }
    if rose(ctx, id.with("active"), active, false) {
        trigger(ctx, id.with("approach"));
    }
    if let Some(t) = since(ctx, id.with("approach")) {
        let u = t / 0.34;
        if u < 1.0 && active {
            let grow = 14.0 * (1.0 - ease(u));
            ui.painter().rect_stroke(
                rect.expand(grow),
                2,
                Stroke::new(2.0, ACCENT_LINE.gamma_multiply(0.25 + 0.6 * u.min(0.8))),
                egui::StrokeKind::Outside,
            );
            ctx.request_repaint();
        }
    }
}

/// A short fade on panel changes; the first frame is already fully visible.
pub fn panel_opacity(ctx: &egui::Context, id: Id, key: u64) -> f32 {
    let still = reduced(ctx);
    let shown = ctx.data(|d| d.get_temp::<u64>(id.with("key")));
    if shown != Some(key) {
        ctx.data_mut(|d| d.insert_temp(id.with("key"), key));
        if shown.is_some() && !still {
            trigger(ctx, id);
        }
    }
    if still {
        ctx.data_mut(|d| d.remove::<f64>(id));
        return 1.0;
    }
    let t = ease(since(ctx, id).unwrap_or(1.0) / 0.15);
    if t < 1.0 {
        ctx.request_repaint();
    }
    0.75 + 0.25 * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opacity_at(ctx: &egui::Context, time: f64, key: u64) -> f32 {
        let mut opacity = 0.0;
        ctx.run_pass(
            egui::RawInput {
                time: Some(time),
                ..Default::default()
            },
            |ctx| {
                opacity = panel_opacity(ctx, Id::unique("panel"), key);
            },
        )
        .drop_without_applying_deltas();
        opacity
    }

    #[test]
    fn panel_switches_finish_and_restart_on_the_latest_selection() {
        let ctx = egui::Context::default();
        assert_eq!(opacity_at(&ctx, 0.0, 0), 1.0);
        assert_eq!(opacity_at(&ctx, 0.05, 0), 1.0);
        assert_eq!(opacity_at(&ctx, 1.0, 1), 0.75);
        let halfway = opacity_at(&ctx, 1.075, 1);
        assert!(halfway > 0.75 && halfway < 1.0);
        assert_eq!(opacity_at(&ctx, 1.2, 1), 1.0);
        assert_eq!(opacity_at(&ctx, 2.0, 2), 0.75);
        assert_eq!(opacity_at(&ctx, 2.05, 3), 0.75);
        assert_eq!(opacity_at(&ctx, 2.25, 3), 1.0);
    }

    #[test]
    fn reduced_motion_cancels_the_fade_without_replaying_it() {
        let ctx = egui::Context::default();
        assert_eq!(opacity_at(&ctx, 0.0, 0), 1.0);
        assert_eq!(opacity_at(&ctx, 1.0, 1), 0.75);
        ctx.global_style_mut(|style| style.animation_time = 0.0);
        assert_eq!(opacity_at(&ctx, 1.01, 1), 1.0);
        assert_eq!(opacity_at(&ctx, 1.02, 2), 1.0);
        ctx.global_style_mut(|style| style.animation_time = 0.12);
        assert_eq!(opacity_at(&ctx, 1.03, 2), 1.0);
    }
}
