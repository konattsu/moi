#!/usr/bin/env bash
set -euo pipefail

data_dir="${DEVELOPER_VM_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/moi/developer_vm}"
image_dir="${IMAGE_DIR:-$data_dir/images}"
seed_dir="${SEED_DIR:-$data_dir/seeds/base-build}"

ubuntu_image_url="${UBUNTU_IMAGE_URL:-https://cloud-images.ubuntu.com/noble/current/noble-server-cloudimg-amd64.img}"
pristine_image="${PRISTINE_IMAGE:-$image_dir/ubuntu-server-cloudimg-amd64.img}"
base_image="${BASE_IMAGE:-$image_dir/develop-base.qcow2}"
build_image="${BUILD_IMAGE:-$image_dir/develop-base-build.qcow2}"

vm_name="${VM_NAME:-develop-base-build}"
vm_user="${VM_USER:-ubuntu}"
memory="${MEMORY:-4096}"
vcpus="${VCPUS:-2}"
disk_size="${DISK_SIZE:-40G}"
moi_install_url="${MOI_INSTALL_URL:-https://raw.githubusercontent.com/konattsu/moi/main/install.sh}"
ssh_key="${SSH_KEY:-$seed_dir/id_ed25519}"
ssh_opts=(-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5 -i "$ssh_key")

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: missing required command: $1" >&2
    exit 1
  }
}

cleanup_domain() {
  if virsh dominfo "$vm_name" >/dev/null 2>&1; then
    virsh destroy "$vm_name" >/dev/null 2>&1 || true
    virsh undefine "$vm_name" --nvram >/dev/null 2>&1 || true
  fi
}

wait_for_ip() {
  local ip=""
  for _ in $(seq 1 120); do
    ip="$(virsh domifaddr "$vm_name" --source agent 2>/dev/null | awk '/ipv4/ { sub("/.*", "", $4); print $4; exit }')"
    if [[ -z "$ip" ]]; then
      ip="$(virsh domifaddr "$vm_name" --source lease 2>/dev/null | awk '/ipv4/ { sub("/.*", "", $5); print $5; exit }')"
    fi
    if [[ -n "$ip" ]]; then
      printf '%s\n' "$ip"
      return 0
    fi
    sleep 5
  done
  return 1
}

wait_for_ssh() {
  local ip="$1"
  for _ in $(seq 1 120); do
    if ssh "${ssh_opts[@]}" "$vm_user@$ip" true >/dev/null 2>&1; then
      return 0
    fi
    sleep 5
  done
  return 1
}

main() {
  local ip state

  require_cmd cloud-localds
  require_cmd curl
  require_cmd qemu-img
  require_cmd ssh
  require_cmd ssh-keygen
  require_cmd virt-install
  require_cmd virsh

  mkdir -p "$image_dir" "$seed_dir"

  if [[ -e "$base_image" ]]; then
    echo "error: base image already exists: $base_image" >&2
    echo "remove it explicitly before rebuilding from the pristine Ubuntu image" >&2
    exit 1
  fi

  if [[ ! -f "$pristine_image" ]]; then
    curl -fL "$ubuntu_image_url" -o "$pristine_image"
  fi

  cleanup_domain
  rm -f "$build_image" "$seed_dir/seed.iso" "$ssh_key" "$ssh_key.pub"
  ssh-keygen -t ed25519 -N '' -f "$ssh_key" -C "$vm_name" >/dev/null

  cp "$pristine_image" "$build_image"
  qemu-img resize "$build_image" "$disk_size"

  cat >"$seed_dir/user-data" <<EOF
#cloud-config
users:
  - name: $vm_user
    groups: [adm, sudo]
    shell: /bin/bash
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - $(<"$ssh_key.pub")
package_update: true
packages:
  - curl
  - git
  - openssh-server
  - python3
  - qemu-guest-agent
runcmd:
  - systemctl enable --now qemu-guest-agent
EOF

  cat >"$seed_dir/meta-data" <<EOF
instance-id: $vm_name-$(date +%s)
local-hostname: $vm_name
EOF

  cloud-localds "$seed_dir/seed.iso" "$seed_dir/user-data" "$seed_dir/meta-data"

  virt-install \
    --name "$vm_name" \
    --memory "$memory" \
    --vcpus "$vcpus" \
    --disk "path=$build_image,format=qcow2,bus=virtio" \
    --disk "path=$seed_dir/seed.iso,device=cdrom" \
    --os-variant ubuntu24.04 \
    --import \
    --network network=default,model=virtio \
    --graphics none \
    --noautoconsole

  ip="$(wait_for_ip)" || {
    echo "error: timed out waiting for VM IP" >&2
    cleanup_domain
    exit 1
  }
  wait_for_ssh "$ip" || {
    echo "error: timed out waiting for SSH on $ip" >&2
    cleanup_domain
    exit 1
  }

  ssh "${ssh_opts[@]}" "$vm_user@$ip" "curl -fsSL '$moi_install_url' | bash"

  ssh "${ssh_opts[@]}" "$vm_user@$ip" "sudo bash -s" <<'EOF'
set -euo pipefail
cloud-init clean --logs || true
rm -rf /var/lib/cloud/instances/* /var/lib/cloud/instance
truncate -s 0 /etc/machine-id
rm -f /var/lib/dbus/machine-id
ln -sf /etc/machine-id /var/lib/dbus/machine-id
rm -f /etc/ssh/ssh_host_*
find /home -maxdepth 2 -name .bash_history -delete
find /root -maxdepth 1 -name .bash_history -delete
apt-get clean
sync
poweroff
EOF

  for _ in $(seq 1 120); do
    state="$(virsh domstate "$vm_name" 2>/dev/null || true)"
    [[ "$state" == "shut off" ]] && break
    sleep 5
  done
  if [[ "$state" != "shut off" ]]; then
    echo "error: timed out waiting for VM shutdown" >&2
    exit 1
  fi

  virsh undefine "$vm_name" --nvram >/dev/null 2>&1 || true
  mv "$build_image" "$base_image"
  chmod 0444 "$base_image"
  echo "created $base_image"
}

main "$@"
