#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: create-vm.sh <vm-name>

Environment:
  BASE_IMAGE   Path to develop-base.qcow2
  DEVELOPER_VM_DIR
               Base data directory, default ~/.local/share/moi/developer_vm
  IMAGE_DIR    Directory for VM overlay images
  SEED_DIR     Directory for cloud-init seeds
  SSH_PUBKEY   SSH public key to inject
  NETWORK_NAME libvirt network name, default default
  MAC_ADDRESS  MAC address to assign
  IP_ADDRESS   DHCP-reserved IPv4 address on NETWORK_NAME
  MEMORY       VM memory MiB, default 4096
  VCPUS        VM vCPU count, default 2
  DISK_SIZE    Overlay virtual size, default 40G
EOF
}

data_dir="${DEVELOPER_VM_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/moi/developer_vm}"
image_dir="${IMAGE_DIR:-$data_dir/images}"
seed_root="${SEED_DIR:-$data_dir/seeds}"
base_image="${BASE_IMAGE:-$image_dir/develop-base.qcow2}"
ssh_pubkey="${SSH_PUBKEY:-$HOME/.ssh/id_ed25519.pub}"
network_name="${NETWORK_NAME:-default}"
mac_address="${MAC_ADDRESS:-}"
ip_address="${IP_ADDRESS:-}"
memory="${MEMORY:-4096}"
vcpus="${VCPUS:-2}"
disk_size="${DISK_SIZE:-40G}"
vm_user="${VM_USER:-ubuntu}"

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: missing required command: $1" >&2
    exit 1
  }
}

generate_mac_address() {
  local hash
  hash="$(printf '%s' "$1" | sha256sum | awk '{ print $1 }')"
  printf '52:54:00:%s:%s:%s\n' "${hash:0:2}" "${hash:2:2}" "${hash:4:2}"
}

reserve_dhcp_address() {
  local host_xml
  host_xml="<host mac='$mac_address' name='$vm_name' ip='$ip_address'/>"
  virsh net-update "$network_name" add ip-dhcp-host "$host_xml" --live --config
}

main() {
  if [[ $# -ne 1 || "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    usage
    exit $([[ $# -eq 1 ]] && echo 0 || echo 2)
  fi

  local vm_name="$1"
  local vm_image="$image_dir/$vm_name.qcow2"
  local seed_dir="$seed_root/$vm_name"
  local seed_iso="$seed_dir/seed.iso"
  local network_spec

  require_cmd cloud-localds
  require_cmd qemu-img
  require_cmd virt-install
  require_cmd virsh
  if [[ -n "$ip_address" && -z "$mac_address" ]]; then
    require_cmd sha256sum
    mac_address="$(generate_mac_address "$vm_name")"
  fi

  [[ -f "$base_image" ]] || {
    echo "error: missing base image: $base_image" >&2
    exit 1
  }
  [[ -f "$ssh_pubkey" ]] || {
    echo "error: missing SSH public key: $ssh_pubkey" >&2
    exit 1
  }
  if virsh dominfo "$vm_name" >/dev/null 2>&1; then
    echo "error: libvirt domain already exists: $vm_name" >&2
    exit 1
  fi
  [[ ! -e "$vm_image" ]] || {
    echo "error: VM image already exists: $vm_image" >&2
    exit 1
  }

  network_spec="network=$network_name,model=virtio"
  if [[ -n "$mac_address" ]]; then
    network_spec="$network_spec,mac=$mac_address"
  fi
  if [[ -n "$ip_address" ]]; then
    reserve_dhcp_address
  fi

  mkdir -p "$image_dir" "$seed_dir"
  qemu-img create -f qcow2 -F qcow2 -b "$base_image" "$vm_image" "$disk_size"

  cat >"$seed_dir/user-data" <<EOF
#cloud-config
preserve_hostname: false
hostname: $vm_name
users:
  - default
  - name: $vm_user
    groups: [adm, sudo]
    shell: /bin/bash
    sudo: ALL=(ALL) NOPASSWD:ALL
    ssh_authorized_keys:
      - $(<"$ssh_pubkey")
ssh_deletekeys: true
ssh_genkeytypes: [ed25519, rsa]
package_update: false
EOF

  cat >"$seed_dir/meta-data" <<EOF
instance-id: $vm_name-$(date +%s)
local-hostname: $vm_name
EOF

  cloud-localds "$seed_iso" "$seed_dir/user-data" "$seed_dir/meta-data"

  virt-install \
    --name "$vm_name" \
    --memory "$memory" \
    --vcpus "$vcpus" \
    --disk "path=$vm_image,format=qcow2,bus=virtio" \
    --disk "path=$seed_iso,device=cdrom" \
    --os-variant ubuntu24.04 \
    --import \
    --network "$network_spec" \
    --graphics none \
    --noautoconsole

  echo "created VM: $vm_name"
  echo "image: $vm_image"
  echo "seed:  $seed_iso"
  [[ -n "$mac_address" ]] && echo "mac:   $mac_address"
  [[ -n "$ip_address" ]] && echo "ip:    $ip_address"
  echo "check IP with: virsh domifaddr $vm_name"
}

main "$@"
