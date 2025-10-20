--!strict

local _, _G = ...

-- Helper functions
local function create_unsafe_from_string<T>(func: (string) -> T): (string) -> T?
    return function(str)
        local status, result = pcall(func, str)
        if status then
            return result
        else
            return nil
        end
    end
end

-- Function definitions for `String` module
local String = _G.String or {}
_G.String = String

function String.is_valid_utf8(str: string): boolean
    if utf8.len(str) then
        return true
    else
        return false
    end
end

function String.length(str: string): number
    local len = utf8.len(str)
    return if len then len else #str
end

function String.length_ascii(str: string): number
    return #str
end

String.trim = string.trim

-- TODO: String.to_integer
String.to_number = tonumber
String.to_float = tonumber

function String.join(sep: string, list: { string }): string
    return table.concat(list, sep)
end

String.ends_with = string.endswith
String.starts_with = string.startswith

-- Function definitions for 'JSON' module
local JSON = _G.JSON or {}
_G.JSON = JSON

JSON.unsafe_from_string = create_unsafe_from_string(JSON.from_string)

-- Function definitions for 'TOML' module
local TOML = _G.TOML or {}
_G.TOML = TOML

TOML.unsafe_from_string = create_unsafe_from_string(TOML.from_string)

-- Function definitions for 'YAML' module
local YAML = _G.YAML or {}
_G.YAML = YAML

YAML.unsafe_from_string = create_unsafe_from_string(YAML.from_string)

-- Function definitions for 'YAML' module
local CSV = _G.CSV or {}
_G.CSV = CSV

CSV.unsafe_from_string = create_unsafe_from_string(CSV.from_string)

-- Function definitions for `Table` module
local Table = _G.Table or {}
_G.Table = Table

function Table.has_key(table, key)
    if table[key] == nil then
        return nil
    else
        return true
    end
end

function Table.copy<K, V>(value: { [K]: V }): { [K]: V }
    local new = {}
    for k, v in value do
        new[k] = value[k]
    end
    return new
end

function Table.sort<T>(compare: (T, T) -> number, value: { T })
    table.sort(value, function(a, b)
        return compare(a, b) < 0
    end)
end

-- Function definitions for 'Sys' module
local Sys = _G.Sys or {}
_G.Sys = Sys

Sys.get_file_modification_date = Sys.get_file_modification_time

function Sys.run_program(program)
    return Process.run({ shell = program })
end

function Sys.run_program_get_exit_code(program)
    return Process.try_run({ shell = program })
end

function Sys.get_program_output(program)
    return Process.run_output({ shell = program })
end

function Sys.random(n)
    return math.random(0, n)
end

Sys.strip_extensions = Sys.strip_all_extensions

Sys.basename_unix = Sys.basename
Sys.dirname_unix = Sys.dirname
Sys.join_path_unix = Sys.join_path
Sys.split_path_unix = Sys.split_path

Sys.basename_url = Sys.basename
Sys.dirname_url = Sys.dirname
Sys.join_url = Sys.join_path
Sys.split_path_url = Sys.split_path

-- Function definitions for 'HTML' module
local function join_selectors(selectors: any?): string
    if type(selectors) == "table" then
        return table.concat(selectors, ",")
    else
        error("selector list must be a table")
    end
end

function _G.HTML.select_any_of(node, list)
    return HTML.select_one(node, join_selectors(list))
end
function _G.HTML.select_all_of(node, list)
    return HTML.select(node, join_selectors(list))
end
function _G.HTML.matches_selector(_doc, node, selector)
    return HTML.matches(node, selector)
end
function _G.HTML.matches_any_of_selectors(_doc, node, list)
    return HTML.matches(node, join_selectors(list))
end

_G.HTML.append_root = _G.HTML.append_child
_G.HTML.prepend_root = _G.HTML.prepend_child
_G.HTML.replace = _G.HTML.replace_element
_G.HTML.clone_content = _G.HTML.clone
_G.HTML.clone_document = _G.HTML.clone

-- Function definitions for `Value` module
local Value = _G.Value or {}
_G.Value = Value

function Value.is_nil<T>(value: T): boolean
    return value == nil
end

function Value.is_int<T>(value: T): boolean
    if type(value) == "number" then
        return math.isfinite(value) and math.round(value) == value
    else
        return false
    end
end

function Value.is_float<T>(value: T): boolean
    return type(value) == "number"
end

function Value.is_string<T>(value: T): boolean
    return type(value) == "string"
end

function Value.is_table<T>(value: T): boolean
    return type(value) == "table"
end

function Value.is_list<T>(value: T): boolean
    if not value or type(value) ~= "table" then
        return false
    else
        for k in value do
            if type(k) ~= "number" then
                return false
            end
        end
        return true
    end
end

function Value.is_html<T>(value: T): boolean
    return typeof(value) == "NodeRef"
end
