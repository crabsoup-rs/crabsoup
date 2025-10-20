--!strict

local module = {}

local ty_boolean = newproxy()
local ty_string = newproxy()
local ty_string_list = newproxy()
local ty_command = newproxy()
local ty_command_map = newproxy()

type SchemaType = typeof(ty_boolean) | typeof(ty_string) | typeof(ty_string_list)
type Schema = {
    __kind: nil | "string_map",
    __inner: Schema?,
    __allow_unknown: boolean?,
    [string]: Schema | SchemaType,
}

local schema: Schema = {
    settings = {
        soupault_version = ty_string,
        site_dir = ty_string,
        build_dir = ty_string,
        page_file_extensions = ty_string_list,
        clean_urls = ty_boolean,
        keep_extensions = ty_string_list,
        default_extension = ty_string,
        ignore_extensions = ty_string_list,
        generator_mode = ty_boolean,
        complete_page_selector = ty_string,
        default_template_file = ty_string,
        default_content_selector = ty_string_list,
        default_content_action = ty_string,
        keep_doctype = ty_boolean,
        doctype = ty_string,
        pretty_print_html = ty_boolean,
        plugin_discovery = ty_boolean,
        plugin_dirs = ty_string_list,
        page_character_encoding = ty_string,

        -- Deprecated
        verbose = ty_boolean,
        debug = ty_boolean,
        caching = ty_boolean,
        cache_dir = ty_string,
        strict = ty_boolean,
    },
    plugins = {
        __kind = "string_map",
        __inner = {
            file = ty_string,
            lua_source = ty_string,
        },
    },
    widgets = {
        __kind = "string_map",
        __inner = {
            widget = ty_string,
            after = ty_string_list,
            page = ty_string_list,
            section = ty_string_list,
            path_regex = ty_string_list,
            exclude_page = ty_string_list,
            exclude_section = ty_string_list,
            exclude_path_regex = ty_string_list,
            include_subsections = ty_boolean,
            -- TODO: Implement os_family
            __allow_unknown = true,
        },
    },
    hooks = {
        __kind = "string_map",
        __inner = {
            file = ty_string,
            lua_source = ty_string,
        },
    },
    templates = {
        __kind = "string_map",
        __inner = {
            file = ty_string,
            content_selector = ty_string_list,
            content_action = ty_string,
            page = ty_string_list,
            section = ty_string_list,
            path_regex = ty_string_list,
            exclude_page = ty_string_list,
            exclude_section = ty_string_list,
            exclude_path_regex = ty_string_list,
            include_subsections = ty_boolean,
        },
    },
    preprocessors = ty_command_map,
    __allow_unknown = true, -- custom options are pretty common in configs, for use by scripts
}

local function schema_error(): never
    error("Type error in configuration!", 2)
end

local function process_string_list(key_name, value): { string }
    if not value then
        return {}
    elseif type(value) == "string" then
        return { value }
    elseif type(value) == "table" then
        for k, v in value do
            if type(k) ~= "number" then
                Log.error(`'{key_name}' should be a list, but it is a dictionary.`)
                schema_error()
            end
            if type(v) ~= "string" then
                Log.error(`'{key_name}' should be a list of strings, not of {type(v)}.`)
                schema_error()
            end
        end

        return value
    else
        Log.error(`'{key_name}' should be a list, but it is a {type(value)}.`)
        return schema_error()
    end
end

local function process_command(key_name, value): { shell: string } | { string } | nil
    if not value then
        return nil
    elseif type(value) == "string" then
        return { shell = value }
    elseif type(value) == "table" then
        return process_string_list(key_name, value)
    else
        Log.error(`'{key_name}' should be a list, but it is a {type(value)}.`)
        return schema_error()
    end
end

local function process_command_map(key_name, value): { [string]: { shell: string } | { string } | nil }
    if not value then
        return {}
    elseif type(value) == "table" then
        for k, v in value do
            if type(k) ~= "string" then
                Log.error(`'{key_name}' should be a dictionary, but it is a list.`)
                schema_error()
            end
            value[k] = process_command(key_name, v)
        end
        return value
    else
        Log.error(`'{key_name}' should be a dictionary of commands, but it is a {type(value)}.`)
        return schema_error()
    end
end

