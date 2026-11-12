#!/usr/bin/env bash
# A small Linux machine in VirtualBox, only to learn whether the apps still work there,
# with a real window on a real X server. Debian without a desktop, plus twm, and lavapipe
# drawing Vulkan in software. It never measures performance: it has no GPU.
#
#   scripts/vm-linux.sh create   install the machine (once; takes a while, unattended)
#   scripts/vm-linux.sh provision   install the packages, if the installer's last step failed
#   scripts/vm-linux.sh seal     slim the machine down and keep that as its base
#   scripts/vm-linux.sh start    turn it on and wait for it
#   scripts/vm-linux.sh test [example]   copy this checkout, build, open the app, take a picture
#   scripts/vm-linux.sh stop     turn it off and throw away everything since the base
#
# The base holds only what runs an app, under 1 GB. The compiler, the -dev packages, Rust,
# the copy of the checkout and the build output come in with each test and go away at
# `stop`, which returns the disk to the base. Run `stop` after every use, even a failed one.
set -euo pipefail
cd "$(dirname "$0")/.."

NAME=ravenrs-linux
HOME_VMS="$HOME/VirtualBox VMs"
ISO_NAME=debian-13.7.0-arm64-netinst.iso
ISO_URL="https://cdimage.debian.org/debian-cd/current/arm64/iso-cd/$ISO_NAME"
ISO="$HOME_VMS/iso-debian/$ISO_NAME"
SECRET="$HOME_VMS/$NAME.password"
KEY="$HOME_VMS/$NAME.key"
ASKPASS="$HOME_VMS/$NAME.askpass"
BASE=limpa
PORT=2222
USER_NAME=raven
# What an app needs to run: kept in the base.
RUNTIME="openssh-server sudo rsync curl ca-certificates xserver-xorg-core xserver-xorg-input-libinput twm mesa-vulkan-drivers libvulkan1 libasound2t64 libssl3t64 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 libgtk-3-0t64 libdbus-1-3 libxdo3 libayatana-appindicator3-1 libx11-6 libxcursor1 libxrandr2 libxi6 libfontconfig1 libfreetype6 libudev1"
# What building one needs: installed by each test, gone at `stop`.
BUILD="build-essential pkg-config libasound2-dev libssl-dev libxkbcommon-dev libwayland-dev libgtk-3-dev libdbus-1-dev libxdo-dev libayatana-appindicator3-dev"
# Packages the installer brings that nothing here uses, matched whole.
EXTRA='-dev$|^(gcc|g\+\+|cpp)(-[0-9]+)?(-aarch64-linux-gnu)?$|^(binutils.*|build-essential|make|dpkg-dev|pkgconf.*|pkg-config|libc-devtools|manpages.*|man-db|groff-base|info|install-info|locales|util-linux-locales|iso-codes|perl|perl-modules-.*|libperl5.*|python3-.*|reportbug|apt-listchanges|installation-report|tasksel.*|task-.*|debian-faq|doc-debian|nano|vim-tiny|vim-common|bash-completion|laptop-detect|eject|os-prober|inetutils-telnet|traceroute|dmidecode|whiptail|lsof|wget|file|libmagic.*|bsdextrautils|wamerican|openbox|xorg|xterm|scrot|vulkan-tools|x11-xserver-utils)$'

SSH_OPTIONS=(-p "$PORT" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR)

ssh_vm() {
    ssh "${SSH_OPTIONS[@]}" -i "$KEY" -o IdentitiesOnly=yes -o BatchMode=yes "$USER_NAME@127.0.0.1" "$@"
}

# With the password, through OpenSSH's own askpass: sshpass misses now and then here.
ssh_password() {
    if [ ! -x "$ASKPASS" ]; then
        printf '#!/bin/sh\ncat "%s"\n' "$SECRET" >"$ASKPASS"
        chmod 700 "$ASKPASS"
    fi
    SSH_ASKPASS="$ASKPASS" SSH_ASKPASS_REQUIRE=force \
        ssh "${SSH_OPTIONS[@]}" -o PubkeyAuthentication=no "$USER_NAME@127.0.0.1" "$@" </dev/null
}

