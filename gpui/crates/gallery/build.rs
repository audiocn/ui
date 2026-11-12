//! Embeds every font in `assets/fonts` so demos can use them by family name.
use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/fonts");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut fonts: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("ttf" | "otf")))
        .collect();
    fonts.sort();
    let list = fonts
        .iter()
        .map(|p| {
            format!(
                "    include_bytes!({:?}).as_slice(),\n",
                p.display().to_string()
            )
        })
        .collect::<String>();
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("fonts.rs");
    fs::write(out, format!("pub static FONTS: &[&[u8]] = &[\n{list}];\n")).unwrap();
}
