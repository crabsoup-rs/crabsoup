use mlua::ffi::{
    luaL_checktype, luaL_sandbox, luaL_sandboxthread, lua_getfenv, lua_getmetatable, lua_gettop,
    lua_mainthread, lua_newthread, lua_pushglobaltable, lua_pushnil, lua_replace, lua_rotate,
    lua_setfenv, lua_setmetatable, lua_setsafeenv, lua_xmove, LUA_GLOBALSINDEX, LUA_TFUNCTION,
    LUA_TTABLE,
};
use mlua::prelude::{LuaString, LuaTable};
use mlua::{
    lua_State, ChunkMode, Compiler, Error, Lua, Result, Table, UserData, UserDataFields,
    UserDataMethods, UserDataRef,
};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::ops::Deref;

pub fn create_base_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;

    {
        let version = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));
        table.raw_set("_VERSION", version)?;
        table.raw_set("VERSION_ONLY", env!("CARGO_PKG_VERSION"))?;
    }

    table.raw_set(
        "open_rustyline",
        lua.create_function(|_, ()| {
            let editor = DefaultEditor::new().map_err(Error::runtime)?;
            Ok(RustyLineEditor { editor })
        })?,
    )?;

    table.raw_set(
        "loadstring_rt",
        lua.create_function(|lua, (code, chunkname): (LuaString, LuaString)| {
            Ok(lua
                .load(code.as_bytes().deref())
                .set_mode(ChunkMode::Binary)
                .set_name(chunkname.to_str()?.deref())
                .into_function()?)
        })?,
    )?;

    table.raw_set(
        "loadstring",
        lua.create_function(
            |lua, (code, chunkname, env): (LuaString, Option<LuaString>, Option<LuaTable>)| {
                let code = code.to_str()?;
                let mut chunk = lua.load(code.deref());
                if let Some(chunkname) = chunkname {
                    chunk = chunk.set_name(chunkname.to_str()?.deref());
                } else {
                    if code.chars().count() > 40 {
                        let mut str: String = code.chars().take(40).collect();
                        str.push_str("...");
                        chunk = chunk.set_name(&str);
                    } else {
                        chunk = chunk.set_name(code.deref());
                    };
                }
                if let Some(env) = env {
                    chunk = chunk.set_environment(env);
                }
                match chunk.into_function() {
                    Ok(func) => Ok((Some(func), None)),
                    Err(Error::SyntaxError { message, .. }) => Ok((None, Some(message))),
                    Err(e) => Ok((None, Some(e.to_string()))),
                }
            },
        )?,
    )?;

    table.raw_set("is_nan", lua.create_function(|_, f: f64| Ok(f.is_nan()))?)?;
    table.raw_set("is_inf", lua.create_function(|_, f: f64| Ok(f.is_infinite()))?)?;
    table.raw_set("is_finite", lua.create_function(|_, f: f64| Ok(f.is_finite()))?)?;

    table.raw_set(
        "plugin_fail",
        lua.create_function(|_, str: LuaString| {
            Ok(PluginInstruction::Fail(str.to_str()?.to_string()))
        })?,
    )?;
    table.raw_set(
        "plugin_exit",
        lua.create_function(|_, str: LuaString| {
            Ok(PluginInstruction::Exit(str.to_str()?.to_string()))
        })?,
    )?;

    table.raw_set(
        "raw_freeze",
        lua.create_function(|_, table: Table| {
            table.set_readonly(true);
            Ok(())
        })?,
    )?;

    table.raw_set(
        "create_opaque_environment",
        lua.create_function(|_, ()| Ok(OpaqueEnvironment(())))?,
    )?;
    table.raw_set(
        "compile_for_environment",
        lua.create_function(|_, (source, mutable_globals): (LuaString, Option<Table>)| {
            let mut mutable = Vec::new();
            if let Some(globals) = mutable_globals {
                for global in globals.sequence_values::<LuaString>() {
                    mutable.push(global?.to_string_lossy().to_string());
                }
            }

            let data = Compiler::new()
                .set_optimization_level(2)
                .set_type_info_level(1)
                .set_mutable_globals(mutable)
                .compile(source.as_bytes())?;
            Ok(CompiledChunk(data))
        })?,
    )?;
    table.raw_set(
        "load_precompiled_chunk",
        lua.create_function(|lua, (chunk, chunkname): (UserDataRef<CompiledChunk>, LuaString)| {
            Ok(lua
                .load(chunk.0.as_slice())
                .set_mode(ChunkMode::Binary)
                .set_name(chunkname.to_str()?.deref())
                .into_function()?)
        })?,
    )?;

    table.raw_set("opaque_key", OpaqueKey(()))?;

    load_unsafe_functions(lua, &table)?;

    Ok(table)
}

