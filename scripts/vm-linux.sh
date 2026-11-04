#!/usr/bin/env bash
# A small Linux machine in VirtualBox, only to learn whether the apps still work there,
# with a real window on a real X server. Debian without a desktop, plus Openbox, and
# lavapipe drawing Vulkan in software. It never measures performance: it has no GPU.
#
#   scripts/vm-linux.sh create   install the machine (once; takes a while, unattended)
#   scripts/vm-linux.sh provision   install the packages, if the installer's last step failed
#   scripts/vm-linux.sh start    turn it on and wait for it
#   scripts/vm-linux.sh test [example]   copy this checkout, build, open the app, take a picture
#   scripts/vm-linux.sh stop     turn it off
set -euo pipefail
cd "$(dirname "$0")/.."

NAME=ravenrs-linux
HOME_VMS="$HOME/VirtualBox VMs"
ISO_NAME=debian-13.7.0-arm64-netinst.iso
ISO_URL="https://cdimage.debian.org/debian-cd/current/arm64/iso-cd/$ISO_NAME"
ISO="$HOME_VMS/iso-debian/$ISO_NAME"
SECRET="$HOME_VMS/$NAME.password"
PORT=2222
USER_NAME=raven
PACKAGES="openssh-server sudo xorg openbox xterm mesa-vulkan-drivers libvulkan1 vulkan-tools build-essential pkg-config libasound2-dev libssl-dev libxkbcommon-dev libxkbcommon-x11-0 libwayland-dev libgtk-3-dev libdbus-1-dev libxdo-dev libayatana-appindicator3-dev rsync curl scrot"

ssh_vm() {
    sshpass -f "$SECRET" ssh -p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
        -o LogLevel=ERROR "$USER_NAME@127.0.0.1" "$@"
}

wait_ssh() {
    for _ in $(seq 1 "${1:-120}"); do
        if ssh_vm true 2>/dev/null; then
            return 0
        fi
        sleep 5
    done
    echo "the machine did not answer on port $PORT" >&2
    return 1
}

create() {
    command -v sshpass >/dev/null || { echo "install sshpass first: brew install sshpass" >&2; exit 1; }
    if VBoxManage showvminfo "$NAME" >/dev/null 2>&1; then
        echo "$NAME already exists"
        return
    fi
    mkdir -p "$(dirname "$ISO")"
    [ -f "$ISO" ] || curl -fsSL -o "$ISO" "$ISO_URL"
    # `head` closing the pipe is how this ends, so the pipe's status means nothing here.
    [ -f "$SECRET" ] || { (LC_ALL=C tr -dc 'a-zA-Z0-9' </dev/urandom | head -c 20 >"$SECRET") || true; chmod 600 "$SECRET"; }
    VBoxManage createvm --name "$NAME" --ostype Debian13_arm64 --platform-architecture=arm --register
    VBoxManage modifyvm "$NAME" --memory 6144 --cpus 6 --vram 128 --graphicscontroller vmsvga \
        --firmware efi --nic1 nat --natpf1 "ssh,tcp,127.0.0.1,$PORT,,22" --mouse usbtablet --keyboard usb \
        --audio-enabled off
    # Dynamic: it takes only what is written. Most of it will be Rust's build output.
    VBoxManage createmedium disk --filename "$HOME_VMS/$NAME/$NAME.vdi" --size 40000 --variant Standard
    VBoxManage storagectl "$NAME" --name VirtIO --add virtio --controller VirtIO --portcount 2
    VBoxManage storageattach "$NAME" --storagectl VirtIO --port 0 --device 0 --type hdd \
        --medium "$HOME_VMS/$NAME/$NAME.vdi"
    VBoxManage unattended install "$NAME" --iso="$ISO" --user="$USER_NAME" \
        --user-password-file="$SECRET" --admin-password-file="$SECRET" --hostname="$NAME.local" \
        --locale=en_US --time-zone=UTC --package-selection-adjustment=minimal \
        --post-install-command="apt-get update; apt-get install -y $PACKAGES; adduser $USER_NAME sudo; echo '$USER_NAME ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/$USER_NAME" \
        --start-vm=headless
    echo "installing; follow it with: scripts/vm-linux.sh start"
}

# Types a line on the machine's console, a little at a time: its keyboard queue is short.
type_line() {
    local line="$1" piece
    while [ -n "$line" ]; do
        piece="${line:0:8}"
        line="${line:8}"
        VBoxManage controlvm "$NAME" keyboardputstring "$piece"
        sleep 1
    done
    VBoxManage controlvm "$NAME" keyboardputscancode 1c 9c
    sleep 1
}

# Installs the packages through the console, which works before the machine has SSH.
provision() {
    type_line root
    sleep 5
    type_line "$(cat "$SECRET")"
    sleep 8
    type_line "sed -i '/cdrom/d' /etc/apt/sources.list; apt-get update; DEBIAN_FRONTEND=noninteractive apt-get install -y $PACKAGES; adduser $USER_NAME sudo; echo '$USER_NAME ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/$USER_NAME; echo PROVISIONED"
    echo "packages installing on the console; scripts/vm-linux.sh start waits for SSH"
}

start() {
    if ! VBoxManage list runningvms | grep -q "\"$NAME\""; then
        VBoxManage startvm "$NAME" --type headless
    fi
    wait_ssh 240
    echo "$NAME is up"
}

test_app() {
    local example="${1:-family-finance}"
    start
    ssh_vm 'command -v cargo >/dev/null || [ -x ~/.cargo/bin/cargo ] || curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal'
    sshpass -f "$SECRET" rsync -az --delete --exclude target --exclude .git --exclude .claude \
        -e "ssh -p $PORT -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR" \
        ./ "$USER_NAME@127.0.0.1:RavenRS/"
    # gemm-f16, under the local AI, needs an ARM instruction Linux does not assume. The
    # machine has it: it runs on Apple Silicon.
    ssh_vm "cd RavenRS && RUSTFLAGS='-C target-feature=+fp16' ~/.cargo/bin/cargo build -q -p $example"
    # An X server with a window manager, as a real session has, and the app on top of it.
    ssh_vm "pgrep -x Xorg >/dev/null || (sudo nohup Xorg :0 -noreset >/tmp/xorg.log 2>&1 &) ; sleep 3; pgrep -x openbox >/dev/null || (DISPLAY=:0 nohup openbox >/dev/null 2>&1 &) ; sleep 1"
    # Process names stop at 15 characters, and -f would also match this very command.
    local process="${example:0:15}"
    ssh_vm "pkill -x $process || true; cd RavenRS && (DISPLAY=:0 nohup target/debug/$example >/tmp/app.log 2>&1 &) ; sleep 30; pgrep -x $process >/dev/null && echo 'the app is running' || { echo 'the app stopped'; tail -30 /tmp/app.log; exit 1; }"
    local picture
    picture="$(mktemp -t "$example").png"
    VBoxManage controlvm "$NAME" screenshotpng "$picture"
    echo "picture of the screen: $picture"
    ssh_vm "tail -5 /tmp/app.log; pkill -x $process || true"
}

stop() {
    VBoxManage controlvm "$NAME" acpipowerbutton
}

case "${1:-}" in
    create) create ;;
    provision) provision ;;
    start) start ;;
    test) test_app "${2:-}" ;;
    stop) stop ;;
    *) sed -n 2,10p "$0"; exit 2 ;;
esac
