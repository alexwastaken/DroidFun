#!/usr/bin/env bash
set -euo pipefail
# Use the fixture from this checked-out build pipeline while testing the exact
# requested application source and its installed native binary.
usb_fixture="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/test-linux-usb.py"
release="$CARGO_TARGET_DIR/$DESKBOT_TARGET/release"
shopt -s nullglob
debs=("$release"/bundle/deb/*.deb)
rpms=("$release"/bundle/rpm/*.rpm)
test "${#debs[@]}" -eq 1
test "${#rpms[@]}" -eq 1
dpkg-deb --info "${debs[0]}"
dpkg-deb --contents "${debs[0]}"
test "$(dpkg-deb -f "${debs[0]}" Architecture)" = amd64
/usr/bin/python3 - "${debs[0]}" <<'PY' | tee "$DESKBOT_ARTIFACT_DIR/linux-package-directory-modes.json"
import io, json, subprocess, sys, tarfile
data = subprocess.run(['dpkg-deb', '--fsys-tarfile', sys.argv[1]], check=True, capture_output=True).stdout
with tarfile.open(fileobj=io.BytesIO(data)) as archive:
    directories = [{"path": p.name, "uid": p.uid, "gid": p.gid, "mode": oct(p.mode)} for p in archive if p.isdir()]
print(json.dumps(directories, indent=2))
assert directories and all(p['uid'] == 0 and p['gid'] == 0 and int(p['mode'], 8) & 0o022 == 0 for p in directories), 'Package contains an untrusted directory mode or owner'
PY
rpm -qp --queryformat '%{NAME} %{VERSION} %{ARCH}\n' "${rpms[0]}"
test "$(rpm -qp --queryformat '%{ARCH}' "${rpms[0]}")" = x86_64
groups_before="$(id -nG)"
# Record pre-existing shared directories separately from the archive metadata;
# the package must never make a system policy directory writable by users.
stat -c 'Before install: %U:%G:%a %n' /usr/share /usr/share/polkit-1 /usr/share/polkit-1/actions
# GitHub's Ubuntu image deliberately chmods /usr/share recursively to 0777:
# https://github.com/actions/runner-images/blob/main/images/ubuntu/scripts/build/configure-system.sh
# Restore ordinary OS permissions only for these three policy ancestors in the
# ephemeral hosted runner. Never weaken the app's trust gate or modify a package.
export DESKBOT_CI_POLICY_DIR_FIXTURE=false
if [[ "${GITHUB_ACTIONS:-}" == true && "${RUNNER_ENVIRONMENT:-}" == github-hosted ]]; then
  /usr/bin/python3 - <<'PY'
import pathlib, stat
for name in ['/usr/share', '/usr/share/polkit-1', '/usr/share/polkit-1/actions']:
    meta = pathlib.Path(name).lstat()
    assert stat.S_ISDIR(meta.st_mode) and meta.st_uid == 0 and meta.st_gid == 0, f'Unexpected runner policy directory owner/type: {name}'
    assert stat.S_IMODE(meta.st_mode) in (0o755, 0o777), f'Unexpected runner policy directory mode: {name}'
PY
  sudo chmod 0755 /usr/share /usr/share/polkit-1 /usr/share/polkit-1/actions
  export DESKBOT_CI_POLICY_DIR_FIXTURE=true
fi
sudo apt-get install -y "${debs[0]}"
helper=/usr/lib/droidfun/setup-robot-access
rule=/usr/lib/udev/rules.d/70-droidfun.rules
policy=/usr/share/polkit-1/actions/io.droidfun.usb-access.policy
cmp src-tauri/linux/70-droidfun.rules "$rule"
cmp src-tauri/linux/70-droidfun.rules /usr/share/droidfun/70-droidfun.rules
cmp src-tauri/linux/setup-robot-access "$helper"
test "$(stat -c '%U:%G:%a' "$helper")" = root:root:644
# Match the native repair gate exactly so packaging failures identify the unsafe
# file or ancestor instead of surfacing later as an unexplained disabled button.
/usr/bin/python3 - <<'PY' | tee "$DESKBOT_ARTIFACT_DIR/linux-repair-path-trust.json"
import json, pathlib, stat
checks = []
for name, executable in [
    ("/usr/lib/droidfun/setup-robot-access", False),
    ("/usr/bin/sh", True),
    ("/usr/bin/pkexec", True),
    ("/usr/share/polkit-1/actions/io.droidfun.usb-access.policy", False),
    ("/usr/share/droidfun/70-droidfun.rules", False),
]:
    path = pathlib.Path(name).resolve(strict=True)
    parts = []
    for index, part in enumerate([path, *path.parents]):
        meta = part.lstat()
        trusted = meta.st_uid == 0 and meta.st_mode & 0o022 == 0
        trusted = trusted and (stat.S_ISREG(meta.st_mode) and (not executable or meta.st_mode & 0o111 != 0) if index == 0 else stat.S_ISDIR(meta.st_mode))
        parts.append({"path": str(part), "uid": meta.st_uid, "gid": meta.st_gid, "mode": oct(stat.S_IMODE(meta.st_mode)), "trusted": trusted})
    checks.append({"requested": name, "trusted": all(p["trusted"] for p in parts), "components": parts})
print(json.dumps(checks, indent=2))
assert all(check["trusted"] for check in checks), "Installed Linux USB repair paths fail the native ownership/permission guard; see components above"
PY
xmllint --noout --nonet "$policy"
pkaction --action-id io.droidfun.setup-robot-access --verbose > "$DESKBOT_ARTIFACT_DIR/linux-polkit-policy.txt"
grep -q 'auth_admin' "$DESKBOT_ARTIFACT_DIR/linux-polkit-policy.txt"
if /usr/bin/sh "$helper"; then echo 'Unprivileged helper must fail'; exit 1; fi
if sudo /usr/bin/sh "$helper" unexpected; then echo 'Helper must reject arguments'; exit 1; fi
sudo /usr/bin/sh "$helper"
sudo /usr/bin/sh "$helper" # idempotence
sudo umockdev-wrapper /usr/bin/python3 "$usb_fixture" --rules | tee "$DESKBOT_ARTIFACT_DIR/linux-rule-tests.json"
sudo mkdir -p /run/systemd/seats
sudo unshare --mount --propagation private /usr/bin/python3 "$usb_fixture" --acl | tee "$DESKBOT_ARTIFACT_DIR/linux-acl-tests.json"
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
# Drive the exact installed app, bot themes, actual IPC, pkexec and helper.
dbus-run-session -- xvfb-run -a /usr/bin/python3 "$usb_fixture" --ui | tee "$DESKBOT_ARTIFACT_DIR/linux-usb-ui-tests.json"
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
node -e 'require("node:fs").writeFileSync(require("node:path").join(process.env.DESKBOT_ARTIFACT_DIR,"native-smoke.json"),JSON.stringify({result:"passed",debInstalled:true,automaticUsbRuleInstalled:true,repairButtonRealIpc:true,mandatoryBotChooser:true,botSelectionRealIpc:true,automaticBotThemes:["claw","meep","boo","nimbus"],botSpecificWidgetOptions:true,scopedUaccessFixture:true,removalAndReinstallPreserveUserFiles:true,noGroupChanges:true,installedBinaryMatchesPackage:true,rpmMetadataArchitecture:"x86_64",nativeWindowCreated:true,aliveSeconds:15,runnerPolicyDirectoryFixture:process.env.DESKBOT_CI_POLICY_DIR_FIXTURE==="true",environment:"Ubuntu 24.04, isolated fresh XDG configuration, Xvfb and D-Bus; hosted runner policy ancestors normalized to ordinary root:root 0755; no tray host, robot or account credentials"},null,2))'
