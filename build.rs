use std::{env, fs, path::PathBuf};
fn main() {
    #[cfg(windows)]
    if env::var_os("CARGO_FEATURE_APP").is_some() {
        println!("cargo:rerun-if-changed=assets/branding/elmapa.rc");
        println!("cargo:rerun-if-changed=assets/branding/elmapa.ico");
        embed_resource::compile_for(
            "assets/branding/elmapa.rc",
            ["elmapa"],
            embed_resource::ParamsIncludeDirs(["assets/branding"]),
        )
        .manifest_required()
        .expect("embed the Elmapa executable icon");
    }
    println!("cargo:rerun-if-changed=assets/icons");
    let mut code = String::from("fn embedded_icon(path:&str)->Option<&'static [u8]>{match path{\n");
    if let Ok(entries) = fs::read_dir("assets/icons") {
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "svg"))
            .collect();
        files.sort();
        for path in files {
            let name = path.file_name().unwrap().to_str().unwrap();
            let full = fs::canonicalize(&path).unwrap();
            code.push_str(&format!("{:?}=>Some(include_bytes!({:?})),\n", name, full));
        }
    }
    code.push_str("_=>None}}\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("icons.rs"),
        code,
    )
    .unwrap();
    println!("cargo:rerun-if-changed=assets/landmarks/packs");
    let mut code = String::from("pub(crate) const EMBEDDED_LANDMARK_FILES: &[(&str, &str)] = &[\n");
    fn embed(directory: &std::path::Path, code: &mut String) {
        let mut files: Vec<_> = fs::read_dir(directory)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        files.sort();
        for path in files {
            if path.is_dir() {
                embed(&path, code);
            } else if path.extension().is_some_and(|e| e == "json" || e == "svg") {
                let name = path
                    .strip_prefix("assets/landmarks/packs")
                    .unwrap()
                    .iter()
                    .map(|component| component.to_str().expect("landmark filename must be UTF-8"))
                    .collect::<Vec<_>>()
                    .join("/");
                code.push_str(&format!(
                    "({:?}, include_str!({:?})),\n",
                    name,
                    fs::canonicalize(&path).unwrap()
                ));
            }
        }
    }
    embed(std::path::Path::new("assets/landmarks/packs"), &mut code);
    code.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("landmark-packs.rs"),
        code,
    )
    .unwrap();
    embed_brush_textures();
}

fn embed_brush_textures() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("package path"));
    let assets = root.join("assets/brush-materials");
    let manifest = assets.join("manifest.json");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let data: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest).expect("brush manifest")).expect("brush JSON");
    let mut textures = String::from("&[\n");
    for entry in data["materials"].as_array().expect("brush materials") {
        if let Some(filename) = entry["texture"].as_str() {
            assert!(
                !filename.is_empty() && !filename.contains(['/', '\\']),
                "brush textures must be files beside the manifest"
            );
            let file = assets.join(filename);
            println!("cargo:rerun-if-changed={}", file.display());
            textures.push_str(&format!("    Some(include_bytes!({file:?})),\n"));
        } else {
            textures.push_str("    None,\n");
        }
    }
    textures.push(']');
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("build output"));
    fs::write(output.join("brush_textures.rs"), textures).expect("embed brush textures");
}
