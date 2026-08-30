# Migrating to acorde v0.5

The v0.5 release completes the production-hardening work from the roadmap. The renderer and WASM
JSON contracts are compatible with v0.4: existing calls, `RenderMetadata.contract_version`, and
stable `address_bounds` fields are unchanged.

The browser verification suite now includes a Chromium device-scale-factor-2 baseline. Consumers
do not need to change their rendering code; hosts may continue to choose their own device scale
factor and CSS sizing. The published WASM artifact is also checked against a one-megabyte CI size
two-megabyte CI budget to catch accidental dependency or binary growth.
