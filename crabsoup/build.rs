use anyhow::Result;
use crabsoup_mlua_analyze::LuaAnalyzerBuilder;
use mlua::{ffi::luau_setfflag, Compiler};
use std::path::PathBuf;

fn compile_script(input: &[u8], has_require: bool) -> Vec<u8> {
    let mut compiler = Compiler::new()
        .set_optimization_level(2)
        .set_type_info_level(1);
    if has_require {
        compiler =
            compiler.set_mutable_globals(vec!["require".to_string(), "require_env".to_string()]);
    }
    compiler.compile(input).unwrap()
}

pub fn main() -> Result<()> {
    // TODO: Set this in a far cleaner way
    unsafe {
        luau_setfflag(c"LuauAttributeSyntax".as_ptr(), 1);
        luau_setfflag(c"LuauNativeAttribute".as_ptr(), 1);
        luau_setfflag(c"LintRedundantNativeAttribute".as_ptr(), 1);
    }

    // do the actual compilation
    let mut out_path = PathBuf::from(std::env::var("OUT_DIR")?);
    out_path.push("luau_compiled");
    std::fs::create_dir_all(&out_path)?;

    let mut all_paths = Vec::new();

    let mut analyze = LuaAnalyzerBuilder::new();
    analyze.add_definitions("defs_shared.d_lua", include_str!("lua/defs/defs_shared.d_lua"));
    analyze
        .add_definitions("defs_standalone.d_lua", include_str!("lua/defs/defs_standalone.d_lua"));
    let analyze = analyze.build();
    for path in glob::glob("lua/**/*")? {
        let path = path?;
        let file_name = path.file_name().unwrap().to_string_lossy();
        let suffix = path.strip_prefix("lua/")?;
        let str_path = path
            .to_string_lossy()
            .strip_prefix("lua/")
            .unwrap()
            .to_string();
        if path.is_file() && (file_name.ends_with(".lua") || file_name.ends_with(".luau")) {
            let mut output = out_path.clone();
            output.push(suffix);

            let script = std::fs::read_to_string(&path)?;
            let name = path.file_name().unwrap().to_string_lossy();
            let result = analyze.check(&name, &script, false);

            if result.iter().any(|x| x.is_error) {
                println!("{result:#?}");
                panic!("Error in script: {name}");
            }

            let mut parent = output.clone();
            parent.pop();
            std::fs::create_dir_all(parent)?;
            std::fs::write(
                &output,
                compile_script(script.as_bytes(), !suffix.starts_with("app/")),
            )?;

            all_paths.push((str_path, output.to_string_lossy().to_string()));
        } else if path.is_file() {
            all_paths.push((str_path, path.canonicalize()?.to_string_lossy().to_string()));
        }
        println!("cargo::rerun-if-changed={}", path.display());
    }

    out_path.pop();
    out_path.push("luau_modules.rs");

    let mut push_lines = Vec::new();
    for (k, v) in all_paths {
        push_lines.push(format!(r#"map.insert({k:?}, include_bytes!({v:?}).as_slice());"#));
    }
    let push_lines = push_lines.join("\n");
    std::fs::write(
        out_path,
        format!(
            r#"
                fn load_lua_sources() -> crate::wyhash::WyHashMap<&'static str, &'static [u8]> {{
                    let mut map = crate::wyhash::WyHashMap::default();
                    {push_lines}
                    map
                }}
            "#
        ),
    )?;

    Ok(())
}
