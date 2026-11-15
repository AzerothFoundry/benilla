# Mounted models sinking in Alterac Valley: legacy build backport

The legacy client displayed a rider and mount partly below the ground at Durotan's Rock and Frostwolf Village in Alterac Valley. Dismounting restored the player's normal appearance. The reporter confirmed the original client worked; after testing this backport with the complete zhCN Data directory, the reporter confirmed the mounted appearance was normal.

Mount child transforms are relative to their host, not world positions. The legacy ground-clamp and swim queries treated those child models as independent units. A collision probe at the child's local origin interpreted as a world point could therefore introduce an extra vertical offset into the mounted hierarchy. Both queries now exclude `MountBody`. The host still uses normal world collision; there are no map-specific offsets or changes to game data.

This is a backport of the mount filtering and regression from upstream [PR #2992](https://github.com/samwhosung/benilla/pull/2992), merged as `a1079ce6f7831f243447ed21391f629dc2ce6be5` on 2026-11-04. It is not a new upstream fix. The Foundry main source already includes that upstream implementation; the installed legacy game-client executable had not received it.

Validation on 2026-11-15:

- With the new regression and the old query, the physics test failed: child local Y was -1 instead of 0.
- After excluding mount children, all 42 tests under `net::motion::spline::` passed, including the new regression.
- macOS arm64 release build succeeded. A separate startup check stayed at the login screen for ten seconds without ERROR or panic.
- The reporter confirmed normal appearance after switching to the test executable, using the same complete zhCN Data. This does not certify every mount, map, or terrain boundary.

The test executable was built from the legacy `e369e1f` checkout with the user's pre-existing local client changes preserved. Only the mount fix and its regression are committed here; unrelated local changes, binaries, credentials, game data, and screenshots are not included.
