//! Times a library scan: cold, then with the cache from the first pass.
use skinweaver::library::Library;
use std::{sync::atomic::AtomicBool, time::Instant};

fn main() {
    let root = std::env::args().nth(1).expect("osu! folder");
    let library = Library::new(&root).unwrap();
    let cancel = AtomicBool::new(false);
    let start = Instant::now();
    let (skins, _, cache) = library.scan_cached(None, &cancel).unwrap();
    println!("cold scan: {} skins in {:?}", skins.len(), start.elapsed());
    let start = Instant::now();
    let (skins, _, _) = library.scan_cached(Some(&cache), &cancel).unwrap();
    println!(
        "cached scan: {} skins in {:?}",
        skins.len(),
        start.elapsed()
    );
    let start = Instant::now();
    let file = std::env::temp_dir().join("bench-cache.json");
    cache.save(&file).unwrap();
    let loaded = skinweaver::library::ScanCache::load(&file, &library.root).unwrap();
    let skins = loaded.skins();
    println!(
        "cache save+load: {} skins in {:?}",
        skins.len(),
        start.elapsed()
    );
}
