local M = {}

function M.setup()
  vim.opt.termguicolors = true
  vim.opt.background = "dark"

  vim.cmd.highlight "clear"

  if vim.fn.exists "syntax_on" == 1 then
    vim.cmd.syntax "reset"
  end

  vim.g.colors_name = "solarized-dark"

  require("colors.solarized_dark.base").apply()
  require("colors.solarized_dark.neo_tree").apply()
  require("colors.solarized_dark.rust").apply()
end

return M
