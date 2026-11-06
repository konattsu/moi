# desktop environment

GUIのバー・検索・ウィンドウ一覧・scratchpad・日本語入力・配色は [desktop-shell](modules/desktop-shell/README.md) で管理する。設定の実装と、実機へのapplyは別に行う。

Arch Linux desktop 用。host の共有 module に desktop-shell、Zen、UDEV Gothic を加える。[共通の定義・移行手順](../README.md) に従い、環境の検出結果を when で評価する。

```sh
moi -e desktop --source "file://$PWD" plan udev-gothic zen
moi -e desktop --source "file://$PWD" apply udev-gothic zen
```

## UDEV Gothic

[公式 v2.2.0](https://github.com/yuru7/udev-gothic/releases/tag/v2.2.0) の Nerd Fonts 版を固定 URL と SHA-256 で取得する。pinned archive の include に、UDEV Gothic 35NF の Regular / Bold / Italic / BoldItalic の4書体を指定し、~/.local/share/fonts/udev-gothic へ直接配置する。

選択外の NF / NFLG / 35NFLG は最終配置へ含めず、保管用ディレクトリと symlink は作らない。管理対象と管理記録は選択した4ファイルに限り、同じフォントディレクトリ内の他のファイルは変更・削除しない。

fontconfig は packages で用意し、配置後に別の shell 操作で fc-cache を実行する。フォント選択は各アプリで行う。更新は module.toml の version・成果物情報・include を変更する。

## Zen Browser

[公式 1.24b](https://github.com/zen-browser/desktop/releases/tag/1.24b) の x86_64 / aarch64 の成果物を固定している。

- 本体：`~/.local/share/moi/apps/zen/`
- 起動リンク：`~/.local/bin/zen`
- desktop entry：`~/.local/share/applications/zen.desktop`

本体は preserve archive とし、zen・updater・アイコンを required_paths に指定する。同じ版・成果物の定義の間は self-update を保持する。定義の版・成果物・policy を変えた場合や必須パスの修復時には再導入するため、downgrade も起こり得る。

本体、起動リンク、desktop entry は別操作で管理する。desktop entry は HOME を展開した絶対パスで生成し、差分があるときだけ更新して desktop database を再生成する。既定のブラウザーは自動変更しない。GTK など Zen が必要とする desktop runtime は利用環境で用意する。

旧配置の `~/.local/share/zen` と通常ファイルの起動 wrapper は自動移譲しない。移行前に [共通の注意事項](../README.md#旧定義からの移行) を確認する。
