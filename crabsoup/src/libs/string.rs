use crate::wyhash::WyHashSet;
use base64::Engine;
use minijinja::Environment;
use mlua::{prelude::LuaString, Error, Lua, Result, Table, Value};
use regex::Regex;
use std::{ops::Deref, sync::LazyLock};

pub fn create_string_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;

    table.raw_set(
        "slugify",
        lua.create_function(|lua, str: LuaString| {
            static REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("[\\pZ_]+").unwrap());
            lua.create_string(&*REGEX.replace_all(&str.to_str()?, "_").trim_matches('_'))
        })?,
    )?;
    table.raw_set(
        "slugify_soft",
        lua.create_function(|lua, str: LuaString| {
            static REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("\\pS+").unwrap());
            lua.create_string(&*REGEX.replace_all(&str.to_str()?, "-"))
        })?,
    )?;
    table.raw_set(
        "slugify_ascii",
        lua.create_function(|lua, str: LuaString| {
            static REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("[^a-zA-Z0-9]+").unwrap());
            lua.create_string(&*REGEX.replace_all(&str.to_str()?, "-"))
        })?,
    )?;
    table.raw_set(
        "render_template",
        lua.create_function(|lua, (template_str, lua_env): (LuaString, Value)| {
            let mut env = Environment::new();
            let template = template_str.to_str()?;
            env.add_template("template", template.deref())
                .map_err(Error::runtime)?;
            let tmpl = env.get_template("template").map_err(Error::runtime)?;
            Ok(lua.create_string(tmpl.render(lua_env).map_err(Error::runtime)?)?)
        })?,
    )?;
    table.raw_set(
        "base64_encode",
        lua.create_function(|lua, str: LuaString| {
            lua.create_string(base64::engine::general_purpose::STANDARD.encode(str.as_bytes()))
        })?,
    )?;
    table.raw_set(
        "base64_decode",
        lua.create_function(|lua, str: LuaString| {
            lua.create_string(
                base64::engine::general_purpose::STANDARD
                    .decode(str.as_bytes())
                    .map_err(Error::runtime)?,
            )
        })?,
    )?;
    table.raw_set(
        "url_encode",
        lua.create_function(|lua, str: LuaString| {
            lua.create_string(&*urlencoding::encode(&str.to_str()?))
        })?,
    )?;
    table.raw_set(
        "url_decode",
        lua.create_function(|lua, str: LuaString| {
            lua.create_string(&*urlencoding::decode(&str.to_str()?).map_err(Error::runtime)?)
        })?,
    )?;
    table.raw_set(
        "html_encode",
        lua.create_function(|lua, html: LuaString| {
            Ok(lua.create_string(html_escape::encode_text(&html.to_str()?).deref())?)
        })?,
    )?;
    table.raw_set(
        "html_decode",
        lua.create_function(|lua, html: LuaString| {
            Ok(lua.create_string(html_escape::decode_html_entities(&html.to_str()?).deref())?)
        })?,
    )?;

    const CSS_SPECIAL_CHARS: &str = "!\"#$%&'()*+,-./:;<=>?@[\\]^`{|}~";
    let mut css_chars = WyHashSet::default();
    css_chars.extend(CSS_SPECIAL_CHARS.chars());
    table.raw_set(
        "escape_css",
        lua.create_function(move |_, html: LuaString| {
            let mut str = String::new();
            let mut first = true;
            for ch in html.to_str()?.chars() {
                if (first && ch.is_numeric()) || ch.is_control() {
                    str.push_str(&format!("\\{:06x}", ch as u32));
                } else {
                    if css_chars.contains(&ch) {
                        str.push('\\');
                    }
                    str.push(ch);
                }
                first = false;
            }
            Ok(str)
        })?,
    )?;

    Ok(table)
}
