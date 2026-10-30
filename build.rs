fn main() {
    use std::{fmt::Write, fs, path::PathBuf};
    println!("cargo:rerun-if-changed=assets/defaultskin");
    let root =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/defaultskin");
    let mut source =
        String::from("pub fn default_bytes(name: &str) -> Option<&'static [u8]> { match name {\n");
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() && !path.extension().is_some_and(|e| e == "import") {
            writeln!(
                source,
                "{:?} => Some(include_bytes!({:?})),",
                path.file_name().unwrap().to_str().unwrap(),
                path.to_str().unwrap()
            )
            .unwrap();
        }
    }
    source.push_str("_ => None, } }\n");
    fs::write(
        PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("defaults.rs"),
        source,
    )
    .unwrap();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/icon.ico")
            .set("ProductName", "Skinweaver")
            .set("FileDescription", "Skinweaver — skin mixer for osu!")
            .compile()
            .expect("Windows icon resource");
    }
}
