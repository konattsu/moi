-- https://www.andersevenrud.net/neovim.github.io/lsp/configurations/
-- https://github.com/neovim/nvim-lspconfig/blob/master/doc/configs.md

return {
  { import = "plugins.lang.c" },
  { import = "plugins.lang.docker" },
  { import = "plugins.lang.go" },
  { import = "plugins.lang.html-css" },
  { import = "plugins.lang.json" },
  { import = "plugins.lang.lua" },
  { import = "plugins.lang.markdown" },
  { import = "plugins.lang.python" },
  { import = "plugins.lang.rust" },
  { import = "plugins.lang.shell" },
  { import = "plugins.lang.toml" },
  { import = "plugins.lang.typescript" },
  { import = "plugins.lang.yaml" },
}
