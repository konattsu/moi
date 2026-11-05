# Neovim

## 構成

この repository では Neovim 本体、Lua プラグイン群、設定を分けて管理する。

- 本体：module.toml の pinned archive。Neovim 0.12.4 の x86_64 / aarch64 の固定 URL と SHA-256 を使い、`~/.local/share/moi/apps/nvim` へ配置する。
- 起動リンク：`~/.local/bin/nvim` から本体の bin/nvim へ symlink する。
- AstroNvim とプラグイン：lazy.nvim と repository の lazy-lock.json が管理する。
- 設定：host / devcontainer の module 内の静的ファイルを file 操作で `~/.config/nvim` へコピーする。

host と devcontainer の本体は同じ版に揃える。npm は node module が提供する default の Node の PATH を使う。Debian の fd リンクと WSL x86_64 の win32yank は独立 module とし、nvim の依存から用意する。

## Neovim 本体の更新

host / devcontainer 両方の module.toml の archive 操作の version と、when.arch で分けた各操作の URL・SHA-256・root を更新する。版の問い合わせをする shell や実行時の latest 取得は行わない。

```sh
moi -e host plan nvim
moi -e host apply nvim
nvim --version
```

pinned は定義と実配置が一致していれば取得・解凍・再配置しない。定義変更や実配置の差分がある場合は宣言した成果物から再配置する。apply nvim は本体だけでなく、選択された依存と設定も適用する。

旧本体の `~/.local/share/nvim-linux-x86_64` は自動削除しない。新しい実行器と管理記録への移行は [environment の移行手順](../environments/README.md#旧定義からの移行) を参照。

## AstroNvimとプラグインの更新

Neovim内で次を実行する。

```vim
:Lazy update
```

AstroNvim本体の更新後に追加の更新が表示される場合は、Neovimを再起動してもう一度
`:Lazy update` を実行する。更新後は `:checkhealth` を実行し、普段使うfiletypeを
いくつか開いて確認する。問題がなければ生成された `~/.config/nvim/lazy-lock.json`
を module配下のlockfileへ反映する。

## lockfile

lazy.nvimは `~/.config/nvim/lazy-lock.json` を使ってプラグインのcommitを固定する。
このリポジトリでは host と devcontainer の module配下に同じ内容のlockfileを置き、
`moi apply nvim` で `~/.config/nvim/lazy-lock.json` へ配置する。

`pin_plugins` は `nil` のままにし、プラグイン固定はlockfileへ集約する。
host と devcontainer のlockfileは同じ内容に保つ。現在は devcontainer 側にだけ
`conform.nvim` のspecがあるため、lockfile更新はplugin specのsupersetである
devcontainer 側で行う。host 側で生成したlockfileをそのまま反映すると、
devcontainer 専用pluginのentryが落ちる可能性がある。

## 通常の更新順序

1. Neovim本体を更新する場合は、host/devcontainer 両方の archive の version と成果物情報を変更する。
2. `moi apply nvim` で指定versionのNeovimと既存lockfileを配置する。
3. `:Lazy update` でAstroNvimとプラグインを更新する。
4. Neovimを再起動し、必要ならもう一度 `:Lazy update` を実行する。
5. `:checkhealth` と普段使うfiletypeで確認する。
6. 問題がなければ `~/.config/nvim/lazy-lock.json` を host/devcontainer両方の
   `files/lazy-lock.json` へ反映する。

## サイドバーの幅

Neo-treeの左サイドバーは、閉じる直前とNeovim終了時の実際の幅を保存し、
次回表示時に復元する。host・devcontainer両方で有効。

状態ファイルは `stdpath("state") .. "/sidebar-width.txt"` に置き、
`width=36` のような1行を直接上書きする。一時ファイルは使わない。
保存値がない・不正・100列以上の場合は40列で開く。全プロジェクトで共通の幅を使い、
複数のNeovimを起動した場合は最後に保存した値を採用する。
端末サイズ変更による幅の変化も保存対象になる。ただし100列以上の幅は保存しない。
編集ウィンドウを閉じた際にNeo-treeが画面全幅まで広がり、その幅が終了時に
保存されるのを防ぐための簡易的な制限。

ファイルの場所はNeovimで次のコマンドから確認できる。

```vim
:echo stdpath('state') .. '/sidebar-width.txt'
```
