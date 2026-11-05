# 実行モデル

## 読み込み、選択、検証

plan と apply は次の順で定義を確定する。

1. source と指定 environment を取得する。
2. その environment の全 module.toml を parse し、構文・型・未知キー・必須項目・名前・パスの書式を検証する。
3. 全 module の依存先の存在と循環を検証し、指定された module と推移的な依存先を選ぶ。指定がなければ全 module を選ぶ。
4. 実行先の環境を検出して、選択された操作の `when` を評価する。
5. 選択され、when に一致する操作について、静的入力の存在・配置先とその親・配置先の競合を検証する。

未選択 module や条件不一致の操作も定義として正しい必要があるが、その src の存在や配置先の状態は要求しない。他の environment は読み込まない。全ての事前検証を終えてから system package の導入と module の変更を始める。

file / block の src は取得した repository 内の静的ファイルとする。先行処理による生成・変更を前提とせず、変更開始前に存在と読み取りを確認する。生成から配置までを行う独自処理は shell にまとめる。

src は module 基準で解決し、共有する静的入力への symlink を許可する。実体が取得した repository 内の通常ファイルであることを確認し、内容とコピーに使う権限を変更開始前に読み取って保持する。

## 環境条件

実行先から distro 系統、CPU architecture、WSL を検出する。distro は `debian` / `arch`、architecture は `x86_64` / `aarch64` に正規化する。検出不能・非対応の実行先は変更前にエラーにする。配布元の `arm64` などの表記は url や root の具体的な値に閉じ込め、when.arch には正規化した名称を使う。

```toml
[[operations]]
kind = "shell"
when = { wsl = true, arch = ["x86_64"] }
check = "test -x ~/.local/bin/win32yank.exe"
run = "bash files/install-win32yank.sh"
```

`when` のキーは distro、arch、wsl。distro と arch は空でない配列、wsl は真偽値。各キーを AND、配列の要素を OR として扱い、省略した軸には制約を置かない。

条件不一致は正常な対象外とし、状態判定も変更も行わない。archive も when だけで適用条件を評価する。条件を省略した軸は制約なしとし、URL などから適用先を推測しない。異なる distro などで排他的に適用する操作は、同じ配置先を宣言できる。

## 配置先と管理範囲

HOME を展開し、重複した区切りと `.` 要素を除き、ルート以外の末尾の `/` を除いた絶対パスで対象を比較する。`..` は許可しない。標準操作の配置先の親に symlink があれば拒否する。未作成の親は作成できるが、選択された適用対象の操作が親を symlink にする構成も拒否する。操作直前にも親と対象の種類を確認する。

file / directory / block / archive は管理対象自身の symlink をたどらない。symlink 操作はリンク自身を管理し、target の存在は要求しない。配置先の親に関する制限は、入力の共有 module の symlink や archive 内の安全な相対リンクとは別の規則である。

競合検証は、選択され、今回の when に一致する操作同士に行う。同じ対象全体の重複管理、file と block の併用、同一 marker の block を拒否する。

archive は include 省略時に dst 配下全体、指定時に dst と各 include を結合した最終配置ファイルだけを管理する。管理範囲の重複や、全体配置の内部への別操作の書き込みを拒否する。同じ dst の選択配置でも、ファイルが重ならなければ共存できる。親を通常ディレクトリとして用意する directory 操作とその配下の配置は共存できるが、ファイルや symlink を親とする配置は拒否する。

include の書式・正規化後の重複・ファイルの親子関係と、選択配置の required_paths が include の部分集合であることは変更前に検証する。配布物内での root・include の存在と種類は、導入が必要な場合の取得・展開後に検証する。plan はそのための download を行わない。

## apply の順序と状態

system package は選択 module から集約して重複を除き、module 操作より先に導入する。module は依存先を先に、実行可能な候補の名前の辞書順で選ぶ。module 内は operations の記述順とする。

標準操作は直前に状態を評価する。

1. when 不一致なら `not-applicable` とする。
2. 充足なら `unchanged`、判定エラーなら失敗とする。
3. 未充足の場合だけ変更に必要な前提を確認し、変更する。
4. 再度状態を確認し、充足した場合だけ `changed` とする。

権限不足、壊れた管理記録、読み取り失敗を未充足として変更を始めない。download・展開など変更時だけに必要な前提は、導入済みの状態確認では要求しない。

結果は `changed`、`unchanged`、`not-applicable`、`executed`、`failed` に分ける。shell の成功は変更有無不明の `executed` とし、標準操作の変更件数と分けて集計する。失敗時は後続を止め、それまでの変更は保持する。全体の原子性や rollback は保証しない。

system package の操作だけ必要に応じて sudo を使い、他の標準操作は実行ユーザーの権限で行う。独自の shell に必要な昇格はその定義に書く。

## shell の起動と状態判定

各 check / run / path_export は module ディレクトリを作業ディレクトリとして、独立した非対話・非ログインの bash で起動する。ユーザー設定ファイルの自動読み込みに頼らず、`BASH_ENV` と export された Bash 関数は引き継がない。環境変数を介して起動オプションが暗黙に変わらないようにし、run と path_export では `errexit` / `pipefail` を有効にする。

`run` は必須の非空文字列。`check` は省略可能な非空文字列で、次の終了コードを使う。

| check の終了コード | 意味 |
| --- | --- |
| `0` | 充足 |
| `1` | 未充足 |
| その他・シグナル終了・起動失敗 | 判定エラー |

check は副作用を持たせず、判定用コマンドの失敗を未充足の `1` に丸めない。必要なら script 内で判定エラーへ明示的に変換する。moi は script 内部の意味を解析しない。

