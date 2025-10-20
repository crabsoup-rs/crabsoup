use csv::ReaderBuilder;
use mlua::{prelude::LuaString, serde::ser, Error, Lua, LuaSerdeExt, Result, Table, Value};
use std::io::Cursor;

fn options() -> ser::Options {
    let mut opts = ser::Options::new();
    opts.set_array_metatable = false;
    opts.serialize_none_to_null = false;
    opts.serialize_unit_to_null = false;
    opts.detect_serde_json_arbitrary_precision = true;
    opts
}

fn create_json_table(lua: &Lua) -> Result<Table<'_>> {
    let table = lua.create_table()?;

    table.raw_set(
        "from_string",
        lua.create_function(|lua, str: LuaString| {
            let value: serde_json::Value =
                serde_json::from_slice(str.as_bytes()).map_err(Error::runtime)?;
            lua.to_value_with(&value, options())
        })?,
    )?;
    table.raw_set(
        "to_string",
        lua.create_function(|_, value: Value| {
            serde_json::to_string(&value).map_err(Error::runtime)
        })?,
    )?;
    table.raw_set(
        "pretty_print",
        lua.create_function(|_, value: Value| {
            serde_json::to_string_pretty(&value).map_err(Error::runtime)
        })?,
    )?;

    Ok(table)
}

fn create_toml_table(lua: &Lua) -> Result<Table<'_>> {
    let table = lua.create_table()?;

    table.raw_set(
        "from_string",
        lua.create_function(|lua, str: LuaString| {
            let value: toml::Value = toml::from_str(str.to_str()?).map_err(Error::runtime)?;
            lua.to_value_with(&value, options())
        })?,
    )?;
    table.raw_set(
        "to_string",
        lua.create_function(|_, value: Value| toml::to_string(&value).map_err(Error::runtime))?,
    )?;

    Ok(table)
}

fn create_yaml_table(lua: &Lua) -> Result<Table<'_>> {
    let table = lua.create_table()?;

    table.raw_set(
        "from_string",
        lua.create_function(|lua, str: LuaString| {
            let value: serde_yaml::Value =
                serde_yaml::from_slice(str.as_bytes()).map_err(Error::runtime)?;
            lua.to_value_with(&value, options())
        })?,
    )?;
    table.raw_set(
        "to_string",
        lua.create_function(|_, value: Value| {
            serde_yaml::to_string(&value).map_err(Error::runtime)
        })?,
    )?;

    Ok(table)
}

fn create_csv_table(lua: &Lua) -> Result<Table<'_>> {
    let table = lua.create_table()?;

    table.raw_set(
        "from_string",
        lua.create_function(|lua, str: LuaString| {
            let bytes = str.as_bytes();
            let mut csv = ReaderBuilder::new()
                .has_headers(false)
                .double_quote(true)
                .flexible(true)
                .from_reader(Cursor::new(bytes));

            let outer = lua.create_table()?;
            for record in csv.records() {
                let record = record.map_err(Error::runtime)?;
                let inner = lua.create_table()?;
                for field in record.iter() {
                    inner.push(field)?;
                }
                outer.push(inner)?;
            }
            Ok(outer)
        })?,
    )?;
    table.raw_set(
        "to_list_of_tables",
        lua.create_function(|lua, str: LuaString| {
            let bytes = str.as_bytes();
            let mut csv = ReaderBuilder::new()
                .double_quote(true)
                .from_reader(Cursor::new(bytes));

            let mut names = Vec::new();
            for header in csv.headers().map_err(Error::runtime)? {
                names.push(header.to_string());
            }

            let outer = lua.create_table()?;
            for record in csv.records() {
                let record = record.map_err(Error::runtime)?;
                let inner = lua.create_table()?;
                for (i, field) in record.iter().enumerate() {
                    inner.set(names[i].as_str(), field)?;
                }
                outer.push(inner)?;
            }
            Ok(outer)
        })?,
    )?;

    Ok(table)
}

pub fn create_codec_table(lua: &Lua) -> Result<Table<'_>> {
    let table = lua.create_table()?;
    table.raw_set("JSON", create_json_table(lua)?)?;
    table.raw_set("TOML", create_toml_table(lua)?)?;
    table.raw_set("YAML", create_yaml_table(lua)?)?;
    table.raw_set("CSV", create_csv_table(lua)?)?;
    Ok(table)
}
