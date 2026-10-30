use crate::{
    catalog::{OptionNode, Target, catalog_ref},
    ini::SkinIni,
    library::{check_cancel, relative_path},
    mixer::TRANSPARENT,
};
use anyhow::{Context, Result, ensure};
use image::{
    Rgba, RgbaImage,
    imageops::{self, FilterType},
};
use std::{fs, io::Cursor, path::Path, sync::atomic::AtomicBool};

include!(concat!(env!("OUT_DIR"), "/defaults.rs"));

pub fn decode_image(bytes: &[u8]) -> Result<RgbaImage> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?.to_rgba8())
}

pub fn element_image(folder: &Path, stem: &str, high_dpi: bool) -> Result<RgbaImage> {
    let stem = relative_path(stem)?;
    let stem = stem.to_string_lossy();
    let suffix = if high_dpi { "@2x" } else { "" };
    let filename = format!("{stem}{suffix}.png");
    let path = folder.join(&filename);
    if path.exists() {
        return decode_image(&fs::read(path)?);
    }
    if high_dpi && folder.join(format!("{stem}.png")).exists() {
        let image = decode_image(&fs::read(folder.join(format!("{stem}.png")))?)?;
        return Ok(imageops::resize(
            &image,
            image.width() * 2,
            image.height() * 2,
            FilterType::Lanczos3,
        ));
    }
    let fallback = stem.rsplit('/').next().unwrap_or(&stem);
    let filename = format!("{fallback}{suffix}.png");
    decode_image(default_bytes(&filename).context(format!("Missing image: {filename}"))?)
}

#[derive(Clone, Debug)]
pub struct Modifications {
    pub smooth_trail: bool,
    pub instafade: bool,
    pub static_interface: bool,
    pub invisible_cursor: bool,
    pub combo_colors: Option<Vec<[u8; 3]>>,
    pub cursor_color: Option<[u8; 3]>,
    pub saturation_threshold: f32,
}
impl Default for Modifications {
    fn default() -> Self {
        Self {
            smooth_trail: false,
            instafade: false,
            static_interface: false,
            invisible_cursor: false,
            combo_colors: None,
            cursor_color: None,
            saturation_threshold: 0.1,
        }
    }
}
impl Modifications {
    pub fn apply(&self, folder: &Path, ini: &mut SkinIni, cancel: &AtomicBool) -> Result<()> {
        if let Some(colors) = &self.combo_colors {
            set_combo_colors(ini, colors)?;
        }
        if let Some(color) = self.cursor_color {
            for stem in ["cursor", "cursormiddle", "cursortrail"] {
                for suffix in ["", "@2x"] {
                    check_cancel(cancel)?;
                    let path = folder.join(format!("{stem}{suffix}.png"));
                    if path.exists() {
                        let mut image = decode_image(&fs::read(&path)?)?;
                        recolor(&mut image, color, self.saturation_threshold);
                        image.save(path)?;
                    }
                }
            }
        }
        if self.smooth_trail {
            for suffix in ["", "@2x"] {
                check_cancel(cancel)?;
                smooth_trail(folder, suffix)?;
            }
        }
        if self.instafade {
            for high_dpi in [true, false] {
                check_cancel(cancel)?;
                instafade(folder, ini, high_dpi)?;
            }
        }
        if self.static_interface {
            static_interface(folder, cancel)?;
        }
        if self.invisible_cursor {
            for stem in ["cursor", "cursortrail", "cursormiddle"] {
                for suffix in ["", "@2x"] {
                    fs::write(folder.join(format!("{stem}{suffix}.png")), TRANSPARENT)?;
                }
            }
        }
        Ok(())
    }
}

pub fn set_combo_colors(ini: &mut SkinIni, colors: &[[u8; 3]]) -> Result<()> {
    ensure!(
        !colors.is_empty() && colors.len() <= 8,
        "Use between 1 and 8 combo colors"
    );
    let section = ini.section_mut("Colours");
    for n in 1..=8 {
        section.remove(&format!("Combo{n}"));
    }
    // osu! starts play at Combo2, so the last user color belongs at Combo1.
    for n in 0..colors.len().max(2) {
        let color = colors[(n + colors.len() - 1) % colors.len()];
        section.set(
            &format!("Combo{}", n + 1),
            format!("{},{},{}", color[0], color[1], color[2]),
        );
    }
    Ok(())
}

