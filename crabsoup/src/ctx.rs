use crate::libs::{analyze, base, codec, date, digest, html, log, process, regex, string, sys};
use mlua::ffi::luau_setfflag;
use mlua::prelude::LuaFunction;
use mlua::serde::ser;
use mlua::{ChunkMode, Lua, LuaOptions, LuaSerdeExt, Result, StdLib, Table, Thread};
use serde::Serialize;
use std::sync::OnceLock;

const SHARED_TABLE_LOC: &str = "crabsoup-shared";

include!(concat!(env!("OUT_DIR"), "/luau_modules.rs"));

fn set_fflags() {
    static ONCE_LOCK: OnceLock<()> = OnceLock::new();
    ONCE_LOCK.get_or_init(|| unsafe {
        luau_setfflag(c"LuauAttributeSyntax".as_ptr(), 1);
        luau_setfflag(c"LuauNativeAttribute".as_ptr(), 1);
        luau_setfflag(c"LintRedundantNativeAttribute".as_ptr(), 1);
    });
}

pub struct CrabsoupLuaContext {
    lua: Lua,
}
impl CrabsoupLuaContext {
    pub fn new() -> Result<Self> {
        set_fflags();

        let lua = Lua::new_with(StdLib::ALL, LuaOptions::new())?;

        // setup operating environment
        {
            // Create the shared table
            let shared_table = lua.create_table()?;
            lua.set_named_registry_value(SHARED_TABLE_LOC, &shared_table)?;
            shared_table.set("analyze", analyze::create_analyze_table(&lua)?)?;
            shared_table.set("baselib", base::create_base_table(&lua)?)?;
            shared_table.set("codecs", codec::create_codec_table(&lua)?)?;
            shared_table.set("Date", date::create_date_table(&lua)?)?;
            shared_table.set("Digest", digest::create_digest_table(&lua)?)?;
            shared_table.set("HTML", html::create_html_table(&lua)?)?;
            shared_table.set("Log", log::create_log_table(&lua)?)?;
            shared_table.set("Process", process::create_process_table(&lua)?)?;
            shared_table.set("Regex", regex::create_regex_table(&lua)?)?;
            shared_table.set("String", string::create_string_table(&lua)?)?;
            shared_table.set("Sys", sys::create_sys_table(&lua)?)?;

            // Create the lua data table
            let sources = load_lua_sources();
            let sources_table = lua.create_table()?;
            for (&name, &source) in &sources {
                sources_table.set(name, lua.create_string(source)?)?;
            }
            sources_table.set_readonly(true);
            shared_table.set("sources", &sources_table)?;

            // Load initialization code.
            lua.load(*sources.get("init.luau").unwrap())
                .set_name("@<rt>/init.luau")
                .set_mode(ChunkMode::Binary)
                .call::<()>((&shared_table, &lua.globals()))?;
        }

        // finish initialization
        lua.sandbox(true)?;
        Ok(CrabsoupLuaContext { lua })
    }

    pub fn run_standalone(&self, code: &str, chunk_name: Option<&str>) -> Result<Thread> {
        let shared_table = self.lua.named_registry_value::<Table>(SHARED_TABLE_LOC)?;
        let thread = shared_table
            .get::<LuaFunction>("run_standalone")?
            .call::<Thread>((code, chunk_name))?;
        Ok(thread)
    }

    pub fn repl(&self) -> Result<()> {
        let shared_table = self.lua.named_registry_value::<Table>(SHARED_TABLE_LOC)?;
        shared_table
            .get::<LuaFunction>("run_repl_from_console")?
            .call::<()>(())?;
        Ok(())
    }

    pub fn repl_in_plugin_env(&self) -> Result<()> {
        let shared_table = self.lua.named_registry_value::<Table>(SHARED_TABLE_LOC)?;
        shared_table
            .get::<LuaFunction>("run_repl_from_console_plugin")?
            .call::<()>(())?;
        Ok(())
    }

    pub fn run_main(&self, args: impl Serialize) -> Result<()> {
        let mut options = ser::Options::new();
        options.serialize_none_to_null = false;
        options.serialize_unit_to_null = false;
        options.set_array_metatable = false;

        let value = self.lua.to_value_with(&args, options)?;
        let shared_table = self.lua.named_registry_value::<Table>(SHARED_TABLE_LOC)?;
        shared_table
            .get::<LuaFunction>("run_main")?
            .call::<()>(value)?;
        Ok(())
    }
}
