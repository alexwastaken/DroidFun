# Droid identification and private inventory

The app recognizes a prepared droid from its USB firmware handshake. A remembered local profile takes precedence over a registry hint. An internet connection or customer account is not required to operate the droid.

For a single connected older board without a model identity or saved profile, the native app can POST that board's 16-character ID to `https://droidfun.io/api/droids/identify`. The response contains only `model`, one of Boo, Claw, Meep or Nimbus, or null. An unavailable service falls back to choosing the model once. Requests have a two-second timeout, a 256-byte response limit and no redirects. Pause online data blocks and cancels setup lookups. Before applying a result, the app checks that the board is still attached, the network policy is still current, and no other droid has been selected.

A lookup is a public setup hint. Board IDs are identifiers, not proof of ownership or device authenticity. Hints never create firmware identity, grant firmware-writing permissions, or change existing saved screens. They do not record customer connection history.

## Owner access

The full inventory is served only through owner-authenticated dashboard routes or an explicitly paired private Workshop. Customer builds contain no database credentials, owner sessions, Workshop tokens or inventory-list commands. The website Worker accesses Cloudflare D1 through its server-side binding. There is no database query endpoint available to the app.

The owner creates a one-use, ten-minute code in Dashboard → Droids and pastes it into private Workshop → Droid list. It grants a revocable thirty-day Workshop session scoped to inventory read/write. This session cannot access owner administration, orders or payments. Changing the owner password invalidates its signature. Codes and token digests are not a substitute for owner sign-in; anonymous requests cannot create them.

Workshop stores its session outside its app bundle in private runtime storage, with owner-only directory/file permissions on Mac. Session credentials stay in the native process and never enter webview responses, events or localStorage. Releasing the customer app or the private Workshop package does not include that runtime storage.

## Inventory behavior

Workshop records model, board ID, firmware and actual wiring after inspection. Older boards can be recorded after the owner chooses their model, without reinstalling firmware. Offline records are queued locally and retried after connection returns. A board becomes prepared only after Workshop's reconnect and readable-screen confirmations. Changing the recorded configuration clears previous preparation status; owner retirement and private notes survive sync.

“Connected here” describes USB presence on the Workshop computer. Inventory dates describe Workshop checks. The registry does not claim that customer droids are currently online. Previously programmed boards can be added by reconnecting them to Workshop once.

The public single-device lookup is rate-limited, accepts no SQL or arbitrary fields, and returns no inventory rows, notes, dates, status, session details, or fleet counts. The owner list uses bounded pagination. These boundaries are enforced by the service; hiding dashboard controls is not an authorization mechanism.

## Bundled 3D previews

The app includes the same four GLB models, finishes and preview hardware used on the website. Model files load locally. Motion stops when hidden or paused, follows reduced-motion preferences, and supports mouse/touch dragging and keyboard rotation. A bundled still image remains available if WebGL cannot start. The separate OLED preview remains the readable representation of the selected screen.