fn smooth_trail(folder: &Path, suffix: &str) -> Result<()> {
    let cursor = folder.join(format!("cursor{suffix}.png"));
    let trail = folder.join(format!("cursortrail{suffix}.png"));
    if !cursor.is_file() || !trail.is_file() {
        return Ok(());
    }
    let mut image = decode_image(&fs::read(&trail)?)?;
    let mut bounds = (image.width(), image.height(), 0, 0);
    let mut visible = false;
    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] > 0 {
            bounds = (
                bounds.0.min(x),
                bounds.1.min(y),
                bounds.2.max(x),
                bounds.3.max(y),
            );
            visible = true;
        }
    }
    if !visible {
        return Ok(());
    }
    imageops::crop(
        &mut image,
        bounds.0,
        bounds.1,
        bounds.2 - bounds.0 + 1,
        bounds.3 - bounds.1 + 1,
    )
    .to_image()
    .save(&trail)?;
    fs::copy(&cursor, folder.join(format!("cursormiddle{suffix}.png")))?;
    fs::write(cursor, TRANSPARENT)?;
    Ok(())
}

fn instafade(folder: &Path, ini: &mut SkinIni, high_dpi: bool) -> Result<()> {
    let prefix = ini
        .get("Fonts", "HitCirclePrefix")
        .unwrap_or("default")
        .to_owned();
    relative_path(&prefix)?;
    let color = {
        let colors = ini.colors();
        colors[if colors.len() > 1 { 1 } else { 0 }]
    };
    let mut circle = element_image(folder, "hitcircle", high_dpi)?;
    for pixel in circle.pixels_mut() {
        for n in 0..3 {
            pixel[n] = (pixel[n] as u16 * color[n] as u16 / 255) as u8;
        }
    }
    let overlay = element_image(folder, "hitcircleoverlay", high_dpi)?;
    ensure!(
        circle
            .width()
            .max(circle.height())
            .max(overlay.width())
            .max(overlay.height())
            <= 4096,
        "Images are too large for instafade"
    );
    let circle = imageops::resize(
        &circle,
        (circle.width() as f32 * 1.25).round() as u32,
        (circle.height() as f32 * 1.25).round() as u32,
        FilterType::Lanczos3,
    );
    let overlay = imageops::resize(
        &overlay,
        (overlay.width() as f32 * 1.25).round() as u32,
        (overlay.height() as f32 * 1.25).round() as u32,
        FilterType::Lanczos3,
    );
    let suffix = if high_dpi { "@2x" } else { "" };
    let mut width = 0;
    for n in 0..=9 {
        let stem = format!("{prefix}-{n}");
        let digit = if folder.join(format!("{stem}{suffix}.png")).is_file()
            || (high_dpi && folder.join(format!("{stem}.png")).is_file())
        {
            element_image(folder, &stem, high_dpi)?
        } else {
            decode_image(
                default_bytes(&format!("default-{n}{suffix}.png"))
                    .context("Missing default digit")?,
            )?
        };
        let mut output = RgbaImage::new(
            circle.width().max(overlay.width()).max(digit.width()),
            circle.height().max(overlay.height()).max(digit.height()),
        );
        width = output.width();
        if n != 0 {
            for layer in [&circle, &overlay, &digit] {
                let x = (output.width() - layer.width()) as i64 / 2;
                let y = (output.height() - layer.height()) as i64 / 2;
                imageops::overlay(&mut output, layer, x, y);
            }
        }
        let target = folder.join(relative_path(&format!("{stem}{suffix}.png"))?);
        fs::create_dir_all(target.parent().context("Digit folder")?)?;
        output.save(target)?;
    }
    for stem in ["hitcircle", "hitcircleoverlay"] {
        fs::write(folder.join(format!("{stem}{suffix}.png")), TRANSPARENT)?;
    }
    if !high_dpi {
        ini.set("Fonts", "HitCircleOverlap", width.to_string());
    }
    ini.set(
        "Colours",
        "Combo1",
        format!("{},{},{}", color[0], color[1], color[2]),
    );
    for n in 3..=8 {
        ini.section_mut("Colours").remove(&format!("Combo{n}"));
    }
    Ok(())
}

