# moi の仕様

moi は Linux の個人環境を module 単位で整える。定義には望む状態と適用条件を書き、moi が現在の状態を確認して必要な変更を行う。標準操作で表せない手順は shell に任せる。

この文書は機能と管理範囲を定める。実行順序・状態判定・失敗時の契約は [実行モデル](execution.md)、archive の記録と再配置は [archive の配置と管理](archive.md)、全キーは [TOML リファレンス](toml-reference.md)、設計理由は [ADR](adr.md)、構造の検証規則は [Schema](../schemas/README.md) に記す。

## 実行対象と権限

moi は環境を整える対象のユーザーとして実行する。HOME、設定ファイル、配置先、archive の管理記録は実行ユーザーのものを使い、sudo 元のユーザーを推測して切り替えない。

標準操作は実行ユーザーの権限で行い、system package の導入・更新だけ必要に応じて sudo で昇格する。独自の shell が管理者権限を必要とする場合は、定義側で sudo を明記する。

同一ユーザーによる複数の apply の同時実行はサポートしない。重複実行を検出するロックは設けず、同時実行時の結果の整合性や復旧を保証しない。

## environment と module

environment は module の集合。一つの実行先には一つの environment を使う運用を想定する。module は `<folder_name>/<environment>/modules/<name>/module.toml` に置き、`name` をディレクトリ名と一致させる。共有 module はディレクトリへの symlink でも取り込める。

指定された environment の全 module を読み、構文・型・未知キー・名前・パスの書式・依存先の存在・循環を検証する。未選択 module の定義不備もエラーにする。他の environment は検証しない。

module 名の指定がなければ全 module を選ぶ。指定があれば、その module と `depends_on` の推移的な依存先を選ぶ。依存先を先に実行し、順序が制約されない候補は名前の辞書順で選ぶ。ある module に依存している利用側を逆向きに選ぶことはない。

`depends_on` は実行に必要な module の宣言であり、外部コマンドの一覧ではない。選択を伴わない順序指定や全体の phase は設けない。module 内は `operations` の記述順に実行し、操作間の依存グラフは持たない。

```toml
name = "editor"
depends_on = ["core"]
note = ["エディタを起動して認証する"]

[packages]
apt = ["ripgrep"]
pacman = ["ripgrep"]

[env]
path_prepend = ["~/.local/bin"]

[[operations]]
kind = "file"
src = "files/init.lua"
dst = "~/.config/editor/init.lua"
mode = "644"
```

## 標準操作

| kind | 宣言するもの | 充足している状態 |
| --- | --- | --- |
| `directory` | `path`、任意の `mode` | ディレクトリがあり、指定した権限を満たす |
| `file` | `src`、`dst`、任意の `mode` | 通常ファイルの内容と指定した権限が一致する |
| `symlink` | `target`、`dst` | リンク先が宣言と一致する |
| `block` | `src`、`dst`、`marker` | 一意な marker 内の内容が一致する |
| `archive` | 成果物、配置先、維持方針 | 配置内容が維持方針を満たす |
| `shell` | `run`、任意の `check` | check の契約に従う |

各操作に `kind` を付ける。診断では module 名、1始まりの配列位置（`operations[1]`）、kind、対象パスを使う。操作に名前や一意な ID は要求しない。共通の適用条件 `when` により、distro・architecture・WSL を指定できる。

ディレクトリとファイルの親は必要に応じて作る。`mode` は3〜4桁の8進数文字列とし、owner / group は管理しない。`file` は既存ファイル全体を置き換え、`block` は次の marker の内側だけを管理する。内容が一致する操作は書き換えない。

```text
# >>> moi:example >>>
# <<< moi:example <<<
```

marker の欠落した対、重複、逆順はエラーとし、推測で修復しない。marker は空文字列・制御文字・区切り記号を含めない。

