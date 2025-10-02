--!strict
--!native

local template = require("crabsoup.template")
local utils = require("crabsoup.utils")
local widget = require("crabsoup.widget")

local module = {}

local function build_globals(config, source_path, target_path): any
    local relative_prefix = config.settings.site_dir
    while string.endswith(relative_prefix, "/") do
        relative_prefix = string.sub(relative_prefix, 1, #relative_prefix - 1)
    end
    relative_prefix ..= "/"

    assert(string.startswith(source_path, relative_prefix), "Internal error: could not build relative_page_file")

    local relative_page_file = string.sub(source_path, #relative_prefix + 1)
    local page_url

    local name = Sys.basename(relative_page_file)
    if name == "index.html" or name == "index.htm" then
        page_url = Sys.dirname(relative_page_file)
    else
        if config.settings.clean_urls then
            page_url = Sys.strip_extension(relative_page_file)
        else
            page_url = relative_page_file
        end
    end

    return {
        page = nil, -- Set by process_page
        page_file = source_path,
        target_dir = `{Sys.dirname(target_path)}/`,
        target_file = target_path,
        nav_path = Sys.split_path(Sys.dirname(page_url)),
        page_url = `/{string.gsub(page_url, "\\", "/")}`,
        config = nil, -- Set by widget module
        soupault_config = config.raw.raw,
        site_dir = config.settings.site_dir,
        build_dir = config.settings.build_dir,
        persistent_data = {}, -- Set by widget module
        global_data = config.global_data,

        -- Index-related globals
        site_index = {}, -- TODO
        index_entry = nil, -- TODO
        soupault_pass = 0, -- TODO

        -- Crabsoup extensions
        relative_page_file = relative_page_file,
        global_config = config.raw.raw,
        parsed_config = config.raw.parsed,
        widget_name = nil, -- Set by widget module
    }
end

local function has_doctype(elem)
    return Table.for_any(HTML.is_doctype, HTML.children(elem))
end
local function strip_doctype(elem)
    for _, v in HTML.children(elem) do
        if HTML.is_doctype(v) then
            HTML.delete(v)
        end
    end
end

local warned_extensions = {}
function module.process_page(config, source_path, target_path)
    local extension = Sys.get_extension(source_path)
    local processor = config.raw.parsed.preprocessors[extension]

    -- Create globals
    local globals = build_globals(config, source_path, target_path)

    -- Load and preprocess page
    local page
    if not processor then
        if not warned_extensions[extension] and extension ~= "htm" and extension ~= "html" then
            warned_extensions[extension] = true
            Log.warn(`Extension '.{extension}' has no preprocessor defined. It will be treated as a HTML document.`)
        end
        page = HTML.parse(Sys.read_file(source_path))
    else
        local raw_cmd = table.clone(processor) :: any
        if raw_cmd.shell then
            raw_cmd.shell = `{raw_cmd.shell} "{source_path}"`
            if Log.trace_enabled then
                Log.trace(`Run preprocessor: {raw_cmd.shell}`)
            end
        else
            table.insert(raw_cmd, source_path)
            if Log.trace_enabled then
                Log.trace(`Run preprocessor: {Value.repr_compact(raw_cmd)}`)
            end
        end
        raw_cmd.capture_stdout = true
        utils.env_from_globals(raw_cmd, globals)

        local command = Process.spawn(raw_cmd)
        local finished_command = Process.wait_on_yield(command)
        Process.check_status(finished_command)
        page = HTML.parse(Process.get_stdout(finished_command))
    end
    globals.page = page

    -- Apply template
    template.apply_template(config.templates, globals)

    -- Apply plugins
    widget.run_widgets(config.widget_list, globals)

    -- Fix doctype
    if not config.settings.keep_doctype then
        strip_doctype(globals.page)
    end
    if not has_doctype(globals.page) then
        HTML.prepend(globals.page, HTML.clone(config.doctype))
    end

    -- Return and format the page
    if config.settings.pretty_print_html then
        return HTML.pretty_print(globals.page)
    else
        return HTML.to_string(globals.page)
    end
end

return module
