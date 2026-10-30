//! Times the file-heavy operations on a synthetic library:
//! `cargo run --release --example bench_mix -- [skins] [extra files per skin]`.
//! Every skin holds the default skin's file names (tiny PNG/WAV bodies), 60-frame
//! animations, a fonts folder referenced from skin.ini and some unrelated extra files.
use skinweaver::{
    catalog::{Target, catalog},
    ini::SkinIni,
    library::Library,
    mixer::{self, Selection, Source, TRANSPARENT},
    modify::Modifications,
};
use std::{fs, path::Path, sync::atomic::AtomicBool, time::Instant};

fn make_skin(dir: &Path, n: usize, extra: usize) {
    fs::create_dir_all(dir.join("fonts")).unwrap();
    for entry in fs::read_dir("assets/defaultskin").unwrap() {
        let name = entry.unwrap().file_name();
        fs::write(dir.join(&name), TRANSPARENT).unwrap();
    }
    for stem in [
        "menu-back",
        "play-skip",
        "scorebar-colour",
        "followpoint",
        "sliderb",
        "pippidonidle",
        "hit300k",
    ] {
        for i in 0..60 {
            for scale in ["", "@2x"] {
                fs::write(dir.join(format!("{stem}-{i}{scale}.png")), TRANSPARENT).unwrap();
            }
        }
    }
    for d in 0..10 {
        fs::write(dir.join(format!("fonts/score-{d}@2x.png")), TRANSPARENT).unwrap();
        fs::write(dir.join(format!("fonts/combo-{d}@2x.png")), TRANSPARENT).unwrap();
    }
    for i in 0..extra {
        fs::create_dir_all(dir.join(format!("extra/{}", i % 10))).unwrap();
        fs::write(
            dir.join(format!("extra/{}/file{i}.png", i % 10)),
            TRANSPARENT,
        )
        .unwrap();
    }
    fs::write(
        dir.join("skin.ini"),
        format!("[General]\nName: skin{n}\nAuthor: a{n}\n\n[Colours]\nCombo1: 1,2,3\n\n[Fonts]\nScorePrefix: fonts/score\nComboPrefix: fonts/combo\n"),
    )
    .unwrap();
}

fn main() {
    let skins: usize = std::env::args().nth(1).map_or(40, |s| s.parse().unwrap());
    let extra: usize = std::env::args().nth(2).map_or(300, |s| s.parse().unwrap());
    let root = tempfile::tempdir().unwrap();
    for n in 0..skins {
        make_skin(&root.path().join(format!("Skins/skin{n:03}")), n, extra);
    }
    let library = Library::new(root.path()).unwrap();
    let cancel = AtomicBool::new(false);
    let quiet = |_: f32, _: &str| {};

    let start = Instant::now();
    let (all, _, _) = library.scan_cached(None, &cancel).unwrap();
    println!("scan: {} skins in {:?}", all.len(), start.elapsed());

    // Every leaf of the catalog, sources spread across all skins.
    let nodes = catalog();
    let leaves: Vec<Target> = nodes
        .iter()
        .flat_map(|n| n.leaves())
        .filter_map(|n| n.target.clone())
        .collect();
    let selections: Vec<Selection> = leaves
        .iter()
        .enumerate()
        .map(|(i, target)| Selection {
            target: target.clone(),
            source: Source::Skin(all[i % all.len()].path.clone()),
        })
        .collect();
    let runs = 5;
    let best = |f: &mut dyn FnMut(usize)| {
        (0..runs)
            .map(|i| {
                let start = Instant::now();
                f(i);
                start.elapsed()
            })
            .min()
            .unwrap()
    };
    let t = best(&mut |i| {
        mixer::create_mix(
            &library,
            &format!("bench mix {i}"),
            &selections,
            &cancel,
            &quiet,
        )
        .unwrap();
    });
    println!(
        "create_mix: {} selections, best of {runs}: {t:?}",
        selections.len()
    );
    let fp = |name: &str| {
        skinweaver::library::fingerprint(&root.path().join("Skins").join(name)).unwrap()
    };
    println!("  fingerprint {}", fp("bench mix 0"));

    let base = all[0].path.clone();
    let t = best(&mut |i| {
        mixer::create_from_skin(
            &library,
            &base,
            &format!("bench copy {i}"),
            &selections,
            &cancel,
            &quiet,
        )
        .unwrap();
    });
    println!("create_from_skin: best of {runs}: {t:?}");
    println!("  fingerprint {}", fp("bench copy 0"));

    let flags = Modifications {
        static_interface: true,
        ..Modifications::default()
    };
    let mut total = std::time::Duration::ZERO;
    let mut fastest = std::time::Duration::MAX;
    for run in 0..runs {
        let work = tempfile::tempdir().unwrap();
        make_skin(work.path(), 0, extra);
        let mut ini = SkinIni::new("x");
        let start = Instant::now();
        flags.apply(work.path(), &mut ini, &cancel).unwrap();
        total += start.elapsed();
        if run == 0 {
            println!(
                "  fingerprint {}",
                skinweaver::library::fingerprint(work.path()).unwrap()
            );
        }
        fastest = fastest.min(start.elapsed());
    }
    println!(
        "static interface on one skin: best {fastest:?}, mean {:?}",
        total / runs as u32
    );

    // What a scene does per frame: find the source of ~150 drawn stems among all selections.
    let stems: Vec<String> = fs::read_dir("assets/defaultskin")
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().to_string_lossy().into_owned();
            name.strip_suffix(".png")
                .map(|s| format!("{}.png", s.trim_end_matches("@2x")))
        })
        .collect();
    let start = Instant::now();
    let mut found = 0;
    for _ in 0..30 {
        for stem in &stems {
            found += selections
                .iter()
                .position(|s| s.target.matches(Path::new(stem)))
                .is_some() as usize;
        }
    }
    println!(
        "scene lookups: {} stems x {} selections, {:?} per frame",
        stems.len(),
        selections.len(),
        start.elapsed() / 30
    );
    let _ = found;
}
