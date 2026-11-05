# environment 定義

host は個人の Linux 環境、desktop は host の共有 module と Arch desktop 用 module、devcontainer はコンテナ内の環境を管理する。共有 module はディレクトリの symlink で取り込む。

定義は [仕様書](../docs/SPECIFICATION.md) の operations 形式を使う。各 module.toml の先頭に入力 Schema を指定する。moi は指定 environment の定義全体を検証し、選択した module の operations を記述順に適用する。

## 配布物と更新

| 対象 | 維持方針 | 宣言した版 | 配置先 |
| --- | --- | --- | --- |
| Neovim | pinned | 0.12.4 | `~/.local/share/moi/apps/nvim` |
| lazygit | pinned・選択配置 | 0.66.2 | `~/.local/bin/lazygit` |
| Yazi | pinned・選択配置 | 26.9.1 | `~/.local/bin/yazi` と `~/.local/bin/ya` |
| Zen | preserve | 1.24b | `~/.local/share/moi/apps/zen` |
| UDEV Gothic NF | pinned・選択配置 | 2.2.0 | `~/.local/share/fonts/udev-gothic` 内の35NFの4書体 |

archive の URL・SHA-256・形式・取り出す root は module.toml で固定する。一つの archive 操作に URL などを直接書く。Neovim・lazygit・Yazi・Zen は when.arch で CPU 別の操作に分け、UDEV Gothic は architecture に依存しない一つの操作とする。lazygit・Yazi・UDEV Gothic は include に必要な通常ファイルだけを列挙し、利用先へ直接配置する。Neovim と Zen は周辺ファイルを含む全体配置と起動リンクを使う。適用時に latest を取得しない。更新は module の version・成果物情報・選択内容を変更して行う。公式リリースの情報を定義更新時に確認する。

preserve は同じ版・成果物の定義の間に self-update を保持し、定義の版・成果物・include・policy が変われば再導入する。定義変更・修復時の downgrade は許容する。詳細は [archive の契約](../docs/archive.md) を参照。

win32yank は WSL の x86_64 でのみ導入する。配布 ZIP に Unix の実行権限がないため、v0.1.1 の archive の checksum と配置した実行ファイルの内容を検証し、shell の install コマンドで実行権限を設定する。

## PATH と依存

node module が nvm と Node を用意し、env.path_export で default の Node の PATH を後続へ提供する。npm を使う側は node に依存し、nvm の読み込みを繰り返し書かない。devcontainer も同じ node module を使う。

fd は共通 module にまとめる。Debian の fdfind から fd へのリンクはそこで一度だけ管理し、Neovim と Yazi はその module に依存する。

追加リポジトリが必要な package は shell 内で登録から導入まで行う。標準の packages は全 module 操作より先に集約して導入する。

## 旧定義からの移行

環境そのものへの apply は、この定義変更では行わない。新しい実行器へ移行してから plan で配置を確認する。

直接配置の archive は選択したファイルだけを管理し、.local/bin や fonts 全体は所有しない。管理記録も最終配置ファイルごとに分ける。配置済みの通常ファイル・symlink を記録なしで引き取る機能は設けない。新定義の管理対象と衝突する既存物は、利用者が退避・整理してから適用する。

全体配置で使っていた ~/.local/share/moi/apps/lazygit・yazi・udev-gothic や、以前の Neovim・Zen の配置は自動削除しない。include から外したファイルと記録も自動削除しない。必要なデータは利用者が確認して扱う。

標準操作の配置先の親 symlink は禁止。note は --show-note を指定した場合だけ表示し、同時 apply は行わない。
