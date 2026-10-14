---@type LazySpec
return {
  "akinsho/toggleterm.nvim",
  opts = function(_, opts)
    opts.start_in_insert = false
    opts.persist_mode = false
    -- Lazygit needs immediate keyboard interaction; shells open in Normal mode for reviewing output.
    opts.on_open = function(term)
      if term.cmd and (term.cmd == "lazygit" or term.cmd:match "^lazygit%s") then vim.cmd "startinsert!" end
    end
    opts.float_opts = vim.tbl_deep_extend("force", opts.float_opts or {}, { border = "single" })
    opts.highlights = opts.highlights or {}
    opts.highlights.FloatBorder = { guifg = "#2887FF", guibg = "NONE" }
  end,
}
