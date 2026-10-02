local format_on_save_filetypes = {
  c = true,
  cpp = true,
  javascript = true,
  javascriptreact = true,
  python = true,
  rust = true,
  typescript = true,
  typescriptreact = true,
}

local function find_upward(start, relative_path)
  local dir = start
  while dir and dir ~= "" do
    local candidate = vim.fs.joinpath(dir, relative_path)
    if vim.fn.executable(candidate) == 1 then return candidate end

    local parent = vim.fs.dirname(dir)
    if parent == dir then break end
    dir = parent
  end
end

local function project_executable(bufnr, executable, relative_paths)
  local filename = vim.api.nvim_buf_get_name(bufnr)
  local start = filename ~= "" and vim.fs.dirname(filename) or vim.uv.cwd()
  for _, relative_path in ipairs(relative_paths) do
    local candidate = find_upward(start, relative_path)
    if candidate then return candidate end
  end
  return executable
end

---@type LazySpec
return {
  "stevearc/conform.nvim",
  event = { "BufWritePre" },
  cmd = { "ConformInfo" },
  opts = {
    formatters_by_ft = {
      c = { "clang_format" },
      cpp = { "clang_format" },
      rust = { "rustfmt" },
      python = { "ruff_format", "black", stop_after_first = true },
      javascript = { "prettierd", "prettier", stop_after_first = true },
      javascriptreact = { "prettierd", "prettier", stop_after_first = true },
      typescript = { "prettierd", "prettier", stop_after_first = true },
      typescriptreact = { "prettierd", "prettier", stop_after_first = true },
    },
    formatters = {
      black = function(bufnr)
        return {
          command = project_executable(bufnr, "black", {
            ".venv/bin/black",
            "venv/bin/black",
          }),
        }
      end,
      prettier = function(bufnr)
        return {
          command = project_executable(bufnr, "prettier", {
            "node_modules/.bin/prettier",
          }),
        }
      end,
      prettierd = function(bufnr)
        return {
          command = project_executable(bufnr, "prettierd", {
            "node_modules/.bin/prettierd",
          }),
        }
      end,
      ruff_format = function(bufnr)
        return {
          command = project_executable(bufnr, "ruff", {
            ".venv/bin/ruff",
            "venv/bin/ruff",
          }),
        }
      end,
    },
    format_on_save = function(bufnr)
      if vim.g.disable_autoformat or vim.b[bufnr].disable_autoformat then return nil end
      if not format_on_save_filetypes[vim.bo[bufnr].filetype] then return nil end
      return {
        lsp_format = "fallback",
        quiet = true,
        timeout_ms = 1000,
      }
    end,
    notify_no_formatters = false,
  },
}
