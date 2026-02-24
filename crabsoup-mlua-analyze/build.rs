use std::fs;
use std::path::Path;

fn main() {
    let source_dir_base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let analysis_dir = source_dir_base.join("luau").join("Analysis");
    let ast_dir = source_dir_base.join("luau").join("Ast");
    let common_dir = source_dir_base.join("luau").join("Common");
    let config_dir = source_dir_base.join("luau").join("Config");

    cc::Build::new()
        .cpp(true)
        .warnings(false)
        .flag_if_supported("-std=c++17")
        .add_files_by_ext(&source_dir_base.join("bindings"), "cpp")
        .include(&analysis_dir.join("include"))
        .include(&ast_dir.join("include"))
        .include(&common_dir.join("include"))
        .include(&config_dir.join("include"))
        .compile("mlua-analyze-bindings");

    println!("cargo:rerun-if-changed=bindings");
    println!("cargo:rerun-if-changed=luau");
}

trait AddFilesByExt {
    fn add_files_by_ext(&mut self, dir: &Path, ext: &str) -> &mut Self;
}

impl AddFilesByExt for cc::Build {
    fn add_files_by_ext(&mut self, dir: &Path, ext: &str) -> &mut Self {
        for entry in fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension() == Some(ext.as_ref()))
        {
            self.file(entry.path());
        }
        self
    }
}
