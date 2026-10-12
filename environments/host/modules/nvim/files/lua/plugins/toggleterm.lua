---@type LazySpec
return {
  "akinsho/toggleterm.nvim",
  opts = function(_, opts)
    opts.start_in_insert = false
    opts.persist_mode = false
    opts.float_opts = vim.tbl_deep_extend("force", opts.float_opts or {}, { border = "single" })
    opts.highlights = opts.highlights or {}
    opts.highlights.FloatBorder = { guifg = "#2887FF", guibg = "NONE" }
  end,
}
