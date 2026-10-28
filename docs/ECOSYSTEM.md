# Customer and Workshop handoff — 0.7.0 / 0.3.1

The current lineup is Nimbus, Boo, Meep and Claw. Workshop is the private preparation tool; DroidFun App is the customer download.

## Before shipping

1. Open Workshop and connect one board. It reads the existing firmware, model, screen health and reported screen wiring before offering programming.
2. For a new/unidentified board, select the actual model and wiring and explicitly install the bundled factory firmware. A bootloader alone cannot prove the board is blank. Current prepared boards can be checked without reinstalling.
3. Complete the unplug/reconnect check and confirm the physical screen is readable. Software checks cannot substitute for those physical steps.

Workshop uses factory 1.3.2 (standard GP4 SDA / GP5 SCL) or 1.3.2-mistake (GP5 SDA / GP4 SCL). Both persist the product model and report it over the standard HELLO protocol. Product codes: Boo 1, Claw 2, Meep 3, Nimbus 4. The identity hardwareProfile names the board family; the HELLO profile identifies the actual screen pins.

## Wipe and reinstall in private Workshop

Workshop 0.3.1 offers **Prepare a droid → Prepare and verify → Wipe and reinstall**. Review the connected board, selected model and actual wiring, confirm erasing its contents, then choose **Erase board and reinstall**. This verifies a supported 2 MiB RP2040 flash, writes and reads back a complete fresh flash image, then provisions the selected model and startup face. Firmware remains 1.3.2 standard or 1.3.2-mistake. The hardware ID and private inventory notes remain; preparation checks must be completed again. A normal reinstall is separate and does not promise whole-flash erasure. Physical wipe acceptance remains open; no board was attached during software verification.

## Customer setup

1. Download and install DroidFun App for the computer from the website.
2. Plug in the prepared droid using a USB data cable and open the app. A single factory droid is recognized automatically, including Nimbus and Mistake units. Its saved profile is restored if it has been used before.
3. Choose screens. Faces and text work locally. Account widgets and online sources are optional. Preview works without a droid.

Older firmware without a model can use a model-only lookup after Workshop registration. Offline or unknown boards ask for the model once. Multiple connected droids require choosing one; no silent switching away from a selected board. Each physical USB identity keeps its own playlist and display settings. Read-only discovery is bounded, cached for each attachment, and retries transient errors; Find droid permits a fresh attempt.

## Private inventory

Dashboard → Droids creates a one-use connection code for Workshop → Droid list. The paired Workshop lists registered IDs, models, firmware, wiring and preparation status; “Connected here” means physically attached to that Workshop computer. It inspects factory firmware before offering installation and queues records while offline. Older programmed boards can be recorded by reconnecting once and choosing their model, without reflashing. Marking a board prepared still requires reconnect and physical screen confirmations.

The customer app contains no database credentials or inventory-list commands. The server returns only a model hint for the connected board ID; board IDs do not authenticate ownership. Full inventory and private notes require owner sign-in or a revocable inventory-scoped Workshop session. No customer connection history is collected. See [the registry boundaries](DROID_REGISTRY.md).

## Updates and recovery

Settings → App contains Check for updates, Download update, and Restart to update. Mac and Windows check quietly once per online session; downloads and restarts are explicit. Pause online data blocks checks/downloads. Linux uses website packages. Existing signed updater verification and settings preservation remain in place.

App updates and droid firmware are separate. The customer recovery image only services recognized standard legacy firmware through 1.2.4, on a supported Mac through direct USB with a verified board ID. Mounted boot drives cannot prove identity before writing, so Windows/Linux firmware service belongs in Workshop/support. Native checks before USB reset reject factory, newer, custom and swapped-wiring firmware so generic recovery cannot downgrade it or erase its factory identity. Factory firmware service belongs in Workshop. Do not tell customers to install a standard UF2 on an unidentified board.

## Release gates

Keep the public feed and website installer records on the last verified release until new native installers and their signatures pass verification. Never relabel an older installer as a newer version. Workshop remains private. Website instructions derive versions from published catalog records and show auto-recognition instructions only for a complete 0.6.0-or-newer installer set.

First-install platform trust remains separate: current Mac packaging is ad-hoc signed without Apple notarization; Windows lacks publisher signing. Physical recognition, power cycling, display visibility, and sleep/wake acceptance still require actual hardware checks.