check が未充足なら run を実行し、成功後に check を再評価する。充足しなければ操作失敗。check を省略すると apply ごとに run を実行する。run の非ゼロ終了は失敗とする。check による実行省略は unchanged、run の実行成功は executed とする。

通常の shell 操作による環境変数、関数、作業ディレクトリなどの変更は他の操作へ伝播しない。汎用のコマンド存在リストや、コマンド不足に応じた特定ツールの初期化は持たない。

## PATH の取得と引き継ぎ

起動時の PATH を基礎に、module の実行順に次を行う。

1. module の開始時に `env.path_prepend` を先頭へ加える。
2. その PATH で module の操作を実行する。
3. 全操作の成功後、`env.path_export` があれば同じ PATH を入力として取得用の bash を一度起動する。操作が空、全て充足、全て対象外の場合も起動する。
4. 取得に成功した場合だけ PATH を置き換え、後続 module の子プロセスへ渡す。省略時は手順1の PATH を保持する。

path_prepend は存在しないディレクトリも許可する。同じパスは最初の出現だけを残す。取得後の PATH は依存する module に限らず後続全体へ渡し、後続の宣言がさらに更新する。利用側は偶然の実行順に頼らず depends_on を宣言する。

path_export は apply ごとに実行し、結果を永続化しない。自 module の操作や、それ以前の system package 導入へ遡って適用しない。

取得用 script は導入済み環境の読み込みと PATH の取得だけを行い、導入・更新・管理対象への書き込みを行わない。stdout は取得専用で、PATH 全体の UTF-8 文字列に NUL を一つ付けて出力する。ログや読み込む script の通常出力は stderr へ送る。

終了コード 0 と、末尾の NUL 一つで区切られた非空の出力を検証する。各 PATH 要素は空でない絶対パスとし、制御文字を拒否する。存在確認や shell 式の展開は行わず、重複は最初だけ残す。保持したい入力の PATH は script 側で含める。

非ゼロ終了、シグナル終了、起動失敗、出力不正は環境取得エラーとし、PATH を更新せず後続を止める。終了コード 1 も未充足として扱わない。診断には提供側 module と env.path_export、失敗理由、stderr を添える。取得は状態変更ではなく、変更件数・shell 実行件数には含めない。

次の例は、node module の operations で nvm、Node、default alias を用意した後、その PATH を提供する宣言である。

```toml
name = "node"

[env]
path_export = '''
{
  export NVM_DIR="$HOME/.nvm"
  . "$NVM_DIR/nvm.sh" --no-use
  nvm use --silent default
} >&2
printf '%s\0' "$PATH"
'''
```

利用側は depends_on に node を指定し、通常の check / run に npm の処理を書く。nvm の関数自体は渡さず、選択された Node の bin ディレクトリを含む PATH だけを渡す。起動時の PATH や path_prepend だけで足りる定義では path_export を省略する。

取得した PATH は以後の子プロセスに設定する。呼び出し元の端末や次回の moi 実行へ書き戻さず、PATH 以外の環境変数・関数・alias・作業ディレクトリも取り込まない。

## archive の導入と維持

archive は具体的な version・url・sha256・format・root・dst を operations に直接宣言する。共通の配布物は一度だけ書き、CPU 別の配布物だけ when.arch で分ける。latest は定義更新時に具体化する。

include 省略時は root 全体を専用 dst へ配置し、指定時は列挙した通常ファイルだけを dst 配下へ直接配置する。選択配置は共有の dst 全体を所有しない。各管理対象の絶対パスを識別に使い、全体配置はディレクトリごと、選択配置はファイルごとの JSON に記録する。

まず version・sha256・format・root・include・policy と適用済みの記録を比較する。異なれば現在の宣言から再導入し、成功後に記録を更新する。URL だけの変更や include の順序は再導入の理由にせず、アプリへの版の問い合わせや大小比較も行わない。

定義が一致する場合、pinned は管理する内容を照合する。全体配置では余分なファイルも差分にするが、選択配置では他のファイルを検査・変更しない。preserve は required_paths の条件だけを確認し、内容変更を許す。充足なら取得・解凍・再配置せず、定義変更・修復時の downgrade は許容する。

再配置では検証済みの管理対象だけを退避・差し替え、配置確認と操作内の全記録の保存成功後に旧対象を削除する。共有ディレクトリ全体を退避・置換せず、include から外れたファイルも自動削除しない。中断からの自動復旧は行わない。

保存形式、選択範囲、照合と更新順序は [archive の配置と管理](archive.md) に従う。本体と設定・desktop entry は別操作とし、必要な起動リンクは symlink、font cache などの後処理は shell で扱う。archive に任意の前後フックは設けない。

## plan、note、同時実行

plan は管理対象へ書き込まず、package manager、check、run、path_export を起動しない。source の一時 clone は定義取得のために許可する。副作用なく確認できるファイル内容や管理記録だけを調べる。

操作ごとに module・配列位置・kind、対象環境、望む状態、配置先、充足・変更予定・対象外の理由を表示する。shell 判定や動的 PATH などは実行時に判定・取得すると表示する。path_export 後の PATH を確定値として扱わず、現在の PATH にコマンドがないことを理由に失敗させない。package の実変更や shell の副作用も確定しない。

apply は各操作の直前に再評価し、plan の表示を実行許可や固定済み計画として再利用しない。

`--show-note` 指定時だけ、選択 module の note を module の実行順に stderr へ表示する。初回・実行履歴による自動表示はしない。

同一ユーザーによる apply の同時実行は非対応とする。ロックや重複実行の検出は行わず、その場合の整合性・復旧は保証しない。plan は apply と並行して利用できるが、観測結果が apply の途中状態となる場合がある。
