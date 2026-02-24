use std::{fs, path::Path};

fn main() {
    let source_dir_base = Path::new(env!("CARGO_MANIFEST_DIR"));
    let analysis_dir = source_dir_base.join("luau").join("Analysis");
    let ast_dir = source_dir_base.join("luau").join("Ast");
    let common_dir = source_dir_base.join("luau").join("Common");
    let compiler_dir = source_dir_base.join("luau").join("Compiler");
    let config_dir = source_dir_base.join("luau").join("Config");
    let vm_dir = source_dir_base.join("luau").join("VM");

    cc::Build::new()
        .cpp(true)
        .warnings(false)
        .flag_if_supported("-std=c++17")
        .define("LUA_API", "extern \"C\"")
        .define("LUACODE_API", "extern \"C\"")
        .define("LUACODEGEN_API", "extern \"C\"")
        .add_files_by_ext(&analysis_dir.join("src"), "cpp")
        .add_files_by_ext(&config_dir.join("src"), "cpp")
        .include(&analysis_dir.join("include"))
        .include(&ast_dir.join("include"))
        .include(&common_dir.join("include"))
        .include(&compiler_dir.join("include"))
        .include(&config_dir.join("include"))
        .include(&vm_dir.join("include"))
        .compile("luauanalysis");
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
