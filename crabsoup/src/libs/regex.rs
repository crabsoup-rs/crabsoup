use mlua::{prelude::LuaString, Error, Lua, Result, Table};

pub fn create_regex_table(lua: &Lua) -> Result<Table<'_>> {
    let table = lua.create_table()?;

    table.raw_set(
        "match",
        lua.create_function(|_, (string, regex): (LuaString, LuaString)| {
            Ok(regex::Regex::new(regex.to_str()?)
                .map_err(Error::runtime)?
                .is_match(string.to_str()?))
        })?,
    )?;
    table.raw_set(
        "find_all",
        lua.create_function(|lua, (string, regex): (LuaString, LuaString)| {
            let table = lua.create_table()?;
            for m in regex::Regex::new(regex.to_str()?)
                .map_err(Error::runtime)?
                .find_iter(string.to_str()?)
            {
                table.raw_push(m.as_str())?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "replace",
        lua.create_function(
            |lua, (string, regex, replacement): (LuaString, LuaString, LuaString)| {
                Ok(lua.create_string(
                    regex::Regex::new(regex.to_str()?)
                        .map_err(Error::runtime)?
                        .replace(string.to_str()?, replacement.to_str()?)
                        .as_bytes(),
                )?)
            },
        )?,
    )?;
    table.raw_set(
        "replace_all",
        lua.create_function(
            |lua, (string, regex, replacement): (LuaString, LuaString, LuaString)| {
                Ok(lua.create_string(
                    regex::Regex::new(regex.to_str()?)
                        .map_err(Error::runtime)?
                        .replace_all(string.to_str()?, replacement.to_str()?)
                        .as_bytes(),
                )?)
            },
        )?,
    )?;
    table.raw_set(
        "split",
        lua.create_function(|lua, (string, regex): (LuaString, LuaString)| {
            let table = lua.create_table()?;
            for m in regex::Regex::new(regex.to_str()?)
                .map_err(Error::runtime)?
                .split(string.to_str()?)
            {
                table.raw_push(m)?;
            }
            Ok(table)
        })?,
    )?;
    table.raw_set(
        "escape",
        lua.create_function(|_, str: LuaString| Ok(regex::escape(str.to_str()?)))?,
    )?;

    Ok(table)
}