選択され、今回の `when` に一致する操作同士について、同じ対象全体の重複管理、file と block の併用、archive の管理範囲の重複・包含、所有範囲への別操作の書き込みを変更開始前に拒否する。同じファイルへの block は marker が異なれば共存できる。symlink はリンク自身を管理し、リンク先の所有権を持たない。

## パスと入力

配置先と `env.path_prepend` は絶対パスか `~` / `~/...` とする。`~` は実行ユーザーの HOME に展開する。環境変数や shell 式は展開しない。パスの詳細な形式は TOML リファレンスに従う。

標準操作の配置先は、親ディレクトリに symlink を含めない。管理対象自身も宣言した種類と一致させ、file / directory / block / archive が symlink をたどって変更することは許可しない。symlink 操作だけがリンク自身を作成・変更する。この制限は書き込み先の規則であり、共有 module の symlink や、symlink 操作の target の存在を制限するものではない。

`file.src` と `block.src` は module 基準の相対パスで、取得した repository 内の静的な通常ファイルを参照する。絶対パスと `..` を拒否する。先行 shell が生成・変更するファイルを標準操作の入力には使わない。生成が必要なら生成から配置まで shell 内で行う。

src は module を基準に解決する。共有する静的入力への symlink は許可するが、リンク解決後の実体は取得した repository 内の通常ファイルに限定する。配置先の親 symlink を禁止する規則は入力側には適用しない。

## 実行環境と PATH

起動時の環境変数を継承する。shell は非対話・非ログインの `bash -c` とし、ユーザーの shell 設定は自動で読み込まない。`BASH_ENV` と export された Bash 関数は子プロセスへ引き継がない。必要な読み込みは module に明記する。

静的な PATH は `env.path_prepend` で宣言し、自 module の開始時に先頭へ加えて後続に引き継ぐ。導入後に PATH を解決する場合は、提供側 module に `env.path_export` を宣言する。全操作の成功後に一度取得し、以後の子プロセスへ渡す。操作がすべて充足していた場合も取得する。

`path_export` は宣言されていれば毎回実行する。起動時の PATH や静的な宣言だけで足りる定義では省略できる。

利用側は `depends_on` を宣言して通常のコマンドを実行する。ツール固有の初期化は提供側に定義する。PATH は moi が保持して子プロセスごとに設定し、呼び出し元の端末へは書き戻さない。

## パッケージと archive

選択 module の `packages.apt` または `packages.pacman` を集約して重複を除き、module 操作より先に導入する。Debian 系は apt、Arch 系は pacman を使う。通常の apply は不足分を導入し、システム全体の更新は `apply --upgrade-packages` で明示する。

`packages` は module 操作前に導入可能なパッケージに使う。追加リポジトリ・署名鍵の登録が必要なものは、登録から package 導入まで shell で行う。module の依存順によって package の一括導入を遅らせることはない。package manager の依存解決や更新結果を moi 独自の状態として複製しない。

archive は url・sha256・format・root・dst と維持の項目を `[[operations]]` に直接書く。適用条件は when に統一し、architecture 共通なら when.arch を省略、異なる場合だけ操作を分ける。

include 省略時は root 全体を専用 dst へ配置し、そのディレクトリ全体を管理する。include 指定時は root 内の通常ファイルを完全一致で選び、dst 配下の同じ相対位置へ直接配置する。管理対象は選択したファイルだけとし、共有の dst 全体は変更・削除しない。glob・除外・リネーム・内容加工は扱わない。

両 policy とも、version・配布物・include・policy が最後に導入成功した定義と異なれば再導入する。version は最低版の指定ではなく、実際のアプリの版の取得・大小比較は行わない。

pinned は定義が同じ場合も管理範囲の配布内容に収束させる。全体配置では余分なファイルも差分とし、選択配置では選択外のファイルを扱わない。preserve は定義が同じ間、required_paths の存在・種類・必要な実行権限だけを確認して内容変更を許す。選択配置の required_paths は include の部分集合にする。定義変更・修復時の downgrade は許容し、どちらも起動成功そのものは保証しない。

