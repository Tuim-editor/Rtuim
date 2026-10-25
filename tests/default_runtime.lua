local notifications = {}
vim.rpcnotify = function(_, method) table.insert(notifications, method); return true end
local function color(group, attr)
    return vim.api.nvim_get_hl(0, { name = group, link = false })[attr or 'fg']
end
local requested = vim.env.TUIM_TEST_SAVED_THEME
if requested then
    vim.fn.mkdir(vim.fn.stdpath('data'), 'p')
    vim.fn.writefile({vim.json.encode({theme=requested})}, vim.fn.stdpath('data') .. '/settings.json')
end
dofile('src/nvim/tuim_init.lua')
local explorer = vim.fn.maparg(' e', 'n', false, true)
assert(type(explorer.callback) == 'function')
explorer.callback()
assert(notifications[#notifications] == 'tuim_toggle_explorer')
for _, mode in ipairs({'n', 'v', 'i'}) do
    assert(vim.fn.maparg('<M-s>', mode, false, true).desc ~= 'Send selected text to AI terminal')
    assert(vim.fn.maparg('<M-c>', mode, false, true).desc ~= 'Send entire file content to AI terminal')
    assert(vim.fn.maparg('<C-M-s>', mode, false, true).desc == 'Send selected text to AI terminal')
end
local before = #vim.api.nvim_list_wins()
vim.fn.maparg('<M-s>', 'n', false, true).callback()
assert(#vim.api.nvim_list_wins() == before + 1, 'Alt+S must split')
vim.fn.maparg('<M-c>', 'n', false, true).callback()
assert(#vim.api.nvim_list_wins() == before, 'Alt+C must close split')

if requested then
    assert(vim.wait(1000, function() return vim.g.colors_name == requested end))
else
    assert(vim.g.colors_name == 'vscode', 'Fresh installs must use VS Code Dark Modern')
    assert(color('Normal', 'bg') == 0x1f1f1f and color('Normal') == 0xcccccc)
    assert(color('String') == 0xce9178 and color('Function') == 0xdcdcaa)
    assert(color('TuimAccent') == 0x0078d4)
    assert(color('Visual', 'bg') == 0x264f78, 'Selection must keep the VS Code palette')
end
if vim.env.TUIM_DISABLE_PLUGINS ~= '1' then
    local config = require('lazy.core.config')
    local ts = config.plugins['nvim-treesitter']
    assert(ts and ts.lazy == false and ts._.loaded, 'Treesitter must load at startup')
    assert(#_G.tuim_default_parsers == 19)
    for _, lang in ipairs(_G.tuim_default_parsers) do
        assert(vim.treesitter.language.add(lang), 'Missing parser: '..lang)
        assert(vim.treesitter.query.get(lang, 'highlights'), 'Missing highlights: '..lang)
    end
    vim.cmd.edit('src/main.rs')
    assert(vim.wait(2000, function() return vim.treesitter.highlighter.active[vim.api.nvim_get_current_buf()] ~= nil end), 'Treesitter did not attach to the opened file')
    local parser = vim.treesitter.get_parser(0, 'rust')
    assert(parser:parse()[1]:root():type() == 'source_file')
    local query = vim.treesitter.query.get('rust', 'highlights')
    local groups = {}
    for id in query:iter_captures(parser:parse()[1]:root(), 0) do groups[query.captures[id]] = true end
    assert(groups['function'] and groups['string'] and groups['keyword'], 'Expected distinct syntax captures')
end
_G.tuim_save_settings()
local saved = vim.json.decode(table.concat(vim.fn.readfile(vim.fn.stdpath('data') .. '/settings.json'), '\n'))
assert(saved.theme == (requested or 'vscode'))
print('Default runtime passed: theme, selection, persistence, and available parser/highlight checks')
vim.cmd('qa!')
