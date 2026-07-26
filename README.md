# Qubit FS Registry

Provider discovery, configuration, and SPI registry integration for
[`qubit-fs`](https://crates.io/crates/qubit-fs).

Applications that only need filesystem traits and value types should depend on
`qubit-fs`; add this crate only when runtime provider selection is required.