local function check_schema(config_name, schema: Schema, table: any): any?
    if type(table) ~= "table" then
        Log.error("Main config root is not a table??")
        error("could not parse config")
    end

    if schema.__kind == "string_map" then
        if not table then
            return {}
        elseif type(table) == "table" then
            for k, v in table do
                if type(k) ~= "string" then
                    Log.error(`'{k}' should be a string, but it is a {type(k)}.`)
                    schema_error()
                end

                if not schema.__inner then
                    schema_error()
                else
                    check_schema(`{config_name}.{k}`, schema.__inner, v)
                end
            end
            return
        else
            Log.error(`'{config_name}' should be a table, but it is a {type(table)}.`)
            schema_error()
        end
    end

    for k, v in table do
        if type(k) ~= "string" then
            Log.warn(`Non-string key '{Value.repr(k)} = {Value.repr(v)}' found in configuration. It will be ignored.`)
            table[k] = nil
        else
            local key_name = if #config_name == 0 then k else `{config_name}.{k}`
            if schema[k] and not string.startswith(k, "__") then
                local k_schema = schema[k]
                if type(k_schema) == "table" then
                    if v and type(v) ~= "table" then
                        Log.error(`'{key_name}' should be a table, but it is a {type(v)}.`)
                        schema_error()
                    end
                    if not v then
                        table[k] = {}
                        v = table[k]
                    end

                    local replace = check_schema(key_name, k_schema, v)
                    if replace then
                        table[k] = replace
                    end
                elseif k_schema == ty_boolean then
                    if v and type(v) ~= "boolean" then
                        Log.warn(`'{key_name}' should be a boolean, but it is a {type(v)}. It will be treated as one.`)
                    end
                    table[k] = not not table[k]
                elseif k_schema == ty_string then
                    if v and type(v) ~= "string" then
                        Log.error(`'{key_name}' should be a string, but it is a {type(v)}.`)
                        schema_error()
                    end
                elseif k_schema == ty_string_list then
                    table[k] = process_string_list(key_name, v)
                elseif k_schema == ty_command then
                    table[k] = process_command(key_name, v)
                elseif k_schema == ty_command_map then
                    table[k] = process_command_map(key_name, v)
                else
                    error("unreachable?")
                end
            else
                if not schema.__allow_unknown then
                    Log.warn(`'{key_name}' is not a recognized configuration option. It will be ignored.`)
                end
                table[k] = nil
            end
        end
    end

    for k, v in schema do
        if not table[k] and not string.startswith(k, "__") then
            if type(v) == "table" then
                if v.__kind == "string_map" then
                    table[k] = {}
                else
                    table[k] = {}
                    check_schema(`{config_name}.{k}`, v, table[k])
                end
            elseif v == ty_string_list then
                table[k] = {}
            elseif v == ty_command_map then
                table[k] = {}
            end
        end
    end

    return
end

local function merge_config(config, default_config)
    if type(config) == "table" and type(default_config) == "table" then
        local new_table = {}
        for k, v in default_config do
            new_table[k] = v
        end
        for k, v in config do
            if new_table[k] ~= nil then
                new_table[k] = merge_config(v, new_table[k])
            else
                new_table[k] = v
            end
        end
        return new_table
    elseif config ~= nil then
        return config
    else
        return default_config
    end
end

local function check_deprecations(config)
    if config.settings.verbose then
        Log.warn("`settings.verbose` option is deprecated: Verbosity may only be set on the command line.")
    end
    if config.settings.debug then
        Log.warn("`settings.debug` option is deprecated: Verbosity may only be set on the command line.")
    end
    if config.settings.caching then
        Log.warn("`settings.caching` option is deprecated: Caching is not supported in crabsoup.")
    end
    for k, _ in config.preprocessors do
        if not table.find(config.settings.page_file_extensions, k) then
            Log.warn(`Preprocessor is defined for extension '.{k}', but it is not found in page_file_extensions.`)
        end
    end
    if not config.settings.strict then
        Log.warn("`settings.strict` option is deprecated: crabsoup does not support non-strict builds.")
    end
end

local default_config = TOML.from_string(require("resources")["app/crabsoup/default_config.toml"])
default_config.settings.strict = true -- deprecated option
check_schema("", schema, default_config)

local function freeze_recursive(tbl: any)
    table.freeze(tbl)
    for _, v in tbl do
        if type(v) == "table" then
            freeze_recursive(v)
        end
    end
    return tbl
end

local function parse_configs(config_source)
    local config = TOML.from_string(config_source)
    check_schema("", schema, config)
    local merged = merge_config(config, default_config)
    check_deprecations(merged)
    local clone = Table.deep_clone(merged)
    freeze_recursive(clone)
    return clone
end

function module.load_configuration(config_source)
    return {
        parsed = parse_configs(config_source),
        raw = freeze_recursive(TOML.from_string(config_source)),
    }
end

return module
