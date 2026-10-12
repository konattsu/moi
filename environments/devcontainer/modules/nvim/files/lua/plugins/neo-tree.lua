local width_file = vim.fn.stdpath "state" .. "/sidebar-width.txt"
local sidebar_width = 40
local sidebar_winid
local ok, lines = pcall(vim.fn.readfile, width_file)
if ok then
  local width = tonumber((lines[1] or ""):match "^width=(%d+)$")
  -- Discard oversized values already saved by the previous implementation.
  if width and width > 0 and width < 100 then sidebar_width = width end
end

local function save_sidebar_width()
  if not sidebar_winid or not vim.api.nvim_win_is_valid(sidebar_winid) then return end
  local width = vim.api.nvim_win_get_width(sidebar_winid)
  -- Closing the editor can expand Neo-tree to the full screen before exit.
  -- Ignore widths >= 100 columns so that expansion does not overwrite the saved width.
  if width >= 100 then return end
  sidebar_width = width
  pcall(function()
    vim.fn.mkdir(vim.fn.stdpath "state", "p")
    vim.fn.writefile({ "width=" .. sidebar_width }, width_file)
  end)
end

vim.api.nvim_create_autocmd("VimLeavePre", {
  group = vim.api.nvim_create_augroup("NeoTreeSidebarWidth", { clear = true }),
  callback = save_sidebar_width,
  desc = "Save Neo-tree sidebar width on exit",
})

local function show_tree(source)
  vim.schedule(function()
    require("neo-tree.command").execute {
      action = "focus",
      source = source,
      position = "left",
    }
  end)
end

return {
  {
    "AstroNvim/astrocore",
    opts = function(_, opts)
      local maps = opts.mappings
      local autocmds = opts.autocmds
      -- AstroNvim default `:Neotree toggle` always uses `default_source` (filesystem), so
      -- toggling from Git/Bufs resets the source selector to File. `source = "last"` matches
      -- the tab you picked (neo-tree updates `last` on next_source / prev_source).
      maps.n["<Leader>e"] = {
        function() require("neo-tree.command").execute { toggle = true, source = "last" } end,
        desc = "Toggle Explorer",
      }
      maps.n["<Leader>o"] = {
        function()
          if vim.bo.filetype == "neo-tree" then
            vim.cmd.wincmd "p"
          else
            require("neo-tree.command").execute { action = "focus", source = "last" }
          end
        end,
        desc = "Toggle Explorer Focus",
      }

      autocmds.neo_tree_default = {
        {
          event = "User",
          pattern = "VeryLazy",
          once = true,
          callback = function() show_tree "filesystem" end,
          desc = "Open and focus Neo-tree on startup",
        },
      }
    end,
  },
  {
    "nvim-neo-tree/neo-tree.nvim",
    opts = {
      event_handlers = {
        {
          event = "neo_tree_window_after_open",
          handler = function(args)
            if args.position ~= "left" then return end
            sidebar_winid = args.winid
            vim.api.nvim_win_set_width(sidebar_winid, math.min(sidebar_width, math.max(1, vim.o.columns - 20)))
          end,
        },
        {
          event = "neo_tree_window_before_close",
          handler = function(args)
            if args.position ~= "left" then return end
            sidebar_winid = args.winid
            save_sidebar_width()
            sidebar_winid = nil
          end,
        },
      },
      window = {
        width = sidebar_width,
        mappings = {
          ["<space>"] = "none",
          ["<Tab>"] = "next_source",
          ["<S-Tab>"] = "prev_source",
        },
      },

      default_component_configs = {
        name = {
          use_git_status_colors = false,
        },
        symlink_target = {
          enabled = true,
          text_format = " @",
        },
      },

      filesystem = {
        filtered_items = {
          visible = true,
          hide_dotfiles = false,
          hide_hidden = false,
          hide_by_pattern = {
            "*.meta",
            "*.unity",
            "*.fls",
            "*.aux",
            "*.dvi",
            "*.pdf",
            "*.gz",
            "*.fdb_latexmk",
          },
        },
      },
    },
  },
}
