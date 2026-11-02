# Windows 0.10.8 publication

Windows customers remained on 0.10.0 because the shared Mac/Windows feed was not advanced while Mac Developer ID signing was unavailable. Windows 0.10.8 uses a separate `distribution/windows-updater.json` feed, with the existing public verification key and version-bound signatures. Existing Windows 0.10.0–0.10.7 installations need one manual installer upgrade; a static feed cannot replace the endpoint compiled into those clients. The website explains this step. Future Windows builds can publish independently of Mac.

Source: `5029841a33555f754d17810b2ead09056c50e304`. This carries forward the 0.10.7 simple clock, separate calendar and smoother scrolling. No firmware is installed by the app update; Workshop and its private update feed are separate.

Both native Windows builds passed 174 tests each, installed their NSIS packages, matched installed executable bytes and architecture, opened a native window and remained running. Frontend type checks, customer security boundary checks and release-tooling tests passed. Version-bound updater signatures, hashes, GitHub release digests and actual live website download bytes match. Both public installers and the two-target Windows feed are published. The shared legacy feed, other catalog files, products and store settings are unchanged. All six checked private Workshop/admin endpoints return 401 to anonymous requests.

The first build caught a version-bump typo in the digest dependency lock entry before producing installers. It was restored to its upstream version, and both published builds use the corrected source above. No failed-build packages were published.

Windows publisher signing is still unavailable. Installer smoke checks do not establish a real Windows in-app upgrade, remote user settings preservation, physical USB behavior or live account integration. The user's PC was not remotely operated. Mac/Linux remain on public 0.10.0; local Mac 0.10.7 and Meep firmware 1.3.3 are unchanged.

See the adjacent native verification files, live feed/download records and publication metadata for exact sources, URLs and checks. No private credentials or physical droid IDs are included.
