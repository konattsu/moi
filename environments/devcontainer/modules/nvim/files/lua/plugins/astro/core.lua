---@type LazySpec
return {
  "AstroNvim/astrocore",
  ---@type AstroCoreOpts
  opts = {
    features = {
      large_buf = { size = 1024 * 256, lines = 10000 },
      autopairs = true,
      cmp = true,
      diagnostics = { virtual_text = true, virtual_lines = false },
      highlighturl = true,
      notifications = true,
    },
    diagnostics = {
      virtual_text = true,
      underline = true,
    },
    filetypes = {
      extension = {
        foo = "fooscript",
      },
      filename = {
        [".foorc"] = "fooscript",
      },
      pattern = {
        [".*/etc/foo/.*"] = "fooscript",
      },
    },
    options = {
      opt = {
        breakindent = false,
        cursorline = true,
        expandtab = true,
        guicursor = "c-i:hor20",
        linebreak = false,
        relativenumber = true,
        number = true,
        shiftwidth = 2,
        spell = false,
        softtabstop = 2,
        signcolumn = "yes",
        tabstop = 2,
        wrap = false,
      },
      g = {},
    },
    mappings = {
      n = {
        ["<Leader>tt"] = { "<Cmd>ToggleTerm direction=float<CR>", desc = "ToggleTerm float" },
        ["q"] = { "<cmd>quit<cr>", desc = "Quit window" },
        ["<F2>"] = { "q", desc = "Record macro" },
        ["<A-z>"] = {
          function()
            vim.wo.wrap = not vim.wo.wrap
            vim.wo.linebreak = false
            vim.wo.breakindent = false
          end,
          desc = "Toggle word wrap",
        },
        ["<Tab>"] = { function() require("astrocore.buffer").nav(vim.v.count1) end, desc = "Next buffer" },
        ["<S-Tab>"] = { function() require("astrocore.buffer").nav(-vim.v.count1) end, desc = "Previous buffer" },

        ["<Leader>bd"] = {
          function()
            require("astroui.status.heirline").buffer_picker(
              function(bufnr) require("astrocore.buffer").close(bufnr) end
            )
          end,
          desc = "Close buffer from tabline",
        },
      },
    },
  },
}
