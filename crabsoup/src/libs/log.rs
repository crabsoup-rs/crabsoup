use mlua::{Lua, MultiValue, Result, Table, Value};
use std::borrow::Cow;
use tracing::{debug, enabled, error, info, trace, warn, Level};

pub fn create_log_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;

    fn target(lua: &Lua) -> Result<Cow<'static, str>> {
        if let Some(source) = lua.inspect_stack(1, |debug| {
            if let Some(source) = debug.source().source {
                if source.starts_with("@") {
                    let source = source.strip_prefix("@").unwrap_or(&source);
                    let source = source.strip_suffix(".lua").unwrap_or(&source);
                    let source = source.strip_suffix(".luau").unwrap_or(&source);
                    let source = source.replace(".lua#", "#");
                    let source = source.replace(".luau#", "#");
                    Ok(source.into())
                } else {
                    Ok("<loadstring>".into())
                }
            } else {
                Ok(module_path!().into())
            }
        }) {
            source
        } else {
            Ok(module_path!().into())
        }
    }
    fn value_to_str(value: &MultiValue) -> Result<String> {
        let mut values = String::new();
        for value in value.iter() {
            if !values.is_empty() {
                values.push('\t');
            }
            values.push_str(&value.to_string()?);
        }
        Ok(values)
    }

    macro_rules! create_log_function {
        ($name:literal, $target:ident, $level:expr) => {
            table.raw_set(
                $name,
                lua.create_function(|lua, value: MultiValue| {
                    let target_str = target(lua)?;
                    $target!(target: "Lua", "{target_str}: {}", value_to_str(&value)?);
                    Ok(())
                })?,
            )?;
            table.raw_set(concat!($name, "_enabled"), enabled!(target: "Lua", $level))?;
        };
    }

    create_log_function!("error", error, Level::ERROR);
    create_log_function!("warn", warn, Level::WARN);
    create_log_function!("info", info, Level::INFO);
    create_log_function!("debug", debug, Level::DEBUG);
    create_log_function!("trace", trace, Level::TRACE);

    table.raw_set("warning", table.raw_get::<Value>("warn")?)?;

    Ok(table)
}
