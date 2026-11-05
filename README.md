# moi

Linux(Debian/Arch) 用の環境をセットアップする repo

いわゆる dotfiles を扱うもの

## 特徴

- 管理対象をモジュール単位で分割できる(例: [modules](environments/host/modules))
- パッケージ, ファイル, marker block, バイナリ, 任意コマンドをまとめて適用できる
- 特殊なインストール手順は module の shell 操作に記述する
  - 追加リポジトリが必要なものは, 登録からパッケージ導入まで shell で行う
  - 標準の `packages` は全 module 操作より先に一括導入する

## Install

実行に `curl`, `git` が必要.

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/install.sh \
  | MOI_ENVIRONMENT=host bash -s -- apply
```

この例は GitHub Releases の latest から環境に合う `moi` の archive を取得し, checksum を検証してから binary を `~/.local/bin/moi` に配置し, `host` environment を apply する.

## Usage

よく使う操作:

```sh
moi -e host plan
moi -e host apply
moi -e host apply nvim zsh
moi upgrade
```

### コマンド

```txt
moi [global options] plan [run options] [module ...]
moi [global options] apply [run options] [apply options] [module ...]
moi [global options] install-command -e ENV [install options] [plan|apply] [arg ...]
moi [global options] upgrade [--force]
```

詳細は `--help`.

## 設定

設定ファイルは `~/.config/moi/config.toml`.

```toml
default_environment = "host"
default_folder_name = "environments"
default_source = "https://github.com/konattsu/moi.git"
```

- 設定値: コマンドライン引数, 環境変数, 設定ファイルの順
- 必須: `environment` と操作(`plan` / `apply`)
- `default_folder_name` と `default_source` は設定がなければ上記の既定値を使う.

| 対象 | コマンドライン | 環境変数 |
| --- | --- | --- |
| environment | `-e`, `--environment` | `MOI_ENVIRONMENT` |
| folder name | `--folder-name` | `MOI_FOLDER_NAME` |
| source | `--source` | `MOI_SOURCE` |

`install-command` だけが使う install script の場所は, 必要なら手で設定ファイルに追加する.
設定ファイルの読み込みだけでは, ファイルの生成・更新は行わない.

```toml
default_install_source = "file:///home/....."
default_install_script = "install.sh"
```

## 仕組み

実行の流れ:

1. `install.sh` が GitHub Releases の latest から `moi` の archive と checksum をダウンロードし, 検証・展開して `~/.local/bin/moi` に配置する
2. `moi` が設定から source, folder, environment を決める
3. source が `https://` の場合は一時ディレクトリへ clone し, `file:///` の場合はその checkout を使う
4. `moi` が `<folder>/<environment>/modules/*/module.toml` を読み, plan または apply を実行する

source が `https://` の場合, 一時 clone は実行後に削除する.

## その他

- 初回セットアップ後の対話的な作業には以下が便利
  - debian: `curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/scripts/followup-debian.sh | bash -s -- --yes`
  - arch: `curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/scripts/followup-arch.sh | bash -s -- --yes`
- Neovim本体, AstroNvim, プラグインの更新方法は [docs/nvim.md](docs/nvim.md) を参照
- モジュール定義の仕様は [仕様書](docs/SPECIFICATION.md)（[実行モデル](docs/execution.md)、[ADR](docs/adr.md)） を参照

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.
