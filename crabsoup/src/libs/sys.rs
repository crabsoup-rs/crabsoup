use crate::paths::{basename, dirname, lstr_to_path, lstr_to_system_path, system_path_to_lstr};
use mlua::{prelude::LuaString, Error, Lua, Result, Table, Value};
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use typed_path::Utf8UnixComponent;

const MICROS: f64 = 1000000.0;

fn time_to_num(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).unwrap().as_secs()
}

pub fn create_sys_table(lua: &Lua) -> Result<Table<'_>> {
    let table = lua.create_table()?;

    table.raw_set(
        "read_file",
        lua.create_function(|lua, path: LuaString| {
            Ok(lua.create_string(std::fs::read(lstr_to_system_path(path)?)?)?)
        })?,
    )?;
    table.raw_set(
        "write_file",
        lua.create_function(|_, (path, data): (LuaString, LuaString)| {
            std::fs::write(lstr_to_system_path(path)?, data.as_bytes())?;
            Ok(())
        })?,
    )?;
    table.raw_set(
        "delete_file",
        lua.create_function(|_, path: LuaString| {
            std::fs::remove_file(lstr_to_system_path(path)?)?;
            Ok(())
        })?,
    )?;
    table.raw_set(
        "copy_file",
        lua.create_function(|_, (src, dst): (LuaString, LuaString)| {
            std::fs::copy(lstr_to_system_path(src)?, lstr_to_system_path(dst)?)?;
            Ok(())
        })?,
    )?;
    table.raw_set(
        "delete_recursive",
        lua.create_function(|_, path: LuaString| {
            let path = lstr_to_system_path(path)?;
            if AsRef::<Path>::as_ref(&path).is_dir() {
                std::fs::remove_dir_all(&path)?;
            } else {
                std::fs::remove_file(&path)?;
            }
            Ok(())
        })?,
    )?;
    table.raw_set(
        "get_file_size",
        lua.create_function(|_, path: LuaString| {
            Ok(std::fs::metadata(lstr_to_system_path(path)?)?.len())
        })?,
    )?;
    table.raw_set(
        "file_exists",
        lua.create_function(|_, path: LuaString| Ok(std::fs::exists(lstr_to_system_path(path)?)?))?,
    )?;
    table.raw_set(
        "is_file",
        lua.create_function(|_, path: LuaString| Ok(lstr_to_system_path(path)?.is_file()))?,
    )?;
    table.raw_set(
        "is_dir",
        lua.create_function(|_, path: LuaString| Ok(lstr_to_system_path(path)?.is_dir()))?,
    )?;
    table.raw_set(
        "get_file_creation_time",
        lua.create_function(|_, path: LuaString| {
            Ok(time_to_num(std::fs::metadata(lstr_to_system_path(path)?)?.created()?))
        })?,
    )?;
    table.raw_set(
        "get_file_modification_time",
        lua.create_function(|_, path: LuaString| {
            Ok(time_to_num(std::fs::metadata(lstr_to_system_path(path)?)?.modified()?))
        })?,
    )?;
    table.raw_set(
        "mkdir",
        lua.create_function(|_, path: LuaString| {
            Ok(std::fs::create_dir_all(lstr_to_system_path(path)?)?)
        })?,
    )?;
    table.raw_set(
        "list_dir",
        lua.create_function(|lua, path: LuaString| {
            let table = lua.create_table()?;
            for dir in std::fs::read_dir(lstr_to_system_path(path)?)? {
                let dir = dir?;
                table.raw_push(dir.file_name().to_string_lossy())?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "glob",
        lua.create_function(|lua, glob: LuaString| {
            let table = lua.create_table()?;
            for result in glob::glob(glob.to_str()?).map_err(Error::runtime)? {
                table.raw_push(system_path_to_lstr(lua, &result.map_err(Error::runtime)?)?)?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "get_extension",
        lua.create_function(|lua, path: LuaString| match lstr_to_path(&path)?.extension() {
            Some(x) => Ok(Some(lua.create_string(x)?)),
            None => Ok(None),
        })?,
    )?;
    table.raw_set(
        "strip_extension",
        lua.create_function(|lua, path: LuaString| {
            let path = lstr_to_path(&path)?;
            if let Some(stem) = path.file_stem() {
                let mut result = path.to_owned();
                result.set_file_name(stem);
                lua.create_string(result.as_str())
            } else {
                Err(Error::runtime("Path must not end in a relative path component"))
            }
        })?,
    )?;
    table.raw_set(
        "get_extensions",
        lua.create_function(|lua, path: LuaString| {
            let name = basename(lstr_to_path(&path)?)?;
            let table = lua.create_table()?;
            for str in name.split('.').skip(1) {
                table.raw_push(str)?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "has_extension",
        lua.create_function(|_, (path, extension): (LuaString, LuaString)| {
            let name = basename(lstr_to_path(&path)?)?;
            let extension = extension.to_str()?;
            let mut found = false;
            for str in name.split('.').skip(1) {
                if str == extension {
                    found = true;
                    break;
                }
            }
            Ok(found)
        })?,
    )?;
    table.raw_set(
        "strip_all_extensions",
        lua.create_function(|lua, path: LuaString| {
            let path = lstr_to_path(&path)?;
            let name = basename(&path)?;
            let stripped = name.split('.').next().unwrap();

            let mut result = path.to_owned();
            result.set_file_name(stripped);
            lua.create_string(result.as_str())
        })?,
    )?;
    table.raw_set(
        "basename",
        lua.create_function(|lua, path: LuaString| {
            lua.create_string(basename(lstr_to_path(&path)?)?)
        })?,
    )?;
    table.raw_set(
        "dirname",
        lua.create_function(|lua, path: LuaString| {
            lua.create_string(dirname(lstr_to_path(&path)?)?)
        })?,
    )?;
    table.raw_set(
        "join_path",
        lua.create_function(|lua, (path_a, path_b): (LuaString, LuaString)| {
            let mut path_a = lstr_to_path(&path_a)?.to_path_buf();
            for component in lstr_to_path(&path_b)?.components() {
                match component {
                    Utf8UnixComponent::RootDir | Utf8UnixComponent::CurDir => {}
                    Utf8UnixComponent::ParentDir => {
                        if !path_a.pop() {
                            path_a.push("..");
                        }
                    }
                    Utf8UnixComponent::Normal(v) => path_a.push(v),
                }
            }
            lua.create_string(path_a.as_str())
        })?,
    )?;
    table.raw_set(
        "split_path",
        lua.create_function(|lua, path: LuaString| {
            let table = lua.create_table()?;
            for component in lstr_to_path(&path)?.iter() {
                table.raw_push(component)?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set("is_unix", lua.create_function(|_, ()| Ok(cfg!(unix)))?)?;
    table.raw_set("is_windows", lua.create_function(|_, ()| Ok(cfg!(windows)))?)?;
    table.raw_set(
        "getenv",
        lua.create_function(|lua, (env, default): (LuaString, Value)| {
            match std::env::var(env.to_str()?) {
                Ok(val) => Ok(Value::String(lua.create_string(val)?)),
                Err(_) => Ok(default),
            }
        })?,
    )?;
    table.raw_set(
        "sleep",
        lua.create_function(|_, secs: f64| {
            std::thread::sleep(Duration::from_micros((secs * MICROS).round() as u64));
            Ok(())
        })?,
    )?;

    table.raw_set("cpu_count", num_cpus::get())?;

    Ok(table)
}
