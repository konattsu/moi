local function show_tree(source)
  vim.schedule(function()
    require("neo-tree.command").execute {
      action = "focus",
      source = source,
      position = "left",
    }
  end)
end

local function is_git_status_segment(segment)
  return type(segment.highlight) == "string" and segment.highlight:match "^NeoTreeGit" ~= nil
end

local function compact_container(config, node, state, available_width)
  local rendered, wanted_width = require("neo-tree.sources.common.components").container(
    config,
    node,
    state,
    available_width
  )
  local first_git_index
  local reclaimed_width = 0

  for index, segment in ipairs(rendered) do
    if is_git_status_segment(segment) then
      first_git_index = first_git_index or index
      local previous = rendered[index - 1]
      if previous and is_git_status_segment(previous) then
        local previous_width = vim.api.nvim_strwidth(previous.text)
        local current_width = vim.api.nvim_strwidth(segment.text)
        previous.text = previous.text:gsub("%s+$", "")
        previous.no_next_padding = true
        segment.text = segment.text:gsub("^%s+", "")
        reclaimed_width = reclaimed_width
          + previous_width
          + current_width
          - vim.api.nvim_strwidth(previous.text)
          - vim.api.nvim_strwidth(segment.text)
      end
    end
  end

  if reclaimed_width > 0 then
    local spacer
    for index = first_git_index - 1, 1, -1 do
      if rendered[index].text:match "^%s*$" then
        spacer = rendered[index]
        break
      end
    end

    if spacer then
      spacer.text = spacer.text .. string.rep(" ", reclaimed_width)
    else
      rendered[#rendered].text = rendered[#rendered].text .. string.rep(" ", reclaimed_width)
    end
  end

  return rendered, wanted_width
end

local function dim_ignored_name(config, node, state)
  local components = require "neo-tree.sources.common.components"
  local name = components.name(config, node, state)
  local git_status = components.git_status({}, node, state)

  if not git_status.highlight and vim.islist(git_status) then
    for _, segment in ipairs(git_status) do
      if segment.highlight == "NeoTreeGitIgnored" then
        git_status = segment
        break
      end
    end
  end

  if git_status.highlight == "NeoTreeGitIgnored" then
    name.highlight = "NeoTreeGitIgnored"
  end

  return name
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
      window = {
        width = 40,
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
        git_status = {
          symbols = {
            added = "A",
            deleted = "D",
            modified = "M",
            renamed = "R",
            untracked = "N",
            ignored = "I",
            unstaged = "U",
            staged = "S",
            conflict = "C",
          },
        },
        symlink_target = {
          enabled = true,
          text_format = " @",
        },
      },

      filesystem = {
        components = {
          container = compact_container,
          name = dim_ignored_name,
        },
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

      buffers = {
        components = {
          container = compact_container,
        },
      },

      git_status = {
        components = {
          container = compact_container,
        },
      },
    },
  },
}
