local shared = ...

local module = {}
local sources = shared.sources

-- build table for standalone ctx
do
    local function common_deps(analyzer)
        analyzer:add_definitions("defs_shared", sources["defs/defs_shared.d_lua"])
        analyzer:add_definitions("defs_standalone", sources["defs/defs_standalone.d_lua"])
    end

    module.standalone = {}
    module.standalone.require_ctx = shared.analyze.create(function(analyzer)
        common_deps(analyzer)
    end)
    module.standalone.plugin_ctx = shared.analyze.create(function(analyzer)
        common_deps(analyzer)
        analyzer:add_definitions("defs_plugin_globals", sources["defs/defs_plugin_globals.d_lua"])
    end)
end

-- build table for plugin ctx
do
    local function common_deps(analyzer)
        analyzer:add_definitions("defs_shared", sources["defs/defs_shared.d_lua"])
        analyzer:add_definitions("defs_plugin", sources["defs/defs_plugin.d_lua"])
    end
    local function deprecations(analyzer)
        -- CSV library
        analyzer:set_deprecation("CSV.unsafe_from_string", "pcall(CSV.from_string, x)")

        -- HTML library
        analyzer:set_deprecation("HTML.select_any_of", 'HTML.select_one(e, "h1,h2")')
        analyzer:set_deprecation("HTML.select_all_of", 'HTML.select(e, "h1,h2")')
        analyzer:set_deprecation("HTML.matches_selector", "HTML.matches")
        analyzer:set_deprecation("HTML.matches_any_of_selectors", 'HTML.matches(e, "h1,h2")')
        analyzer:set_deprecation("HTML.append_root", "HTML.append")
        analyzer:set_deprecation("HTML.prepend_root", "HTML.prepend")
        analyzer:set_deprecation("HTML.replace", "HTML.replace_element")
        analyzer:set_deprecation("HTML.clone_content", "HTML.clone")
        analyzer:set_deprecation("HTML.clone_document", "HTML.clone")

        -- JSON library
        analyzer:set_deprecation("JSON.unsafe_from_string", "pcall(JSON.from_string, x)")

        -- String library
        analyzer:set_deprecation("String.is_valid_utf8", "utf8.len(x) ~= nil")
        analyzer:set_deprecation("String.length", "utf8.len")
        analyzer:set_deprecation("String.length_ascii", "#str")
        analyzer:set_deprecation("String.trim", "string.trim")
        analyzer:set_deprecation("String.to_number", "tonumber")
        analyzer:set_deprecation("String.to_float", "tonumber")
        analyzer:set_deprecation("String.join", "table.concat")
        analyzer:set_deprecation("String.starts_with", "string.starts_with")
        analyzer:set_deprecation("String.ends_with", "string.ends_with")

        -- Sys library
        analyzer:set_deprecation("Sys.get_file_modification_date", "Sys.get_file_modification_time")
        analyzer:set_deprecation("Sys.run_program", "Process.run")
        analyzer:set_deprecation("Sys.run_program_get_exit_code", "Process.try_run")
        analyzer:set_deprecation("Sys.get_program_output", "Process.run_output")
        analyzer:set_deprecation("Sys.random", "math.random")
        analyzer:set_deprecation("Sys.strip_extensions", "Sys.strip_all_extensions")
        analyzer:set_deprecation("Sys.basename_unix", "Sys.basename")
        analyzer:set_deprecation("Sys.dirname_unix", "Sys.dirname")
        analyzer:set_deprecation("Sys.join_path_unix", "Sys.join_path")
        analyzer:set_deprecation("Sys.split_path_unix", "Sys.split_path")
        analyzer:set_deprecation("Sys.basename_url", "Sys.basename")
        analyzer:set_deprecation("Sys.dirname_url", "Sys.dirname")
        analyzer:set_deprecation("Sys.join_url", "Sys.join_path")
        analyzer:set_deprecation("Sys.split_path_url", "Sys.split_path")

        -- Table library
        analyzer:set_deprecation("Table.has_key", "table[x]")
        analyzer:set_deprecation("Table.copy", "table.clone")
        analyzer:set_deprecation("Table.sort", "table.sort")

        -- TOML library
        analyzer:set_deprecation("TOML.unsafe_from_string", "pcall(TOML.from_string, x)")

        -- Value library
        analyzer:set_deprecation("Value.is_nil", "x == nil")
        analyzer:set_deprecation("Value.is_float", 'type(x) == "number"')
        analyzer:set_deprecation("Value.is_string", 'type(x) == "string"')
        analyzer:set_deprecation("Value.is_table", 'type(x) == "table"')
        analyzer:set_deprecation("Value.is_html", 'typeof(x) == "NodeRef"')

        -- YAML library
        analyzer:set_deprecation("YAML.unsafe_from_string", "pcall(YAML.from_string, x)")

        -- Globals
        analyzer:set_deprecation("dostring", "loadstring")
        analyzer:set_deprecation("nextvar")
        analyzer:set_deprecation("setglobal")
        analyzer:set_deprecation("getglobal")
        analyzer:set_deprecation("setfallback")
        analyzer:set_deprecation("mod", "x % y")
        analyzer:set_deprecation("size", "#t")
        analyzer:set_deprecation("strfind", "string.find")
        analyzer:set_deprecation("strlen", "string.len")
        analyzer:set_deprecation("strsub", "string.sub")
        analyzer:set_deprecation("strlower", "string.lower")
        analyzer:set_deprecation("strupper", "string.upper")
        analyzer:set_deprecation("strrep", "string.rep")
        analyzer:set_deprecation("ascii", "string.byte")
        analyzer:set_deprecation("format", "string.format")
        analyzer:set_deprecation("gsub", "string.gsub")
        analyzer:set_deprecation("abs", "math.abs")
        analyzer:set_deprecation("acos", "math.acos")
        analyzer:set_deprecation("asin", "math.asin")
        analyzer:set_deprecation("atan", "math.atan")
        analyzer:set_deprecation("atan2", "math.atan2")
        analyzer:set_deprecation("ceil", "math.ceil")
        analyzer:set_deprecation("cos", "math.cos")
        analyzer:set_deprecation("floor", "math.floor")
        analyzer:set_deprecation("log", "math.log")
        analyzer:set_deprecation("log10", "math.log10")
        analyzer:set_deprecation("max", "math.max")
        analyzer:set_deprecation("min", "math.min")
        analyzer:set_deprecation("sin", "math.sin")
        analyzer:set_deprecation("sqrt", "math.sqrt")
        analyzer:set_deprecation("tan", "math.tan")
        analyzer:set_deprecation("random", "math.random")
        analyzer:set_deprecation("randomseed", "math.randomseed")

        -- Plugin globals
        analyzer:set_deprecation("force")
        analyzer:set_deprecation("hook_config", "config")
    end

    module.plugin = {}
    module.plugin.require_ctx = shared.analyze.create(function(analyzer)
        common_deps(analyzer)
        deprecations(analyzer)
    end)
    module.plugin.plugin_ctx = shared.analyze.create(function(analyzer)
        common_deps(analyzer)
        analyzer:add_definitions("defs_plugin_globals", sources["defs/defs_plugin_globals.d_lua"])
        deprecations(analyzer)
    end)
end

return module
