# C / C++ project の devcontainer

`moi` の devcontainer environment は Neovim の設定を担当し、compiler、LSP、formatter、debugger、build system は各 project の devcontainer が担当する。

小規模から中規模の C / C++ project では、公式の C++ image を土台にして、Neovim が直接利用する `clangd` と `clang-format` を明示的に追加する構成を推奨する。

## 推奨構成

project root に次のファイルを置く。

```text
.
├── .clang-format
├── .devcontainer
│   ├── Dockerfile
│   └── devcontainer.json
├── CMakeLists.txt
└── CMakePresets.json
```

### `.devcontainer/devcontainer.json`

```jsonc
{
  "name": "c-project",
  "build": {
    "dockerfile": "Dockerfile",
    "context": ".."
  },
  "remoteUser": "vscode"
}
```

GDB や LLDB から `ptrace` が必要な場合だけ、次を追加する。
container の分離を弱めるため、debugger を使わない project では追加しない。

```jsonc
{
  "capAdd": ["SYS_PTRACE"],
  "securityOpt": ["seccomp=unconfined"]
}
```

上の断片は既存の top-level object に統合する。

### `.devcontainer/Dockerfile`

```dockerfile
FROM mcr.microsoft.com/devcontainers/cpp:3-bookworm

RUN apt-get update \
    && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install --yes --no-install-recommends \
        clang-format \
        clang-tidy \
        clangd \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*
```

公式 C++ image が compiler、CMake、Ninja、GDB、LLDB などを提供する。
`clangd` と `clang-format` は Neovim の LSP と format on save が確実に利用できるよう、project image で明示する。

`3-bookworm` は image の major versionとOSを固定し、同じmajorの更新を受け取る指定である。
完全な再現性が必要なら image digest まで固定し、Dependabotなどで定期更新する。

### `CMakePresets.json`

`clangd` が実際の include path、define、compile option を認識できるよう、compilation database を生成する。

```json
{
  "version": 6,
  "configurePresets": [
    {
      "name": "dev",
      "generator": "Ninja",
      "binaryDir": "${sourceDir}/build/dev",
      "cacheVariables": {
        "CMAKE_BUILD_TYPE": "Debug",
        "CMAKE_EXPORT_COMPILE_COMMANDS": true
      }
    }
  ],
  "buildPresets": [
    {
      "name": "dev",
      "configurePreset": "dev"
    }
  ],
  "testPresets": [
    {
      "name": "dev",
      "configurePreset": "dev",
      "output": {
        "outputOnFailure": true
      }
    }
  ]
}
```

configure 後、project root から compilation database を参照できるようにする。

```sh
cmake --preset dev
ln -sfn build/dev/compile_commands.json compile_commands.json
cmake --build --preset dev
ctest --preset dev
```

生成物の `build/` と `compile_commands.json` は通常 `.gitignore` に入れる。

### `.clang-format`

format 規約は個人の Neovim 設定ではなく、project に commit する。

```yaml
BasedOnStyle: LLVM
IndentWidth: 4
ColumnLimit: 100
DerivePointerAlignment: false
PointerAlignment: Left
SortIncludes: CaseSensitive
```

これは出発点の例なので、既存 project ではその規約を優先する。

## 運用方針

- compiler、`clangd`、`clang-format`、debugger、CMakeなどは project の devcontainer image に置く
- `.clang-format`、compiler warning、C/C++ standard、依存libraryは project に置く
- `moi` は container 内の Neovim 設定だけを配置する
- project で共有する `.devcontainer` に個人用の `moi` install command を追加しない
- devcontainer に入った後、必要な利用者だけ `moi` の devcontainer environment を適用する
- CI でも同じ CMake preset と warning 設定を使い、エディタだけに検査を依存させない

## さらに小さい構成

Dockerfile に追加するpackageが不要なら、`devcontainer.json` から公式 imageを直接参照できる。

```jsonc
{
  "name": "c-project",
  "image": "mcr.microsoft.com/devcontainers/cpp:3-bookworm",
  "remoteUser": "vscode"
}
```

ただしこの構成を採用する前に、container 内で `clangd` と `clang-format` が利用できることを確認する。
不足する場合は推奨構成の Dockerfile に戻し、project の依存として明示する。

## 参考

- [Dev Container C++ image](https://github.com/devcontainers/images/tree/main/src/cpp)
- [公式 C++ template](https://github.com/devcontainers/templates/tree/main/src/cpp)
- [Dev Container specification](https://containers.dev/)