install_key() {
    [ -f "$KEY" ] || ssh-keygen -q -t ed25519 -N "" -C "$NAME" -f "$KEY"
    ssh_vm true 2>/dev/null && return 0
    ssh_password "mkdir -p ~/.ssh && chmod 700 ~/.ssh && echo '$(cat "$KEY.pub")' >>~/.ssh/authorized_keys && chmod 600 ~/.ssh/authorized_keys"
    ssh_vm true
}

wait_ssh() {
    for _ in $(seq 1 "${1:-120}"); do
        if ssh_vm true 2>/dev/null || ssh_password true 2>/dev/null; then
            return 0
        fi
        sleep 5
    done
    echo "the machine did not answer on port $PORT" >&2
    return 1
}

create() {
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
    # Dynamic: it takes only what is written, and discard gives back what the guest frees.
    VBoxManage createmedium disk --filename "$HOME_VMS/$NAME/$NAME.vdi" --size 40000 --variant Standard
    VBoxManage storagectl "$NAME" --name VirtIO --add virtio --controller VirtIO --portcount 2
    VBoxManage storageattach "$NAME" --storagectl VirtIO --port 0 --device 0 --type hdd \
        --medium "$HOME_VMS/$NAME/$NAME.vdi" --nonrotational on --discard on
    VBoxManage unattended install "$NAME" --iso="$ISO" --user="$USER_NAME" \
        --user-password-file="$SECRET" --admin-password-file="$SECRET" --hostname="$NAME.local" \
        --locale=en_US --time-zone=UTC --package-selection-adjustment=minimal \
        --post-install-command="apt-get update; apt-get install -y $RUNTIME; adduser $USER_NAME sudo; echo '$USER_NAME ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/$USER_NAME" \
        --start-vm=headless
    echo "installing; follow it with: scripts/vm-linux.sh seal"
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
    type_line "sed -i '/cdrom/d' /etc/apt/sources.list; apt-get update; DEBIAN_FRONTEND=noninteractive apt-get install -y $RUNTIME; adduser $USER_NAME sudo; echo '$USER_NAME ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/$USER_NAME; echo PROVISIONED"
    echo "packages installing on the console; follow it with: scripts/vm-linux.sh seal"
}

start() {
    if ! VBoxManage list runningvms | grep -q "\"$NAME\""; then
        VBoxManage startvm "$NAME" --type headless
    fi
    wait_ssh 240
    install_key
    echo "$NAME is up"
}

test_app() {
    local example="${1:-family-finance}"
    start
    ssh_vm "sudo apt-get update -qq && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq $BUILD >/dev/null"
    ssh_vm 'curl -fsSL https://sh.rustup.rs | sh -s -- -y -q --profile minimal >/dev/null'
    rsync -az --delete --exclude target --exclude .git --exclude .claude \
        -e "ssh -p $PORT -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR -i '$KEY' -o IdentitiesOnly=yes -o BatchMode=yes" \
        ./ "$USER_NAME@127.0.0.1:RavenRS/"
    # gemm-f16, under the local AI, needs an ARM instruction Linux does not assume. The
    # machine has it: it runs on Apple Silicon.
    ssh_vm "cd RavenRS && RUSTFLAGS='-C target-feature=+fp16' ~/.cargo/bin/cargo build -q -p $example"
    # An X server with a window manager, as a real session has, and the app on top of it.
    # twm would wait for a person to place each new window.
    ssh_vm "pgrep -x Xorg >/dev/null || (sudo nohup Xorg :0 -noreset >/tmp/xorg.log 2>&1 &) ; sleep 3; printf 'RandomPlacement\\nUsePPosition \\\"on\\\"\\n' >~/.twmrc; pgrep -x twm >/dev/null || (DISPLAY=:0 nohup twm >/dev/null 2>&1 &) ; sleep 1"
    # Process names stop at 15 characters, and -f would also match this very command.
    local process="${example:0:15}"
    ssh_vm "pkill -x $process || true; cd RavenRS && (DISPLAY=:0 nohup target/debug/$example >/tmp/app.log 2>&1 &) ; sleep 30; pgrep -x $process >/dev/null && echo 'the app is running' || { echo 'the app stopped'; tail -30 /tmp/app.log; exit 1; }"
    local picture
    picture="$(mktemp -t "$example").png"
    VBoxManage controlvm "$NAME" screenshotpng "$picture"
    echo "picture of the screen: $picture"
    ssh_vm "tail -5 /tmp/app.log; pkill -x $process || true"
}

