# Cording rules

## rust

### the use of `use`

ファイル先頭, モジュール先頭で`use`を使用してはいけない。
トレイト, 構造体, 関数などに関わらず`use`してはいけない。
どこ由来なのか分からないのでフルパス記法を基本的に用いる。
但し例外が3つある。

- 単体テスト用のtestsモジュール
  - `use super::*`, `use ...`など好きにして
  - `tests/`など単体テストでない統合テストなど用のモジュールでは禁止
- 関数内
  - 関数内の先頭では`use`していい
    - 但し全部`use`しまくるのではなく, `use`したほうが見やすくなるならして
    - 関数内で同一の型が3回以上出てきたら`use`の使用を積極的に検討すること
  - だが, `use foo::*`のようなワイルドカードは禁止
- module分割用のファイル(条件付き)
  - `mod foo`, `pub mod bar`などのモジュール分割しているファイル
  - 条件
    - `mod foo`, `pub use foo::Foo`のように, モジュール分割してかつそのファイルをre-exportしたいときに限る
    - 但し`pub mod foo`, `pub use foo::Foo`のようにモジュール公開かつ, re-exportの形は避ける
  - だが, `use foo::*`のようなワイルドカードは禁止

`std::result::Result`のような`std::prelude`に含まれるものもフルパス記法しろということではない.
`std::prelude`に含まれるものはそのまま使ってよい(e.g. Copy, Result, Into, TryFrom, Future, etc.).

### the range of visibility

- `pub`を使うときは`pub(crate)`, `pub(super)`, `pub(in path)`などより狭い範囲で公開できないか検討する
- また構造体フィールドを公開するときは, 外部から間違えて不正な値が入らないか, そもそも公開する必要があるのかを十分に検討すること
  - getter/setterをすべてに付与するのではなく公開するのが適切な場合ももちろんある
  - getterのみの付与, またはフィールド公開の二つで悩むならgetter付与の方が良い

### run scripts

format, testなどは気にせずガンガン実行して
手動で直すより信頼性も速度も速いので何度でも実行して

### `mod.rs`

`mod.rs`は古い書き方で, もう使ってはいけない
ディレクトリ名と同名のファイルを作るのが新しい書き方

### consider init pattern instead of builder pattern

**不特定多数が多く利用する公開apiを開発しているのでない限り**`builder patten`を採用しない方が良い
これは互換性を保ちやすくするための物であり, 必須の値が分かりにくいしコードが冗長になりやすい.
引数が少なければ逆に単一の関数にするのもよく, 引数が多ければ`init pattern`を採用すべきことが多い.

### dependencies

`Cargo.toml`には導入時点で最新のversionを省略せずに記載すること.
例えばSemVerならpatchまで記載.
毎回確認しなくてよく, 新規追加のときだけでいい.
