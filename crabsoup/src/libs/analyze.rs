use codespan_reporting::{
    diagnostic::{Diagnostic, Label},
    files::{Files, SimpleFiles},
    term,
    term::termcolor::{ColorChoice, StandardStream},
};
use crabsoup_mlua_analyze::{LuaAnalyzer, LuaAnalyzerBuilder};
use mlua::{
    prelude::{LuaFunction, LuaString},
    Error, Lua, Result, Table, UserData, UserDataFields, UserDataMethods, UserDataRef,
};
use tracing::{enabled, Level};

pub fn create_analyze_table(lua: &Lua) -> Result<Table<'_>> {
    let table = lua.create_table()?;

    table.raw_set(
        "create",
        lua.create_function(|lua, setup_func: LuaFunction| {
            let builder = LuaAnalyzerBuilder::new();
            let userdata = lua.create_userdata(AnalyzerSetup(builder))?;
            setup_func.call::<_, ()>(&userdata)?;
            let builder = userdata.take::<AnalyzerSetup>()?;
            Ok(Analyzer(builder.0.build()))
        })?,
    )?;
    table.raw_set(
        "check",
        lua.create_function(
            |_, (analyzer, name, sources): (UserDataRef<Analyzer>, LuaString, LuaString)| {
                let location = name.to_str()?;
                let location = location.strip_prefix("@").unwrap_or(location);

                let sources = sources.to_str()?;

                let mut files = SimpleFiles::new();
                let file_id = files.add(location, sources);

                let result = analyzer.0.check(&location, sources, false);

                let writer = StandardStream::stderr(ColorChoice::Always);
                let config = term::Config::default();
                for value in &result {
                    let enabled_warn = enabled!(Level::WARN);
                    let enabled_error = enabled!(Level::ERROR);

                    if (!enabled_warn && !value.is_error) || (!enabled_error && value.is_error) {
                        continue;
                    }

                    let diagnostic = if value.is_error {
                        Diagnostic::error()
                    } else {
                        Diagnostic::warning()
                    };

                    let start_idx = files
                        .line_range(file_id, value.location_start.line)
                        .map_err(Error::runtime)?
                        .start
                        + value.location_start.column;
                    let end_idx = files
                        .line_range(file_id, value.location_end.line)
                        .map_err(Error::runtime)?
                        .start
                        + value.location_end.column;

                    let diagnostic = diagnostic
                        .with_message(&value.message)
                        .with_labels(vec![Label::primary(file_id, start_idx..end_idx)]);

                    term::emit(&mut writer.lock(), &config, &files, &diagnostic)
                        .map_err(Error::runtime)?;
                }
                Ok(!result.iter().any(|x| x.is_error))
            },
        )?,
    )?;

    Ok(table)
}

struct AnalyzerSetup(LuaAnalyzerBuilder);
impl UserData for AnalyzerSetup {
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "AnalyzerSetup");
    }

    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut(
            "add_definitions",
            |_, this, (name, source): (LuaString, LuaString)| {
                this.0.add_definitions(name.to_str()?, source.to_str()?);
                Ok(())
            },
        );

        methods.add_method_mut(
            "set_deprecation",
            |_, this, (path, replacement): (LuaString, Option<LuaString>)| {
                let replacement = match &replacement {
                    None => None,
                    Some(s) => Some(s.to_str()?),
                };
                this.0.set_deprecation(path.to_str()?, replacement);
                Ok(())
            },
        );
    }
}

struct Analyzer(LuaAnalyzer);
impl UserData for Analyzer {
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "Analyzer");
    }
}