is_off() {
    VBoxManage showvminfo "$NAME" --machinereadable | grep -Eq '^VMState="(poweroff|aborted|saved)"'
}

# The power button is not always heard, so ask the system first and pull the plug last.
power_off() {
    is_off && return 0
    ssh_vm 'sudo poweroff' 2>/dev/null || true
    for _ in $(seq 1 40); do
        is_off && return 0
        sleep 3
    done
    VBoxManage controlvm "$NAME" poweroff
}

has_base() {
    VBoxManage snapshot "$NAME" list --machinereadable 2>/dev/null | grep -q "=\"$BASE\""
}

# Leaves only what runs an app, then keeps that as the base. Running it again starts from
# the current base, so it also refreshes one.
seal() {
    if has_base; then
        power_off
        VBoxManage snapshot "$NAME" restore "$BASE" >/dev/null
        VBoxManage snapshot "$NAME" delete "$BASE" >/dev/null
    fi
    start
    # shellcheck disable=SC2016 # expanded on the machine
    ssh_vm "RUNTIME='$RUNTIME' EXTRA='$EXTRA' bash -s" <<'GUEST'
set -euo pipefail
export DEBIAN_FRONTEND=noninteractive
# Documentation and translations are never installed again, and nothing optional comes along.
# Of Mesa's Vulkan drivers only lavapipe, which draws in software, has hardware to run on.
{
    printf 'path-exclude /usr/share/%s/*\n' doc man info locale lintian
    for gpu in broadcom freedreno panfrost radeon nouveau gfxstream virtio intel asahi powervr; do
        printf 'path-exclude /usr/lib/*/libvulkan_%s.so\npath-exclude /usr/share/vulkan/icd.d/%s*_icd*.json\n' "$gpu" "$gpu"
    done
} | sudo tee /etc/dpkg/dpkg.cfg.d/raven-slim >/dev/null
# The boot image takes only the modules this machine uses.
sudo sed -i 's/^MODULES=.*/MODULES=dep/' /etc/initramfs-tools/initramfs.conf
printf 'APT::Install-Recommends "false";\nAPT::Install-Suggests "false";\n' | sudo tee /etc/apt/apt.conf.d/99raven-slim >/dev/null
sudo apt-get update -qq
# shellcheck disable=SC2086
sudo apt-get install -y -qq $RUNTIME >/dev/null
# shellcheck disable=SC2086
sudo apt-mark manual $RUNTIME >/dev/null
extra=$(dpkg-query -Wf '${Package}\n' | grep -E "$EXTRA" | grep -vxE 'gcc-[0-9]+-base|libgcc-s1|cpp-[0-9]+-base|libstdc\+\+6' || true)
# shellcheck disable=SC2086
[ -z "$extra" ] || sudo apt-get purge -y -qq $extra >/dev/null
sudo apt-get autoremove --purge -y -qq >/dev/null
sudo apt-get clean
sudo rm -rf /usr/share/doc/* /usr/share/man/* /usr/share/info/* /usr/share/locale/* \
    /var/lib/apt/lists/* /var/cache/debconf/*-old ~/RavenRS ~/.cargo ~/.rustup /tmp/* /var/tmp/*
for gpu in broadcom freedreno panfrost radeon nouveau gfxstream virtio intel asahi powervr; do
    sudo rm -f /usr/lib/*/libvulkan_"$gpu".so /usr/share/vulkan/icd.d/"$gpu"*_icd*.json
done
sudo journalctl --vacuum-size=5M >/dev/null 2>&1 || true
# Swap is written to while building; a blank one costs nothing in the base.
swap=$(sudo /usr/sbin/swapon --show=NAME --noheadings | head -1)
if [ -n "$swap" ]; then
    uuid=$(sudo /usr/sbin/blkid -s UUID -o value "$swap")
    sudo /usr/sbin/swapoff "$swap"
    sudo /usr/sbin/blkdiscard -f "$swap"
    sudo /usr/sbin/mkswap -U "$uuid" "$swap" >/dev/null
fi
# The disk image grows by whole megabytes, so files spread over the whole partition cost
# far more than they hold. Shrinking the filesystem to its minimum and growing it back
# moves them to its start; that needs it unmounted, so it runs once, at the next boot. The
# journal is made again at 16 MB: the size chosen for a 36 GB disk, 256 MB, is all written.
sudo tee /etc/initramfs-tools/hooks/raven-pack >/dev/null <<'EOF'
#!/bin/sh
[ "$1" = prereqs ] && exit 0
. /usr/share/initramfs-tools/hook-functions
copy_exec /usr/sbin/e2fsck /sbin
copy_exec /usr/sbin/resize2fs /sbin
copy_exec /usr/sbin/tune2fs /sbin
EOF
sudo tee /etc/initramfs-tools/scripts/local-premount/raven-pack >/dev/null <<'EOF'
#!/bin/sh
[ "$1" = prereqs ] && exit 0
. /scripts/functions
dev=$(resolve_device "$ROOT")
e2fsck -fy "$dev" >/dev/kmsg 2>&1
tune2fs -O ^has_journal "$dev" >/dev/kmsg 2>&1
resize2fs -M "$dev" >/dev/kmsg 2>&1
resize2fs "$dev" >/dev/kmsg 2>&1
tune2fs -J size=16 "$dev" >/dev/kmsg 2>&1
e2fsck -fy "$dev" >/dev/kmsg 2>&1
exit 0
EOF
sudo chmod +x /etc/initramfs-tools/hooks/raven-pack /etc/initramfs-tools/scripts/local-premount/raven-pack
sudo update-initramfs -u >/dev/null 2>&1
GUEST
    ssh_vm '(sleep 1; sudo reboot) >/dev/null 2>&1 &'
    sleep 20
    wait_ssh 120
    ssh_vm 'bash -s' <<'GUEST'
set -euo pipefail
sudo rm -f /etc/initramfs-tools/hooks/raven-pack /etc/initramfs-tools/scripts/local-premount/raven-pack
sudo update-initramfs -u >/dev/null 2>&1
sudo rm -rf /tmp/* /var/tmp/*
sudo /usr/sbin/fstrim -a >/dev/null
df -h / | tail -1
GUEST
    power_off
    # Gives back the blocks the trim left as zeros.
    VBoxManage modifymedium disk "$HOME_VMS/$NAME/$NAME.vdi" --compact >/dev/null
    VBoxManage snapshot "$NAME" take "$BASE" --description "Only what runs an app. scripts/vm-linux.sh stop returns here."
    du -sh "$HOME_VMS/$NAME"
}

stop() {
    power_off
    if has_base; then
        # Restoring drops the disk written since the base, and with it everything the tests left.
        VBoxManage snapshot "$NAME" restore "$BASE" >/dev/null
    else
        echo "warning: $NAME has no base, so what the tests left stays; run seal" >&2
    fi
    du -sh "$HOME_VMS/$NAME"
}

case "${1:-}" in
    create) create ;;
    provision) provision ;;
    seal) seal ;;
    start) start ;;
    test) test_app "${2:-}" ;;
    stop) stop ;;
    *) sed -n 2,15p "$0"; exit 2 ;;
esac
