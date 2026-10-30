#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ui;

use anyhow::{Context, Result, bail};
use skinweaver::{
    ini::SkinIni,
    modify::default_bytes,
    settings::{self, Language, Settings},
};
use std::{fs, path::PathBuf};

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "Skinweaver\n  --demo       Open a temporary sample library\n  --lang ru|en  Interface language\n  --osu-dir DIR Open an osu! folder containing Skins"
        );
        return Ok(());
    }
    let demo = args.iter().any(|arg| arg == "--demo");
    let demo_dir = if demo {
        Some(tempfile::tempdir()?)
    } else {
        None
    };
    let data = demo_dir
        .as_ref()
        .map(|d| d.path().join("settings"))
        .unwrap_or_else(settings::data_dir);
    let lock = settings::instance_lock(&data)?;
    let (mut settings, initial_error) = match Settings::load(&data) {
        Ok(settings) => (settings, None),
        Err(error) => (Settings::default(), Some(format!("{error:#}"))),
    };
    for (i, arg) in args.iter().enumerate() {
        match arg.as_str() {
            "--lang" => {
                settings.language = match args.get(i + 1).map(String::as_str) {
                    Some("ru") => Language::Ru,
                    Some("en") => Language::En,
                    _ => bail!("Use --lang ru or --lang en"),
                }
            }
            "--osu-dir" => {
                settings.osu_folder =
                    PathBuf::from(args.get(i + 1).context("--osu-dir requires a path")?)
            }
            _ => {}
        }
    }
    if let Some(dir) = &demo_dir {
        settings.osu_folder = dir.path().join("osu!");
        create_demo(&settings.osu_folder)?;
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1000.0, 700.0])
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!(
                "../assets/icon.png"
            ))?),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "Skinweaver",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ui::App::new(
                cc,
                data,
                settings,
                lock,
                demo_dir,
                initial_error,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

fn create_demo(root: &std::path::Path) -> Result<()> {
    for (i, (name, colors)) in [
        (
            "Afterglow",
            [[239, 151, 190], [146, 215, 191], [154, 182, 239]],
        ),
        (
            "Mint condition",
            [[146, 215, 191], [233, 220, 150], [151, 193, 237]],
        ),
        (
            "Violet hour",
            [[186, 161, 229], [239, 170, 199], [167, 215, 220]],
        ),
        (
            "Paper planes",
            [[234, 221, 199], [160, 191, 209], [213, 160, 150]],
        ),
        (
            "Pink noise",
            [[241, 133, 178], [255, 203, 153], [182, 166, 233]],
        ),
        (
            "Blue Monday",
            [[146, 185, 238], [157, 224, 214], [222, 177, 227]],
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let folder = root
            .join(if i == 5 { "HiddenSkins" } else { "Skins" })
            .join(name);
        fs::create_dir_all(&folder)?;
        let mut ini = SkinIni::new(name);
        ini.set("General", "Author", "Synthetic demo");
        for (n, color) in colors.iter().enumerate() {
            ini.set(
                "Colours",
                &format!("Combo{}", n + 1),
                format!("{},{},{}", color[0], color[1], color[2]),
            );
        }
        fs::write(folder.join("skin.ini"), ini.to_string())?;
        // Each demo skin recolours a few default elements so previews differ visibly.
        let accent = colors[(i + 1) % 3];
        for name in [
            "hitcircle@2x.png",
            "hitcircleoverlay@2x.png",
            "approachcircle@2x.png",
            "cursor@2x.png",
            "cursortrail@2x.png",
            "hit300@2x.png",
            "hit100@2x.png",
            "default-1@2x.png",
            "default-2@2x.png",
            "default-3@2x.png",
        ] {
            let Some(bytes) = default_bytes(name) else {
                continue;
            };
            let tint =
                !name.starts_with("hitcircle@") && (i % 2 == 0 || name.starts_with("cursor"));
            if tint {
                let mut image = skinweaver::modify::decode_image(bytes)?;
                for pixel in image.pixels_mut() {
                    for c in 0..3 {
                        pixel[c] = ((u16::from(pixel[c]) * accent[c] as u16) / 255) as u8;
                    }
                }
                image.save(folder.join(name))?;
            } else {
                fs::write(folder.join(name), bytes)?;
            }
        }
        for name in [
            "normal-hitnormal.wav",
            "soft-hitnormal.wav",
            "drum-hitnormal.wav",
        ] {
            if let Some(bytes) = default_bytes(name) {
                fs::write(folder.join(name), bytes)?;
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        rfd::MessageDialog::new()
            .set_title("Skinweaver")
            .set_description(format!("{error:#}"))
            .set_level(rfd::MessageLevel::Error)
            .show();
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_is_an_isolated_readable_library() {
        let dir = tempfile::tempdir().unwrap();
        create_demo(dir.path()).unwrap();
        let library = skinweaver::library::Library::new(dir.path()).unwrap();
        let (skins, warnings) = library
            .scan(&std::sync::atomic::AtomicBool::new(false))
            .unwrap();
        assert_eq!(skins.len(), 6);
        assert!(warnings.is_empty());
        assert_eq!(skins.iter().filter(|s| s.hidden).count(), 1);
    }
}