fn load_unsafe_functions(lua: &Lua, table: &Table) -> Result<()> {
    unsafe extern "C-unwind" fn load_in_new_thread(lua: *mut lua_State) -> i32 {
        unsafe {
            // signature: (loader, env) -> thread
            luaL_checktype(lua, 1, LUA_TFUNCTION);
            luaL_checktype(lua, 2, LUA_TTABLE);
            assert_eq!(lua_gettop(lua), 2);

            // Transfer all arguments to the main Lua thread.
            let lua_main = lua_mainthread(lua);
            let lua_thread = lua_newthread(lua_main);
            if lua == lua_main {
                lua_rotate(lua_main, -3, 1);
            }
            lua_xmove(lua, lua_thread, 2);

            // Creates the new thread
            lua_replace(lua_thread, LUA_GLOBALSINDEX);
            luaL_sandboxthread(lua_thread);
            // (loader function is left on stack here)

            // Pushes the newly created thread to the `lua` thread
            if lua != lua_main {
                lua_xmove(lua_main, lua, 1);
            }
            1
        }
    }

    unsafe extern "C-unwind" fn set_safeenv_flag(lua: *mut lua_State) -> i32 {
        unsafe {
            // signature: (env)
            luaL_checktype(lua, 1, LUA_TTABLE);
            lua_setsafeenv(lua, 1, 1);
            0
        }
    }

    unsafe extern "C-unwind" fn deoptimize_env(lua: *mut lua_State) -> i32 {
        unsafe {
            // signature: (env)
            luaL_checktype(lua, 1, LUA_TTABLE);
            lua_setsafeenv(lua, 1, 0);
            0
        }
    }

    unsafe extern "C-unwind" fn get_globals(lua: *mut lua_State) -> i32 {
        unsafe {
            lua_pushglobaltable(lua);
            1
        }
    }

    unsafe extern "C-unwind" fn set_globals(lua: *mut lua_State) -> i32 {
        unsafe {
            // signature: (env)
            luaL_checktype(lua, 1, LUA_TTABLE);
            lua_replace(lua, LUA_GLOBALSINDEX);
            0
        }
    }

    unsafe extern "C-unwind" fn raw_getfenv(lua: *mut lua_State) -> i32 {
        unsafe {
            // signature: (function) -> env
            luaL_checktype(lua, 1, LUA_TFUNCTION);
            lua_getfenv(lua, 1);
            1
        }
    }

    unsafe extern "C-unwind" fn raw_setfenv(lua: *mut lua_State) -> i32 {
        unsafe {
            // signature: (function, env)
            luaL_checktype(lua, 1, LUA_TFUNCTION);
            luaL_checktype(lua, 2, LUA_TTABLE);
            lua_setfenv(lua, 1);
            0
        }
    }

    unsafe extern "C-unwind" fn do_sandbox(lua: *mut lua_State) -> i32 {
        unsafe {
            luaL_sandbox(lua, 1);
            0
        }
    }

    unsafe extern "C-unwind" fn raw_getmetatable(lua: *mut lua_State) -> i32 {
        unsafe {
            // signature: (target) -> metatable
            if lua_gettop(lua) != 1 {
                panic!("wrong number of arguments");
            }
            if lua_getmetatable(lua, 1) != 0 {
                1
            } else {
                0
            }
        }
    }
    unsafe extern "C-unwind" fn raw_setmetatable(lua: *mut lua_State) -> i32 {
        unsafe {
            // signature: (target, metatable)
            let top = lua_gettop(lua);
            if top == 1 {
                lua_pushnil(lua);
            } else if top == 2 {
                // do nothing
            } else {
                panic!("wrong number of arguments");
            }
            lua_setmetatable(lua, 1);
            1
        }
    }

    unsafe {
        table.raw_set("load_in_new_thread", lua.create_c_function(load_in_new_thread)?)?;
        table.raw_set("set_safeenv_flag", lua.create_c_function(set_safeenv_flag)?)?;
        table.raw_set("deoptimize_env", lua.create_c_function(deoptimize_env)?)?;
        table.raw_set("get_globals", lua.create_c_function(get_globals)?)?;
        table.raw_set("set_globals", lua.create_c_function(set_globals)?)?;
        table.raw_set("raw_getfenv", lua.create_c_function(raw_getfenv)?)?;
        table.raw_set("raw_setfenv", lua.create_c_function(raw_setfenv)?)?;
        table.raw_set("do_sandbox", lua.create_c_function(do_sandbox)?)?;
        table.raw_set("raw_getmetatable", lua.create_c_function(raw_getmetatable)?)?;
        table.raw_set("raw_setmetatable", lua.create_c_function(raw_setmetatable)?)?;
    }

    Ok(())
}

enum PluginInstruction {
    Fail(String),
    Exit(String),
}
impl UserData for PluginInstruction {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "PluginInstruction");
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get_message", |lua, this, ()| match this {
            PluginInstruction::Fail(msg) => Ok(lua.create_string(msg)),
            PluginInstruction::Exit(msg) => Ok(lua.create_string(msg)),
        });

        methods.add_method("is_fail", |_, this, ()| match this {
            PluginInstruction::Fail(_) => Ok(true),
            _ => Ok(false),
        });
        methods.add_method("is_exit", |_, this, ()| match this {
            PluginInstruction::Exit(_) => Ok(true),
            _ => Ok(false),
        });
    }
}

struct RustyLineEditor {
    editor: DefaultEditor,
}
impl UserData for RustyLineEditor {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "RustyLineEditor");
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("readline", |_, this, prompt: LuaString| {
            match this.editor.readline(&prompt.to_str()?) {
                Ok(line) => Ok(Some(line)),
                Err(ReadlineError::Interrupted) => Ok(None),
                Err(ReadlineError::Eof) => Ok(None),
                Err(e) => Err(Error::runtime(e)),
            }
        });

        methods.add_method_mut("saveline", |_, this, line: LuaString| {
            this.editor
                .add_history_entry(line.to_str()?.deref())
                .map_err(Error::runtime)?;
            Ok(())
        });
    }
}

struct OpaqueEnvironment(());
impl UserData for OpaqueEnvironment {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "Environment");
    }
}

struct CompiledChunk(Vec<u8>);
impl UserData for CompiledChunk {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "CompiledChunk");
    }
}

pub struct OpaqueKey(());
impl UserData for OpaqueKey {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "OpaqueKey");
    }
}
