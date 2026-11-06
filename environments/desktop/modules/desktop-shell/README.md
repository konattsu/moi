# desktop-shell

Arch / HyprlandのGUI設定。UIは英語、ドキュメントは日本語とする。検索とウィンドウ一覧は入力欄と内容だけを表示し、見出し・キー操作の説明・常時表示の補足文は置かない。

## 設定を変更する場所

| ファイル | 内容 |
| --- | --- |
| [files/config/settings.json](files/config/settings.json) | 日本語キー配列、scratchpadの幅・高さ・位置・透過、壁紙、表示時間 |
| [files/config/theme.json](files/config/theme.json) | 背景・文字・アクセント・警告・成功・エラーの6色 |
| [files/config/waybar.json](files/config/waybar.json) | バーの項目とクリック操作 |
| [files/config/hyprland.lua](files/config/hyprland.lua) | キー操作とウィンドウ配置 |

枠・選択背景・補助文字色は6色から導出する。言語ごとに色を手で揃える必要はない。`generated/` は生成物なので編集しない。設定を再現するため、変更はこのrepoの上記ファイルへ入れる。

`keyboard_layout` の初期値は **`jp`**。Fcitx5のキーボードも同じ値から生成する。Caps Lockは半角英数／Mozcの日本語入力を切り替える専用キーとして使う。

## 操作

