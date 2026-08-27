local c = require "colors.solarized_dark.palette"

local M = {}

local function hi(group, opts)
  vim.api.nvim_set_hl(0, group, opts)
end

function M.apply()
  -- File type is already communicated by the icon, so keep names neutral.
  hi("NeoTreeFileName", { fg = c.base0 })
  hi("NeoTreeDirectoryName", { link = "NeoTreeFileName" })
  hi("NeoTreeFileNameOpened", { fg = c.base0, bold = true })
  hi("NeoTreeRootName", { fg = c.base1, bold = true, italic = true })
  hi("NeoTreeSymbolicLinkTarget", { fg = c.base01, italic = true })
  hi("NeoTreeDotfile", { fg = c.base01 })
  hi("NeoTreeDimText", { fg = c.base01 })

  -- Git status uses both a distinct letter and color for quick recognition.
  hi("NeoTreeModified", { fg = c.yellow })
  hi("NeoTreeGitAdded", { fg = c.green })
  hi("NeoTreeGitDeleted", { fg = c.red })
  hi("NeoTreeGitModified", { fg = c.yellow })
  hi("NeoTreeGitRenamed", { fg = c.cyan })
  hi("NeoTreeGitConflict", { fg = c.red, bold = true })
  hi("NeoTreeGitUntracked", { fg = c.magenta })
  hi("NeoTreeGitStaged", { fg = c.green })
  hi("NeoTreeGitUnstaged", { fg = c.orange })
  hi("NeoTreeGitIgnored", { fg = c.base01, italic = true })
  hi("NeoTreeIgnored", { link = "NeoTreeGitIgnored" })
end

return M
