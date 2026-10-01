use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-changed=apps/web/dist");

    let dist = Path::new("apps/web/dist");
    if !dist.join("index.html").is_file() {
        panic!(
            "apps/web/dist is missing; build the shared UI first with npm in apps/web, \
             or run bash scripts/build-binary.sh"
        );
    }

    let mut files = Vec::new();
    collect_files(dist, dist, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));

    for (_, path) in &files {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let mut generated =
        String::from("pub(crate) static EMBEDDED_WEB_ASSETS: &[EmbeddedAsset] = &[\\n");

    for (relative, path) in files {
        let absolute = fs::canonicalize(&path)
            .unwrap_or_else(|error| panic!("canonicalize {}: {error}", path.display()));
        generated.push_str(&format!(
            "    EmbeddedAsset {{ path: {:?}, bytes: include_bytes!({:?}) }},\\n",
            relative,
            absolute.to_string_lossy()
        ));
    }

    generated.push_str("];\\n");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    fs::write(out_dir.join("embedded_web_assets.rs"), generated)
        .expect("write embedded web asset table");
}

fn collect_files(root: &Path, current: &Path, output: &mut Vec<(String, PathBuf)>) {
    for entry in
        fs::read_dir(current).unwrap_or_else(|error| panic!("read {}: {error}", current.display()))
    {
        let entry = entry.expect("read web asset directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, output);
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .expect("web asset is below dist")
            .to_string_lossy()
            .replace('\\', "/");
        output.push((relative, path));
    }
}
