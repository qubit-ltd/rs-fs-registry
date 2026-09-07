# Registry Contract Migration

For application and provider authors migrating to the filesystem facade/SPI contracts
in registry 0.2. The current documentation/test corrections clarify existing behavior;
they do not introduce a new public API or change selection/fallback semantics.

[中文](registry_contract_migration.zh_CN.md) · [User guide](user_guide.md)

## 1. Credential Source Conflicts

An embedded URI secret and `CredentialRef` cannot both occupy the credential slot.
The registry rejects this before creating a provider with
`FileSystemRegistryError::CredentialSourceConflict`. Its stable reason code is
`credential_source_conflict`, containing no URI/reference/secret payload; its source
is `None`. Match the variant or code rather than an old InvalidConfiguration message.
A username without a secret may coexist with a reference.

References identify a provider-recognized profile, environment-variable names or
external provider ID; never store tokens, passwords or private keys in them.

## 2. Safe Diagnostics and Typed Sources

Ordinary Display/Debug never recursively expand provider sources, never emit internal
messages as raw text, and retain bounded selector/provider/reason-code context.
Internal messages pass through sensitive-field redaction. Every formatting operation
uses immutable `Redactor::standard()`, not the replaceable application-default redactor.
Apply any stricter application policy at your own presentation boundary.
Use explicit `Error::source()` and downcasts for programmatic causal-chain handling.

## 3. Atomic Default Snapshots

Clones share the catalog and default selection. A default resolution captures the
selection and candidate handles together under one catalog snapshot, then creates
providers outside the lock. Later registration/default changes affect later snapshots.
Existing sync or async resolutions keep their candidates even if the originating
registry is cloned, changed or dropped. Request a new resolution to observe new state.
Async snapshots are captured when the method returns its future, not on first poll;
provider creation remains lazy and starts on polling.

## 4. Failure Classification and Fallback

| Failure kind | Meaning | OnAbsence continues |
| --- | --- | --- |
| Unsupported | Request is not supported | Yes |
| Unavailable | Provider/environment unavailable | Yes |
| InvalidConfiguration | Provider-specific configuration rejected | No |
| InitializationFailed | Initialization failed after accepting the request | No |

Never stops after the first failure; OnAnyError may continue after every leaf failure.
Named selections have one candidate and never fall back. Aggregates preserve actual
attempt order; the last actual attempt is decisive. UnknownProviders, NoCandidates
and EmptyRegistry happen before invocation and do not create fictional attempts.
Use classification and policy when deciding whether to retry, not error strings.
A returned identity mismatch is InitializationFailed/ProviderContractViolation.

## 5. Canonical URI Scope

The canonical URI is the selected provider's credential-free location for this
resolution, distinct from the connection input. Providers establish URI/path semantics.
`try_new` checks path constraints/semantics/limits and requires the returned facade to
advertise the canonical scheme; an empty scheme list fails. The `Uri` type ensures its
safe structure. Providers own authority and path normalization and capability semantics.

The canonical URI is not a universal normalization or cross-provider global identity.
Two rooted providers can return the same URI for different files. It can correlate
this resolution safely, but cannot recover original connection text or independently
identify authenticated configurations or future cache entries.

## 6. Scheme-derived Selection Grammar

Without an explicit selection, resolve_config passes only the parsed connection scheme
to `ProviderSelection::named`. Parsing trims surrounding whitespace and lowercases
ASCII. Tokens must be nonempty, have ASCII alphanumeric endpoints, and contain only
ASCII letters/digits or `-`, `_`, `.`, `+` internally. Slash, colon, internal whitespace,
non-ASCII text and other punctuation fail before creation. Authority/userinfo/query
and raw URI text are not concatenated or reinterpreted as selector input.

## 7. Migration Checklist

1. Match CredentialSourceConflict or its stable reason code.
2. Replace dependencies on complete display strings with typed errors and failure kinds.
3. Apply stricter redaction at application output boundaries, rather than replacing the
   application-default redactor and expecting registry diagnostics to follow it.
4. Obtain a new resolution after catalog/default changes when new state is required.
5. Use the documented selector grammar; do not route by userinfo, authority or query.
6. Keep the facade with its decoded path and canonical URI; preserve provider-owned semantics.
7. Use fs 0.3, registry 0.2, local provider 0.2 and spi 0.11 in related examples. Run
   `check-published-docs.sh` for isolated published-dependency verification, independently
   of the locally patched package build used during coordinated development.

[Design](file_system_registry_design.md) · [User guide](user_guide.md)
