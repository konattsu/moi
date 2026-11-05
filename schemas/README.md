# TOML Schema

TOML を解析したオブジェクトを JSON Schema Draft 2020-12 で検証する。全キーの意味と省略時の扱いは [TOML リファレンス](../docs/toml-reference.md)、検証の時点と範囲は [実行モデル](../docs/execution.md#読み込み選択検証) に従う。

- [module.schema.json](module.schema.json): `<folder>/<environment>/modules/<name>/module.toml`
- [config.schema.json](config.schema.json): `~/.config/moi/config.toml`

これらは入力定義用の Schema であり、archive の管理記録には適用しない。管理記録の JSON、保存先、照合と再配置の契約は [archive の配置と管理](../docs/archive.md) に従う。

module の必須項目は name。未知のキーは全階層で拒否する。省略時は各キーの契約に従い、個別の指定がなければ配列・テーブルは空として扱う。文字列は非空とする。note は文字列の配列で、表示履歴や初回判定は持たない。

archive は url・sha256・format・root を操作自身の必須項目として持つ。include は省略時に全体配置、指定時に完全一致の通常ファイルの選択配置を意味する。指定する配列は非空・重複不可。各要素は root 内の相対パスとし、root 自身は選択しない。

preserve は required_paths が必須、pinned では指定不可。選択配置の required_paths が include の部分集合であることや、正規化後の重複・親子関係は moi の意味の検証とする。選択した項目が配布物内の通常ファイルであることは導入時に確認する。

両 policy とも適用済みの version・配布物・include・policy が変われば再導入する。管理記録は全体配置なら専用ディレクトリ、選択配置なら各ファイルの絶対パスで分ける。条件は when のみで、未指定の軸は制約なしとする。

エディタで Schema を対応付ける場合は、module.toml の先頭に次のコメントを置く。パスはファイルの配置に合わせる。

```toml
#:schema ../../../../../schemas/module.schema.json
```

Schema は構文解析後の型・必須項目・列挙値・文字列形式を検証する。moi は指定 environment 全体にこの構造検証と依存関係の検証を行う。HOME 展開後の同一性などは実行器の意味の検証とする。

| 対象 | moi が検証するもの |
| --- | --- |
| 指定 environment の全 module | module 名とディレクトリ名の一致、依存先の存在・循環、パスの解決範囲 |
| 選択され when に一致する操作 | 静的 src の存在・読み取り、配置先の親が symlink でないこと、正規化後の管理範囲の競合、include と required_paths の関係 |
| 状態判定・導入時 | 配置内容と管理記録、required_paths の存在・種類・実行権限、archive の checksum・内部パス・リンク、選択する通常ファイルの存在、shell の終了コード、path_export の出力 |

配置先の親 symlink の禁止は filesystem の検証であり、Schema の文字列検証だけでは保証しない。共有 module の symlink は許可する。URI の pattern は入口の形式の検査であり、完全な URL 解析は moi が行う。
