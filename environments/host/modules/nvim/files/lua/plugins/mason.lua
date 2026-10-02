---@type LazySpec
return {
  {
    "mason-org/mason.nvim",
    opts = {},
  },
  {
    "mason-org/mason-lspconfig.nvim",
    opts = {
      ensure_installed = {
        "clangd",
        "lua_ls",
        "gopls",
        "rust_analyzer",
        "ts_ls",
        "pyright",
        "bashls",
        "taplo",
        "jsonls",
        "yamlls",
        "marksman",
        "dockerls",
        "docker_compose_language_service",
        "html",
        "cssls",
      },
    },
  },
  {
    "WhoIsSethDaniel/mason-tool-installer.nvim",
    enabled = false,
  },
}
