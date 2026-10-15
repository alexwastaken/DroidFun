#!/usr/bin/env bash
set -euo pipefail
release="$CARGO_TARGET_DIR/$DESKBOT_TARGET/release"
shopt -s nullglob
debs=("$release"/bundle/deb/*.deb)
rpms=("$release"/bundle/rpm/*.rpm)
test "${#debs[@]}" -eq 1
test "${#rpms[@]}" -eq 1
dpkg-deb --info "${debs[0]}"
test "$(dpkg-deb -f "${debs[0]}" Architecture)" = amd64
rpm -qp --queryformat '%{NAME} %{VERSION} %{ARCH}\n' "${rpms[0]}"
test "$(rpm -qp --queryformat '%{ARCH}' "${rpms[0]}")" = x86_64
groups_before="$(id -nG)"
sudo apt-get install -y "${debs[0]}"
helper=/usr/lib/droidfun/setup-robot-access
rule=/usr/lib/udev/rules.d/70-droidfun.rules
policy=/usr/share/polkit-1/actions/io.droidfun.usb-access.policy
cmp src-tauri/linux/70-droidfun.rules "$rule"
cmp src-tauri/linux/70-droidfun.rules /usr/share/droidfun/70-droidfun.rules
cmp src-tauri/linux/setup-robot-access "$helper"
test "$(stat -c '%U:%G:%a' "$helper")" = root:root:755
xmllint --noout --nonet "$policy"
pkaction --action-id io.droidfun.setup-robot-access --verbose > "$DESKBOT_ARTIFACT_DIR/linux-polkit-policy.txt"
grep -q 'auth_admin' "$DESKBOT_ARTIFACT_DIR/linux-polkit-policy.txt"
if "$helper"; then echo 'Unprivileged helper must fail'; exit 1; fi
if sudo "$helper" unexpected; then echo 'Helper must reject arguments'; exit 1; fi
sudo "$helper"
sudo "$helper" # idempotence
sudo umockdev-wrapper /usr/bin/python3 scripts/test-linux-usb.py --rules | tee "$DESKBOT_ARTIFACT_DIR/linux-rule-tests.json"
sudo mkdir -p /run/systemd/seats
sudo unshare --mount --propagation private /usr/bin/python3 scripts/test-linux-usb.py --acl | tee "$DESKBOT_ARTIFACT_DIR/linux-acl-tests.json"
test "$(id -nG)" = "$groups_before"
# Tauri embeds bundle-type metadata and restores the original binary after each bundle.
# Compare the installation against its own packaged payload, not the restored build.
deb_payload="$RUNNER_TEMP/deskbot-deb-payload"
dpkg-deb --extract "${debs[0]}" "$deb_payload"
cmp "$deb_payload/usr/bin/deskbot" /usr/bin/deskbot
export XDG_CONFIG_HOME="$RUNNER_TEMP/deskbot-smoke/config"
export XDG_CACHE_HOME="$RUNNER_TEMP/deskbot-smoke/cache"
export XDG_DATA_HOME="$RUNNER_TEMP/deskbot-smoke/data"
export WEBKIT_DISABLE_DMABUF_RENDERER=1
dbus-run-session -- xvfb-run -a bash -eu -c '
  /usr/bin/deskbot >"$DESKBOT_ARTIFACT_DIR/linux-startup.txt" 2>&1 &
  app_pid=$!
  trap "kill $app_pid 2>/dev/null || true" EXIT
  sleep 15
  kill -0 "$app_pid"
  xdotool search --pid "$app_pid" --name "DroidFun App"
'
# Drive the exact installed app, actual IPC, pkexec and helper in both themes.
dbus-run-session -- xvfb-run -a /usr/bin/python3 scripts/test-linux-usb.py --ui | tee "$DESKBOT_ARTIFACT_DIR/linux-usb-ui-tests.json"
# Removal must remove only packaged files and preserve a local administrator rule
# and saved app settings. Reinstallation restores installer-managed USB access.
sentinel=/etc/udev/rules.d/69-droidfun-ci-preserve.rules
echo '# administrator fixture' | sudo tee "$sentinel" >/dev/null
mkdir -p "$XDG_CONFIG_HOME/com.deskbot.companion"
echo 'user data fixture' > "$XDG_CONFIG_HOME/com.deskbot.companion/preserve-test.txt"
settings_before="$(sha256sum "$XDG_CONFIG_HOME/com.deskbot.companion/settings.json")"
package_name="$(dpkg-deb -f "${debs[0]}" Package)"
sudo dpkg --remove "$package_name"
test ! -e "$rule"
test ! -e "$helper"
test ! -e "$policy"
test -f "$sentinel"
test -f "$XDG_CONFIG_HOME/com.deskbot.companion/preserve-test.txt"
test "$(sha256sum "$XDG_CONFIG_HOME/com.deskbot.companion/settings.json")" = "$settings_before"
sudo apt-get install -y "${debs[0]}"
cmp src-tauri/linux/70-droidfun.rules "$rule"
test "$(id -nG)" = "$groups_before"
sudo rm "$sentinel"
node -e 'require("node:fs").writeFileSync(require("node:path").join(process.env.DESKBOT_ARTIFACT_DIR,"native-smoke.json"),JSON.stringify({result:"passed",debInstalled:true,automaticUsbRuleInstalled:true,repairButtonRealIpc:true,scopedUaccessFixture:true,removalAndReinstallPreserveUserFiles:true,noGroupChanges:true,installedBinaryMatchesPackage:true,rpmMetadataArchitecture:"x86_64",nativeWindowCreated:true,aliveSeconds:15,environment:"Ubuntu 24.04, isolated fresh XDG configuration, Xvfb and D-Bus; no tray host, robot or account credentials"},null,2))'
