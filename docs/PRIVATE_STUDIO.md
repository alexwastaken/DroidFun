# Private photography app

DroidFun Studio is a separate, owner-only download for photos and short demonstrations. It generates its remote readings locally, needs no Codex/Claude login, and offers all twelve screen kinds without model restrictions. The interface and rendered OLED frames omit demo labels. This build is not the customer app and must not replace the public downloads or updater feed.

Choose any screen, edit it, adjust the usage sliders, or cycle every screen. Hold readings for a stable photo. Clean view hides the controls; Escape returns. Text supports static/scrolling display; images use the existing local monochrome importer. Select a plugged-in USB droid to send the same rendered frame to its screen. Close other apps using that USB connection first. This app does not install or erase firmware.

Settings use `com.droidfun.studio`, separate from customer and Workshop settings. Mac downloads contain an app for Apple Silicon and Intel; Windows uses the x64 installer, which also runs through x64 emulation on supported Windows ARM systems. These private packages are unsigned and can trigger operating-system trust prompts.

Build with `node node_modules/@tauri-apps/cli/tauri.js build --features showcase --config src-tauri/tauri.showcase.conf.json --target TARGET --bundles app` on Mac, or `--bundles nsis` on Windows. The Studio Vite mode excludes customer model assets and uses its own basic interface. The private build workflow produces temporary installation artifacts, which should be removed after collecting the owner packages.

The website serves Studio through owner-authenticated `/api/admin/studio-downloads` routes. Installers are stored under the Worker's protected `/download-assets/` prefix, excluded from the public catalog, and sent with private/no-store headers. Keep this server authorization when replacing packages; hiding links alone is not protection.
