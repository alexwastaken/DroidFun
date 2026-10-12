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
sudo apt-get install -y "${debs[0]}"
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
  xdotool search --pid "$app_pid" --name Deskbot
'
node -e 'require("node:fs").writeFileSync(require("node:path").join(process.env.DESKBOT_ARTIFACT_DIR,"native-smoke.json"),JSON.stringify({result:"passed",debInstalled:true,installedBinaryMatchesPackage:true,rpmMetadataArchitecture:"x86_64",nativeWindowCreated:true,aliveSeconds:15,environment:"Ubuntu 24.04, isolated fresh XDG configuration, Xvfb and D-Bus; no tray host, robot or account credentials"},null,2))'
