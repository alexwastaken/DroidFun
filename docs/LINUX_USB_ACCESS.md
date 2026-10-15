# Linux robot access

## Customer setup (0.3.4)

Install the DroidFun **DEB** with a graphical package installer on Ubuntu, approve the normal installation password prompt, then plug in the robot and open DroidFun. The package installs and activates the USB access rule automatically. There is no Terminal command, serial-group change, logout or reboot step. If the robot was already attached during installation, the setup helper sends a change event to matching serial interfaces; reconnecting it applies the rule on a new attachment as well.

Under **Connection**, **Set up robot access** repairs a missing rule and reapplies access. The desktop may request an administrator password. Cancellation or denied approval leaves the app running and reports the result. Reconnection is automatic; the UI suggests unplug/replug if the device remains unavailable. Updating an older app to this package adds both automatic installation setup and the repair button; the old installed executable cannot acquire a new button without an update.

The supported flow is an active local desktop session with systemd-logind, udev, polkit and a graphical authorization agent (standard Ubuntu desktops). Headless/SSH sessions, custom security policy, sandboxed repackaging and non-systemd distributions need their own access integration. The app does not promise desktop ACL access to remote/inactive sessions. AppImage/Snap/Flatpak packages are not published here.

## Implementation

- Package-owned `/usr/lib/udev/rules.d/70-droidfun.rules` tags only `ttyACM*` interfaces with USB parent VID `2e8a`, PID `0003` for `uaccess`, before systemd's `73-seat-late.rules`. Logind grants access to the active local desktop user and manages seat changes.
- These are the existing Arduino-Pico/Waveshare descriptors; other RP2040 boards can share them. This is a scoped serial-device access policy, not proof of genuine DroidFun hardware. No bootloader, arbitrary USB-device or all-serial-port grant is installed. Firmware remains 1.1.0.
- RPM dependencies explicitly require the GTK/WebKit/AppIndicator/udev/xdo runtime libraries, the pkexec executable and helper utilities; building an RPM is not Fedora runtime qualification.
- DEB/RPM payloads include the rule, a root-owned backup rule, fixed helper and polkit action. Install scripts call the helper using the package manager's existing authorization. Removal reloads rules and leaves administrator/user files and app preferences alone. Existing open file descriptors are not forcibly closed; unplugging removes the old device/ACL.
- The repair IPC accepts **no arguments**. It validates root ownership and non-writable permissions of the helper, policy, source rule, pkexec and their resolved parent directories, then invokes only `/usr/bin/pkexec --disable-internal-agent /usr/lib/droidfun/setup-robot-access`. It never runs a caller-supplied shell, username, device path or command.
- The helper rejects arguments and non-root execution. It restores only the package's rule, reloads udev and sends targeted change events without resetting USB. Calls are bounded; the app prevents concurrent repair dialogs and limits approval waiting to 120 seconds. Passwords stay with the system agent; the app does not read them. No general-purpose privileged service runs in the background.
- The polkit action requires administrator approval for active local users, denies inactive/other sessions, and never keeps authorization. Headless CI temporarily authorizes only this action and its fixture caller to test real IPC/pkexec; that test rule is removed and never packaged.
- If udev is unavailable during package installation, the files remain installed and installation prints a reconnect/repair notice rather than breaking the package manager. The repair button reports unsuccessful activation instead of claiming the robot is connected.

## Verification boundaries

Release qualification records the actual native Ubuntu package, policy and helper installation; rule matching for correct and incorrect USB descriptors; real kernel denial/read/write ACL behavior using an ACL-capable character node with harmless null-driver I/O and private synthetic seat; active-user change/revocation; installed app button → real IPC → pkexec → helper; approval denial; both skins; and package removal/reinstall preserving a local rule and saved data. Synthetic devices/seats and a test-only authorization grant are fixtures, not a physical robot or a human password-dialog test. The physical plug-in flow and the graphical password dialog still require a real Ubuntu desktop check.

## References

- [Tauri package files and installation scripts](https://v2.tauri.app/reference/config/#debconfig)
- [systemd seat access dispatch](https://github.com/systemd/systemd/blob/main/rules.d/73-seat-late.rules.in)
- [systemd uaccess implementation](https://github.com/systemd/systemd/blob/v255/src/udev/udev-builtin-uaccess.c)
- [pkexec policy and exit statuses](https://polkit.pages.freedesktop.org/polkit/pkexec.1.html)
- [umockdev hardware fixtures](https://github.com/martinpitt/umockdev)