fn static_interface(folder: &Path, cancel: &AtomicBool) -> Result<()> {
    let catalog = catalog_ref();
    let targets: Vec<_> = catalog[0]
        .leaves()
        .into_iter()
        .filter_map(|n: &OptionNode| n.target.as_ref())
        .filter(|t| matches!(t, Target::File { animated: true, .. }))
        .collect();
    // The folder is listed once; every animated element is picked out of that list.
    let mut files = vec![];
    for entry in fs::read_dir(folder)? {
        files.push(entry?.path());
    }
    for target in targets {
        let Target::File { pattern, .. } = target else {
            continue;
        };
        for suffix in ["", "@2x"] {
            check_cancel(cancel)?;
            let mut frames = vec![];
            for path in &files {
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let stem = if suffix.is_empty() {
                    if stem.ends_with("@2x") {
                        continue;
                    } else {
                        stem
                    }
                } else if let Some(stem) = stem.strip_suffix(suffix) {
                    stem
                } else {
                    continue;
                };
                if let Some(tail) = stem.strip_prefix(pattern)
                    && let Ok(n) = tail.trim_start_matches('-').parse::<u32>()
                    && target.matches(path)
                {
                    frames.push((n, path.clone()));
                }
            }
            frames.sort_by_key(|(n, _)| *n);
            if frames.is_empty() {
                continue;
            }
            let middle = &frames[frames.len() / 2].1;
            let still = folder.join(format!("{pattern}{suffix}.png"));
            fs::copy(middle, &still)?;
            for (_, frame) in &frames {
                fs::remove_file(frame)?;
            }
            // Keep the one listing in step with the folder for the next elements.
            // (Compared as raw strings: `Path ==` parses components on every call.)
            let removed: std::collections::HashSet<&std::ffi::OsStr> =
                frames.iter().map(|(_, f)| f.as_os_str()).collect();
            files.retain(|f| !removed.contains(f.as_os_str()));
            if !files.iter().any(|f| f.as_os_str() == still.as_os_str()) {
                files.push(still);
            }
        }
    }
    Ok(())
}

fn saturation(pixel: &Rgba<u8>) -> f32 {
    let max = *pixel.0[..3].iter().max().unwrap() as f32;
    let min = *pixel.0[..3].iter().min().unwrap() as f32;
    if max == 0.0 { 0.0 } else { (max - min) / max }
}
fn recolor(image: &mut RgbaImage, color: [u8; 3], threshold: f32) {
    let (sum, count) = image
        .pixels()
        .filter(|p| p[3] > 0)
        .fold((0.0, 0), |(sum, count), p| (sum + saturation(p), count + 1));
    let grayscale = count > 0 && sum / (count as f32) < 0.03;
    for pixel in image.pixels_mut() {
        let bright = *pixel.0[..3].iter().max().unwrap() as f32 / 255.0;
        if (grayscale && bright >= 0.1) || (!grayscale && saturation(pixel) >= threshold) {
            pixel.0[..3].copy_from_slice(&color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_single_combo_color_is_repeated_without_leaving_gaps() {
        let mut ini = SkinIni::parse("[Colours]\nCombo8: 1,2,3");
        set_combo_colors(&mut ini, &[[90, 120, 150]]).unwrap();
        assert_eq!(ini.get("Colours", "Combo1"), Some("90,120,150"));
        assert_eq!(ini.get("Colours", "Combo2"), Some("90,120,150"));
        assert_eq!(ini.get("Colours", "Combo3"), None);
        assert_eq!(ini.get("Colours", "Combo8"), None);
        assert!(set_combo_colors(&mut ini, &[]).is_err());
    }
    #[test]
    fn smooth_and_recolor_keep_alpha_and_boundaries() {
        let folder = tempfile::tempdir().unwrap();
        let mut trail = RgbaImage::new(10, 10);
        trail.put_pixel(9, 9, Rgba([255, 255, 255, 128]));
        trail.save(folder.path().join("cursortrail.png")).unwrap();
        trail.save(folder.path().join("cursor.png")).unwrap();
        smooth_trail(folder.path(), "").unwrap();
        let image =
            decode_image(&fs::read(folder.path().join("cursortrail.png")).unwrap()).unwrap();
        assert_eq!(image.dimensions(), (1, 1));
        assert!(folder.path().join("cursormiddle.png").exists());
        let mut image = image;
        recolor(&mut image, [255, 0, 100], 0.1);
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 100, 128]);
    }
    #[test]
    fn instafade_uses_bundled_defaults_at_both_resolutions() {
        let dir = tempfile::tempdir().unwrap();
        let mut ini = SkinIni::new("test");
        for high in [true, false] {
            instafade(dir.path(), &mut ini, high).unwrap();
        }
        assert!(dir.path().join("default-9@2x.png").exists());
        assert!(
            ini.get("Fonts", "HitCircleOverlap")
                .unwrap()
                .parse::<u32>()
                .unwrap()
                > 0
        );
        let zero = decode_image(&fs::read(dir.path().join("default-0.png")).unwrap()).unwrap();
        assert!(zero.pixels().all(|p| p[3] == 0));
    }
}
