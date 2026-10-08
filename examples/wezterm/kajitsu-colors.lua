-- Save as a module next to wezterm.lua. Apply AFTER your existing color choice:
-- require('kajitsu-colors').apply(config)
local wezterm = require('wezterm')
local M = {}

function M.apply(config)
  local state = os.getenv('XDG_STATE_HOME')
  if not state or state:sub(1, 1) ~= '/' then
    state = wezterm.home_dir .. '/.local/state'
  end
  local path = state .. '/kajitsu/wezterm-colors.lua'
  wezterm.add_to_config_reload_watch_list(path)
  local ok, colors = pcall(function()
    return assert(loadfile(path))()
  end)
  if not ok or type(colors) ~= 'table' then return end
  local function color(value)
    return type(value) == 'string' and value:match('^#%x%x%x%x%x%x$')
  end
  for _, key in ipairs({ 'foreground', 'background', 'cursor_fg', 'cursor_bg',
    'cursor_border', 'selection_fg', 'selection_bg' }) do
    if not color(colors[key]) then return end
  end
  for _, key in ipairs({ 'ansi', 'brights' }) do
    if type(colors[key]) ~= 'table' or #colors[key] ~= 8 then return end
    for _, value in ipairs(colors[key]) do
      if not color(value) then return end
    end
  end
  config.colors = colors
end

return M