管理対象は正規化した絶対パスで識別し、module 名や environment 名を所有者にしない。管理記録は全体配置の専用ディレクトリごと、選択配置の最終配置ファイルごとに、管理対象パスの SHA-256 をファイル名とする JSON として ~/.local/state/moi/archives/ に保存する。共有 dst が同じでも選択ファイルが違えば記録は衝突しない。

記録のない既存対象は引き取らない。再配置では管理対象だけを退避・差し替え、配置確認と操作内の全記録の保存成功後に旧対象を削除する。include から外した対象の自動削除、中断復旧用の記録、自動復旧は設けない。詳細は [archive の配置と管理](archive.md) に従う。

## CLI と設定

```sh
moi -e host plan [--show-note] [module ...]
moi -e host apply [--upgrade-packages] [--show-note] [module ...]
moi install-command -e host [plan|apply] [arg ...]
moi upgrade [--force]
```

plan と apply は同じ定義・選択・検証規則を使う。環境は実行先から検出し、別環境への適用を装う上書きオプションは設けない。

`note` は選択 module の案内であり、plan / apply に `--show-note` を指定したときだけ表示する。初回かどうかにかかわらず、既定では表示しない。案内に書かれた処理は実行せず、表示履歴や初回導入の記録も持たない。

設定ファイルは `~/.config/moi/config.toml`。読み込むだけでは生成・更新しない。値の優先順位は CLI、環境変数、設定ファイル、既定値の順とする。

| CLI | 環境変数 | 設定キー | 既定値 |
| --- | --- | --- | --- |
| `-e`, `--environment` | `MOI_ENVIRONMENT` | `default_environment` | 必須 |
| `--folder-name` | `MOI_FOLDER_NAME` | `default_folder_name` | `environments` |
| `--source` | `MOI_SOURCE` | `default_source` | `https://github.com/konattsu/moi.git` |
| `--install-source` | `MOI_INSTALL_SOURCE` | `default_install_source` | `https://raw.githubusercontent.com/konattsu/moi/main/` |
| `--install-script` | `MOI_INSTALL_SCRIPT` | `default_install_script` | `install.sh` |

`source` は HTTPS の Git repository または `file:///` のローカル checkout。HTTPS は一時領域へ clone し、終了時に片付ける。branch は `MOI_BRANCH`、既定値は `main`。folder と environment は repository 内に収まる相対パスとし、environment と module 名は単一のパス要素とする。

`install-command` は明示された environment を使う導入コマンドを stdout に出す。操作の既定値は apply。install source は HTTPS またはローカルディレクトリ、install script はその内側の相対パスとする。module の読み込みや導入は行わない。

`install.sh` と `upgrade` は実行先に合う moi の release archive と SHA-256 を取得し、検証して binary を配置する。対応する Linux target は x86_64 / aarch64 の GNU 環境。`upgrade` は新しい release に更新し、`--force` は同版の再配置を許可する。binary の置換は同じディレクトリ上の一時ファイルから行う。

## 出力と管理範囲

plan と導入コマンドは stdout、進捗・診断・子プロセス出力・明示された note は stderr に出す。`--quiet` は通常出力を抑えるが失敗理由は残し、`-v` は診断を増やす。見た目や表示行数は動作契約に含めない。

終了コードは成功 `0`、定義・判定・I/O などの失敗 `1`。変更処理の子プロセスが失敗した場合はその終了コードを返し、シグナル終了など数値が得られない場合は `1` とする。失敗には module、操作の配列位置と kind、失敗した段階、原因、子プロセスの出力を添える。

失敗時は後続を止め、それまでの変更は保持する。宣言から消した対象の自動削除、汎用 rollback、任意 shell の副作用の解析、並行 apply の調整は扱わない。中断後に管理記録と配置が一致せず通常の状態判定で扱えない場合は、利用者が退避・整理して再実行する。