| キー | 動作 |
| --- | --- |
| Super単独を離す | バーだけを表示／非表示。現在のアプリのフォーカスを維持 |
| Super+Space | 共通検索 |
| Super+O | ウィンドウのプレビュー一覧 |
| Super+P | クリップボード履歴 |
| Super+U | scratchpadを表示／非表示。空なら新規起動 |
| Super+Shift+U | scratchpadを現在のワークスペースへタイル化 |
| Super+Ctrl+U | フォーカス中のkittyをscratchpadへ取り込む |
| Super+Enter | 通常のkittyをタイルで起動 |
| Super+H/J/K/L | フォーカスを左／下／上／右へ移動 |
| Super+Shift+H/J/K/L | ウィンドウを移動 |
| Super+Ctrl+H/J/K/L | サイズ変更 |
| Super+N / M / I | 閉じる／全画面切り替え／ロック |
| Super+1〜0 | ワークスペース1〜10へ移動 |
| Super+Shift+1〜0 | ウィンドウをワークスペース1〜10へ移動 |
| Esc / Ctrl+[ | 開いている検索・一覧・詳細を閉じる |

検索は完全な空欄・空白だけなら候補を表示しない。**prefixだけを入力した場合は、その種類の候補を表示する。**

| 入力 | 候補 |
| --- | --- |
| 通常の文字列 | アプリ・ウィンドウ・履歴・Web検索 |
| `@` / `@ 文字列` | アプリ一覧／絞り込み |
| `%` / `% 文字列` | ウィンドウ一覧／絞り込み |
| `$` / `$ 文字列` | 履歴一覧／絞り込み |
| `&` / `& 文字列` | 検索先を開く／Web検索 |

上下キーで候補を選び、Enterで実行する。履歴の選択はクリップボードへ戻すだけで、自動貼り付けはしない。検索文字列をシェルコマンドとして実行しない。各種類25件まで表示する。

## バーと表示

Waybarの標準モジュールに、同梱したGoogle Material IconsのSVGを背景として描画する。`BAT`、`VOL`、`BRT`、`N`などの略称は付けない。バッテリー・音量・明るさはアイコンと数値、通知はベルと未確認件数を表示する。充電・低電池・ミュート・切断・Bluetooth無効には別のSVGを使う。

通知の未確認件数は、ポップアップが時間切れで消えても残る。通知一覧を開くか通知を明示的に閉じると既読扱いになる。置換通知で件数を重複加算しない。履歴はプロセス内に最大50件保持し、ディスクへ履歴本文は保存しない。

バーは下の2角だけを丸める。検索・一覧・通知・詳細などのフローティング表示は4角を丸める。通常のタイル枠は角丸にしない。影・ぼかし・グラデーションは使わない。

音量／明るさの変更時にはバーだけを2秒表示する。Superで明示的に表示したバーはこのタイマーで閉じない。クリックすると対応する詳細や設定アプリを開く。汎用の操作パネルは設けない。

SVGの原本とライセンスは [files/assets/material](files/assets/material) に同梱する。[UPSTREAM.md](files/assets/material/UPSTREAM.md) に固定した取得元を記載した。描画用コピーのみテーマ生成時に着色する。

## scratchpadの修正

初期値は表示可能な論理領域の80% × 80%、上端から24px、透過率は不透明度88%。表示後にサイズと位置を適用する。タイル化すると専用枠を空け、次回呼び出しで新しいterminalを作る。

実機の読み取り調査では、2560×1600・倍率2の画面でscratchpadが `fullscreen=1`、`fullscreenClient=1` になっていた。Hyprlandの値1は最大化で、これもリサイズを拒否するfullscreen状態に含まれる。解除しないままフロート化・リサイズしていたことが `Window is fullscreen` の原因だった。

現在は表示・取り込み・タイル化・通常terminalのフォーカス処理の前に、内部とクライアント双方を0へ戻す。新規kittyには `--start-as normal` を指定し、scratchpadの起動ルールでも最大化要求を抑止する。報告された状態から **1024×640、位置128,24** に戻る回帰テストを用意した。

取り込みでは旧scratchpadに終了要求を出す。終了しなければ処理を中止する。Vimを使う出力閲覧は追加しない。カーソル色は白を維持する。通常のkittyのコピー・貼り付け・URL／パスのヒント操作は残す。

## 適用

この変更の作成・検証では、ホストへのapplyは行っていない。

```sh
moi -e desktop --source "file://$PWD" plan desktop-shell
moi -e desktop --source "file://$PWD" apply desktop-shell
```

既存のWaybar、swaync/dunst、Fcitx5、hypridle、壁紙の重複起動を外す。`udev-gothic` も依存として対象になる。設定は `~/.config` と `~/.zshrc` の専用blockへ配置する。フォントは既存moduleの配置先を使う。

適用時はユーザーサービスの定義を再読込し、**すでに実行中の `moi-desktop.service` だけを再起動**する。停止中なら起動しない。作業用terminalとアプリはUWSMの別unitなので、UIの再起動で終了させない。Hyprlandの設定変更も含むため、初回導入では新しいセッションで確認する。

旧版の `moi:desktop:vi` blockは通常のシェル編集へ置換するために同じmarkerを残している。過去のNeovim設定そのものは変更しない。greetdの `/etc` 設定は自動変更せず、tuigreetの配色引数だけを生成する。独自テーマを持つQt／ブラウザの内部配色まではGTK CSSで統一しない。

## SSHからの調査

SSH側で `HYPRLAND_INSTANCE_SIGNATURE` が未設定でも、GUIが停止しているとは限らない。実行中のUIはsystemdユーザー環境を使い、SSHクライアントの環境で上書きしない。

読み取り用の例:

```sh
hyprctl instances -j
hyprctl -i 0 configerrors
journalctl --user -u moi-desktop.service -b
```

複数のセッションがある場合、`0` は調査対象のinstanceへ合わせる。SSH環境をuser managerへ一括importしたり、調査目的で実機のコンポジターを再起動したりしない。

## 構成と検証

| 場所 | 責務 |
| --- | --- |
| `files/config/` | 編集する設定 |
| `files/ui/` | 起動管理・検索・一覧・通知・操作を分割したUI |
| `files/backend.py` | IPC、検索データ、scratchpadの状態遷移 |
| `files/render_theme.py` | 6色からCSS・Lua・SVG・Waybar設定を生成 |
| `files/theme/templates/` | 配色を含まない各形式のテンプレート |
| `files/assets/material/` | SVG原本とライセンス |
| `tests/` | 状態遷移・入力・実描画の検証 |

```sh
devcontainer exec --workspace-folder "$PWD" python3 -B -m unittest discover -s environments/desktop/modules/desktop-shell/tests -v
devcontainer exec --workspace-folder "$PWD" lua5.4 environments/desktop/modules/desktop-shell/tests/test_hyprland.lua
devcontainer exec --workspace-folder "$PWD" dbus-run-session -- python3 -B environments/desktop/modules/desktop-shell/tests/smoke_gui.py
```

GUIテストは一時XDGディレクトリと独立したheadless Swayを使う。入力・prefix候補・一覧・通知・SVG表示を確認し、Waybarはプロセスの生存だけでなく画面の画素も検査する。`MOI_DESKTOP_PREVIEW_DIR` を指定すると確認用画像を保存できる。画像内のアプリ情報はfixtureであり、ホスト画面ではない。

実機のscratchpad復旧、Hyprlandによるウィンドウ画像取得、物理キー配列、日本語変換、スリープ復帰は適用後の確認が必要。開発コンテナのHyprland候補は実機より古いため、互換性のないコンポジターで実機検証済みとは扱わない。
