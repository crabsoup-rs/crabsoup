--
-- ilua.lua
--
-- A more friendly Lua interactive prompt
-- doesn't need '='
-- will print out tables recursively
--
-- Steve Donovan, 2007
-- Chris Hixon, 2010
-- Alissa Rao, 2024
--

-- imported global functions
local builtin_funcs = ...

local format = string.format
local concat = table.concat
local print = print
local loadstring = loadstring
local select = select
local setfenv = setfenv
local setmetatable = setmetatable
local pairs = pairs
local pcall = pcall
local error = error

-- imported global vars
local _VERSION = _VERSION

-- variables from soupault API
local trim = builtin_funcs.trim
local Pretty = builtin_funcs.Pretty

-- readline support
local readline, saveline
do
    local rustyline_editor = builtin_funcs.crabsoup.open_rustyline()
    function readline(prompt)
        return rustyline_editor:readline(prompt)
    end
    function saveline(line)
        return rustyline_editor:saveline(line)
    end
end

--
-- Ilua class
--

local Ilua = {}

-- defaults
Ilua.defaults = {
    -- evaluation related
    prompt = '>> ',         -- normal prompt
    prompt2 = '.. ',        -- prompt during multiple line input
    chunkname = "stdin",    -- name of the evaluated chunk when compiled
    result_var = "_",       -- the variable name that stores the last results
    verbose = false,        -- currently unused

    -- internal, for reference only
    savef = nil,
    num_prec = nil,
    num_all = nil,
}

function Ilua:new(params)
    local obj = {}
    params = params or {}
    setmetatable(obj, self)
    self.__index = self
    obj:init(params)
    return obj
end

function Ilua:init(params)
    for k, v in pairs(self.defaults) do
        self[k] = v
    end
    for k, v in pairs(params) do
        self[k] = v
    end

    -- setup environment
    if not self.env then
        self.env = self.env or table.clone(_G)
        self.env._G = self.env
    end

    -- setup pretty print objects
    local oh = function(str)
        if str and str ~= "" then print(str) end
    end
    self.p = Pretty:new { output_handler = oh }
end

-- this is mostly meant for the ilua launcher/main
-- a separate Ilua instance may need to do something different so wouldn't call this
function Ilua:start()
    print('ILUA: ' .. _VERSION .. ' + ' .. builtin_funcs.crabsoup._VERSION)
end

function Ilua:precision(len,prec,all)
    if not len then self.num_prec = nil
    else
        self.num_prec = '%'..len..'.'..prec..'f'
    end
    self.num_all = all
end

function Ilua:get_input()
    local lines, i, input, chunk, err = {}, 1
    while true do
        input = readline((i == 1) and self.prompt or self.prompt2)
        if not input then return end
        lines[i] = input
        input = concat(lines, "\n")
        chunk, err = loadstring(format("return(%s)", input), self.chunkname)
        if chunk then return input end
        chunk, err = loadstring(input, self.chunkname)
        if chunk or not err:match("<eof>$") then
            return input
        end
        lines[1] = input
        i = 2
    end
end

function Ilua:wrap(...)
    self.p(...)
    self.env[self.result_var] = select(1, ...)
end

function Ilua:eval_lua(line)
    if self.savef then
        self.savef:write(self.prompt, line, '\n')
    end
    -- is it an expression?
    local chunk, err = loadstring(format("(...):wrap((function() return %s end)())", line), self.chunkname)
    if err then -- otherwise, a statement?
        chunk, err = loadstring(format("(...):wrap((function() %s end)())", line), self.chunkname)
    end
    if err then
        print(err)
        return
    end
    -- compiled ok, evaluate the chunk
    setfenv(chunk, self.env)
    local ok, res = pcall(chunk, self)
    if not ok then
        print(res)
    end
end

function Ilua:run()
    while true do
        local input = self:get_input()
        if not input or trim(input) == 'quit' then break end
        self:eval_lua(input)
        saveline(input)
    end

    if self.savef then
        self.savef:close()
    end
end

--
-- Special functions
--

local is_repl_running = false
local function run_repl(params)
    if is_repl_running then
        error("Please do not try to start a REPL in another REPL.", 3)
    end

    is_repl_running = true
    local success, err = pcall(function()
        local ilua = Ilua:new(params)
        ilua:start()
        ilua:run()

    end)
    is_repl_running = false

    if not success then
        error("REPL encountered an error: " .. err, 3)
    end
end

function builtin_funcs.run_repl_from_console()
    run_repl({})
end

function builtin_funcs.run_repl_in_env(env)
    run_repl({ ["env"] = env })
end