# devcontainer environment

Devcontainer 内で `moi` を適用するための environment。

## Install inside a devcontainer

この repository は devcontainer の中身を直接管理しない。
`.devcontainer/devcontainer.json` はプロジェクトごとに存在したり、存在しなかったり、共有設定として管理されていたりするため、個人用の `postCreateCommand` としてこの設定を混ぜない。

基本は devcontainer に入った後、手動で実行する。

公開 repository から実行する場合:

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/install.sh \
  | MOI_ENVIRONMENT=devcontainer bash -s -- apply
```

## Per-project setup

プロジェクトに `.devcontainer` がある場合は、共有されている `devcontainer.json` に個人用の `curl ... | bash` や `postCreateCommand` を足さない。
そのプロジェクトで必要なら、devcontainer 起動後に上の install command を手動実行する。

プロジェクトに `.devcontainer` がなく、自分だけの devcontainer 設定を置きたい場合は、`.devcontainer/` を作ってから、その repository の `.git/info/exclude` に追加する。

```sh
mkdir -p .devcontainer
printf '%s\n' '.devcontainer/' >> .git/info/exclude
```

`.git/info/exclude` はその clone だけに効く ignore 設定で、commit されない。
個人用の `devcontainer.json` や `postCreateCommand` を置くならここで隔離する。

既に `.devcontainer` が repository に含まれている場合は、`.git/info/exclude` では tracked file の変更を隠せない。
その場合は共有設定を触らず、devcontainer 内で手動実行する。

## SSH agent / commit signing

Git の commit signing や private repository access に host 側の `ssh-agent` / `keychain` を使う場合、agent socket の mount は devcontainer 作成時に決まる。
そのため、devcontainer 起動後にこの environment を `curl ... | bash` で入れても、後から host の agent socket を container に追加することはできない。

VS Code Dev Containers など、client が SSH agent forwarding を自動で行う場合はそれを使う。
この場合、host 側で `SSH_AUTH_SOCK` が client に見えている必要があるため、`keychain` を読んだ shell から `code` / `devcontainer` を起動する。

client の自動 forwarding が使えない project では、project 側の devcontainer 設定に agent forwarding を入れてもらう必要がある。
Docker Compose を使っている場合の例:

```yaml
services:
  app:
    volumes:
      - ${SSH_AUTH_SOCK}:/tmp/ssh-agent.sock
    environment:
      SSH_AUTH_SOCK: /tmp/ssh-agent.sock
```

`devcontainer.json` だけで構成されている場合の例:

```jsonc
{
  "mounts": [
    "source=${localEnv:SSH_AUTH_SOCK},target=/tmp/ssh-agent.sock,type=bind"
  ],
  "containerEnv": {
    "SSH_AUTH_SOCK": "/tmp/ssh-agent.sock"
  }
}
```

ただし、既に repository で管理されている `.devcontainer` を個人設定として変更してはいけない project では、この設定を自分の clone だけに安全に足す標準的な local override はない。
その場合は project に汎用的な SSH agent forwarding 設定を入れる、client の自動 forwarding を使う、または devcontainer 外で署名が必要な Git 操作を行う。

## Modules

`nvim`, `zsh`, `lazygit` と、それらを host と同じ定義のまま使うために必要な依存 module を含めている。

## Neovim project policy

Devcontainer 用の Neovim は project の toolchain を優先する。

- Mason は補助的な LSP だけを自動 install する
- `rust_analyzer`, `ts_ls`, `pyright`, `html`, `cssls` は Mason の自動 install 対象にしない
- LSP の global format on save は有効にしない
- 保存時 format は `conform.nvim` で filetype を限定する
- Markdown, JSON, YAML などは保存時 format 対象にしない

現在、保存時 format 対象は C, C++, Rust, Python, JavaScript, TypeScript のみ。
C / C++ は devcontainer の `PATH` にある `clang-format` を使う。
Python は `.venv/bin` / `venv/bin` の `ruff` と `black`、JavaScript / TypeScript は `node_modules/.bin` の `prettierd` と `prettier` を project root 方向に探索して優先する。
見つからない場合は devcontainer 内の `PATH` から探し、それでもなければ何もしない。

C / C++ project の devcontainer 構成例は [`docs/c-devcontainer.md`](../../docs/c-devcontainer.md) を参照。

想定する project 側の設定例。

Rust:

```toml
# rust-toolchain.toml
[toolchain]
channel = "stable"
components = ["rustfmt", "rust-analyzer"]
```

```toml
# rustfmt.toml
edition = "2021"
max_width = 120
```

Python:

```toml
# pyproject.toml
[tool.ruff]
line-length = 120

