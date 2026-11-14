# AzerothFoundry client fork

This fork builds on samwhosung/benilla and retains standard1.12.1 server compatibility as its boundary. Realm-specific bot intelligence stays server-side; it does not require client modifications. Cross-realm compatibility is a goal, not a claim that every server configuration has been tested.

The fork carries Chinese interface/input fixes and realm interoperability repairs. The optional `companion` feature and standalone `realm-companion` crate add a macOS desktop avatar backed by the Agent status protocol, without a game login. Setup and controls are in [the companion README](crates/realm-companion/README.md).

Game data and interface assets are read from each user's own install and are never distributed here. Build the normal client following the root README. Keep reusable protocol/fidelity fixes small for upstream review; desktop-specific behavior remains feature-gated or in standalone files.
