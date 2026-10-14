# Developer VM

Ubuntu cloud image から共通の開発用ベースイメージを作り、個別の開発 VM は
そのベースを backing file にした qcow2 overlay として作る。

## 方針

- 重い APT package や共通ツールの導入は `develop-base.qcow2` 作成時だけ行う。
- 個別 VM 作成時には `moi` などの共通セットアップを再実行しない。
- `develop-base.qcow2` には SSH 秘密鍵、GitHub token、個人認証情報を入れない。
- `moi` の内容を変えたら既存ベースを継ぎ足し更新せず、Ubuntu cloud image から再ビルドする。
- 個別 VM ごとの hostname、libvirt UUID、MAC address、cloud-init `instance-id`、SSH 公開鍵だけを NoCloud で注入する。

## Host requirements

Debian/Ubuntu host の例:

```sh
sudo apt-get update
sudo apt-get install -y qemu-utils libvirt-daemon-system virtinst cloud-image-utils openssh-client
sudo usermod -aG libvirt "$USER"
```

`libvirt` group はログインし直すまで反映されない。

## 1. ベースイメージを作る

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/build-base.sh | bash
```

デフォルトでは以下を行う。

1. Ubuntu 24.04 cloud image を `~/.local/share/moi/developer_vm/images/ubuntu-server-cloudimg-amd64.img` に取得する。
2. 一時 VM `develop-base-build` を起動する。
3. 一時 SSH 鍵で VM にログインできるまで待つ。
4. VM 内で `curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/install.sh | bash` を実行する。
5. cloud-init state、`machine-id`、SSH host key、shell history などを初期化する。
6. VM を停止し、`~/.local/share/moi/developer_vm/images/develop-base.qcow2` を read-only にする。

`curl ... | bash` で実行するため、生成物のデフォルト置き場はリポジトリ相対ではなく
`~/.local/share/moi/developer_vm` にしている。別の場所へ置く場合は絶対パスで指定する。

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/build-base.sh \
  | DEVELOPER_VM_DIR=/var/lib/developer_vm bash
```

セットアップ元を変える場合:

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/build-base.sh \
  | MOI_INSTALL_URL=https://raw.githubusercontent.com/konattsu/moi/main/install.sh bash
```

ローカル作業ブランチを使いたい場合は、先にその内容をどこかから取得できる URL に置き、
`MOI_INSTALL_URL` で指定する。`develop-base.qcow2` には secret を焼き込まない。

## 2. 個別 VM を作る

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/create-vm.sh | bash -s -- dev01
```

これにより以下が作られる。

- `~/.local/share/moi/developer_vm/images/dev01.qcow2`: `develop-base.qcow2` を backing file にした overlay
- `~/.local/share/moi/developer_vm/seeds/dev01/seed.iso`: NoCloud 用 seed
- libvirt domain `dev01`

SSH 公開鍵を明示する場合:

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/create-vm.sh \
  | SSH_PUBKEY="$HOME/.ssh/id_ed25519.pub" bash -s -- dev01
```

MAC address を固定する場合:

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/create-vm.sh \
  | MAC_ADDRESS=52:54:00:10:20:30 bash -s -- dev01
```

IP address を固定する場合は、libvirt network の DHCP reservation として追加する。
`IP_ADDRESS` だけ指定した場合は VM 名から安定した MAC address を自動生成する。

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/create-vm.sh \
  | IP_ADDRESS=192.168.122.50 bash -s -- dev01
```

MAC と IP の両方を明示する場合:

```sh
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/create-vm.sh \
  | MAC_ADDRESS=52:54:00:10:20:30 IP_ADDRESS=192.168.122.50 bash -s -- dev01
```

起動後の IP 確認:

```sh
virsh domifaddr dev01
```

ログイン:

```sh
ssh ubuntu@<ip-address>
```

## 3. ベースを作り直す

`moi` のセットアップ内容を変えたら、個別 VM を消してからベースを再生成する。

```sh
virsh destroy dev01 || true
virsh undefine dev01 --nvram || true
rm -f ~/.local/share/moi/developer_vm/images/dev01.qcow2
rm -f ~/.local/share/moi/developer_vm/seeds/dev01/seed.iso

rm -f ~/.local/share/moi/developer_vm/images/develop-base.qcow2
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/build-base.sh | bash
curl -fsSL https://raw.githubusercontent.com/konattsu/moi/main/extras/developer_vm/create-vm.sh | bash -s -- dev01
```

## Files

- `a.txt`: 元の設計メモ
- `build-base.sh`: Ubuntu cloud image から `develop-base.qcow2` を作る
- `create-vm.sh`: ベースから個別 VM overlay と NoCloud seed を作る
- `~/.local/share/moi/developer_vm/images/`: 生成される qcow2 置き場
- `~/.local/share/moi/developer_vm/seeds/`: 生成される cloud-init seed 置き場
