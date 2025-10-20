--!strict

local shared, _G = ...
local baselib = shared.baselib

-- Function definitions for `String` module
local String = _G.String or {}
_G.String = String

function String.truncate(str: string, length: number, trailer: string?): string
    local len = utf8.len(str)
    if len then
        if len == length then
            return str
        else
            local offset = utf8.offset(str, length)
            if offset >= #str then
                return str
            else
                return string.sub(str, 0, offset) .. (trailer or "")
            end
        end
    elseif length >= #str then
        return str
    else
        return string.sub(str, 0, length) .. (trailer or "")
    end
end

-- Function definitions for `Plugin` module
local Plugin = _G.Plugin or {}
_G.Plugin = Plugin

function Plugin.fail<T>(message: T): never
    coroutine.yield(baselib.plugin_fail(tostring(message)))
    return error("Coroutine resumed despite Plugin.fail?")
end

function Plugin.exit<T>(message: T): never
    coroutine.yield(baselib.plugin_exit(tostring(message)))
    return error("Coroutine resumed despite Plugin.exit?")
end

local supported_soupault_version = "5.0.0" -- Newest *compatible* version of Soupault.
local crabsoup_version = shared.baselib.VERSION_ONLY

function Plugin.require_version(version: string)
    local min_components = string.split(version, ".")
    local cur_components = string.split(supported_soupault_version, ".")

    for i = 1, math.max(#min_components, #cur_components) do
        local min = tonumber(min_components[i] or "0") or error("could not parse number")
        local cur = tonumber(cur_components[i] or "0") or error("could not parse number")
        if cur > min then
            return
        end
        if min > cur then
            error(
                `Plugin requires Soupault v{version} compatibility, but crabsoup v{crabsoup_version} is compatible with Soupault v{supported_soupault_version}.`,
                2
            )
        end
    end
end

function Plugin.soupault_version(): string
    return supported_soupault_version
end

function Plugin.crabsoup_version(): string
    return crabsoup_version
end

-- Function definitions for `Date` module
local Date = _G.Date or {}
_G.Date = Date

function _G.Date.now_format(fmt, tz)
    return Date.format(Date.now_timestamp(), fmt, tz)
end
function _G.Date.reformat(date, inputs, fmt, tz)
    return Date.format(Date.to_timestamp(date, inputs, tz), fmt)
end

-- Function definitions for `Process` module
local Process = _G.Process or {}
_G.Process = Process

function Process.wait_on_yield(value: Process): CompletedProcess
    Process.is_completed(value) -- ignore value, this just checks several useful things
    local result = coroutine.yield(value)
    if typeof(result) ~= "CompletedProcess" then
        error("`coroutine.yield` did not return CompletedProcess!?")
    end
    return result
end
