# Companion renderer boundary

The companion is a separate application, built through the optional `companion` feature. It consumes the engine's rendering services without introducing player, group, server or Agent policy types into `benilla-world`.

`lighting::SharedLightBuffer` is a published renderer binding for custom M2 preview views as well as the world. The engine creates and updates the storage buffer. A preview may clone its handle and pass it to the engine's material/rig constructors; it must not decode or independently overwrite its storage layout. The companion and engine are built from the same workspace revision, not independently versioned GPU ABIs. This is the reason for the explicit world-API-wall entry.

Other low-level effect internals remain covered by the existing API-wall ratchet. This does not raise its leak ceiling or exempt companion files from inspection. Loading-screen input handling likewise covers IME commits and their ordered WindowEvent records; standalone renderer wake events carry no gameplay actions.