[tool.ruff.format]
quote-style = "double"

[tool.black]
line-length = 120
```

JavaScript / TypeScript:

```json
{
  "scripts": {
    "format": "prettier --write .",
    "lint": "eslint ."
  },
  "devDependencies": {
    "prettier": "^3.0.0",
    "eslint": "^9.0.0",
    "typescript": "^5.0.0",
    "typescript-language-server": "^4.0.0"
  }
}
```

```json
{
  "printWidth": 120,
  "singleQuote": false,
  "trailingComma": "all"
}
```

上の JSON は `.prettierrc` の例。

LSP の project-local 設定が必要な場合は project root に `.neoconf.json` を置く。
これは formatter/linter の設定ファイルではなく、Neovim の LSP 設定用。

```json
{
  "lspconfig": {
    "rust_analyzer": {
      "settings": {
        "rust-analyzer": {
          "checkOnSave": false
        }
      }
    },
    "ts_ls": {
      "settings": {
        "typescript": {
          "format": {
            "enable": false
          }
        },
        "javascript": {
          "format": {
            "enable": false
          }
        }
      }
    }
  }
}
```

Lint はまだ Neovim 側では自動実行しない。
`flake8`, `eslint` などを保存時に必須化したくなったら、project の設定ファイルを先に置いた上で `nvim-lint` を追加する。

## Adding language support

Devcontainer 側で言語を増やすときは、先に project の toolchain と規約を決めてから Neovim 設定を足す。
Neovim 側だけで言語環境を完結させない。

基本方針:

- project 規約に関わる LSP, formatter, linter は devcontainer / project 側で入れる
- Mason の自動 install は補助的な LSP に限る
- project 規約に関わる tool は Mason の `ensure_installed` に入れない
- `plugins/lang/<language>.lua` は LSP 起動と language 固有設定を担当する
- `plugins/conform.lua` は保存時 format する filetype だけを明示する
- formatter は project-local binary を優先し、なければ `PATH` を見る
- Markdown, JSON, YAML など、project ごとの流儀が割れやすい filetype は安易に保存時 format 対象にしない
- lint を保存時に走らせたい場合は `nvim-lint` を追加し、filetype ごとに明示する
- project root に置く formatter/linter 設定例をこの README に追記する

言語追加時の手順:

1. devcontainer image, Dockerfile, project setup script などで必要な runtime / toolchain を入れる
2. project root に tool 自身の設定ファイルを置く
3. 必要なら `environments/devcontainer/modules/nvim/files/lua/plugins/mason.lua` から該当 LSP を外す
4. `environments/devcontainer/modules/nvim/files/lua/plugins/lang/<language>.lua` で LSP を有効化する
5. 保存時 format が必要な filetype だけ `environments/devcontainer/modules/nvim/files/lua/plugins/conform.lua` に追加する
6. `environments/devcontainer/modules/nvim/module.toml` に追加した Lua file を登録する
7. `moi -e devcontainer --source "file://$PWD" plan --platform debian nvim` で確認する

Go を追加する場合の例。

Project 側:

```text
go.mod
```

```yaml
# .golangci.yml
linters:
  enable:
    - govet
    - staticcheck
```

Mason の自動 install から project toolchain 側へ寄せる場合は `mason.lua` から `gopls` を外す。

`plugins/lang/go.lua` では `gopls` を有効化する。
`gopls` が devcontainer 内の `PATH` にあれば起動する。

`plugins/conform.lua` には必要になった時点で Go を追加する。

```lua
local format_on_save_filetypes = {
  go = true,
}
```

```lua
formatters_by_ft = {
  go = { "goimports" },
}
```

`goimports` が devcontainer 内の `PATH` にある前提。
project-local binary を使う運用にしたい場合は、既存の Python / JavaScript と同じように探索 helper に追加する。
