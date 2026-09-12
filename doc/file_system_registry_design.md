# Qubit FS Registry Design

Status: implemented contract for `qubit-fs-registry` 0.6, `qubit-fs` 0.8 and `qubit-spi` 0.12.

[中文](file_system_registry_design.zh_CN.md) · [User guide](user_guide.md)

## 1. Role

The registry is the application assembly boundary for runtime provider registration,
selection, complete configuration and URI resolution. It does not implement filesystem
operations, automatically discover providers, or resolve credential values itself.

```text
FileSystemConfig / ConnectionUri
    → credential and selection checks
    → SPI provider snapshot and creation/fallback
    → provider URI decoding and configured facade
    → validated resolution: filesystem + decoded Path + canonical Uri
```

Only `FileSystem` or `AsyncFileSystem` facades cross this boundary into applications;
operation SPI objects do not.

## 2. Goals and Non-goals

Support sync/async creation, provider-owned configuration interpretation, concrete
facades, credential-free canonical locations, deterministic selection/fallback/errors,
shared catalogs and consistent snapshots. Compose with `qubit-spi` instead of
reimplementing its general service registry.

The registry does not define operations, expose `FileSystemSpi`, validate every
operation's options, implement backend path algorithms, put credentials in locations
or properties, or automatically wrap blocking providers as async providers.

## 3. Provider and SPI Responsibilities

`FileSystemSpi` belongs to `qubit-fs` and supplies configured filesystem operation
primitives. `FileSystemProvider` belongs to this crate and names the configuration,
URI decoding and creation factory. Factories implement
`ServiceProvider<FileSystemSpec>` plus `ProviderMetadata`, or their async equivalent.
A factory's output contains a facade constructed through `FileSystem::from_spi` or
`AsyncFileSystem::from_spi`.

The descriptor ID identifies the factory, aliases route selections, the facade's
provider ID must equal the descriptor ID, and the filesystem ID identifies the
configured filesystem. Advertised schemes constrain canonical URIs, not factory names.

## 4. Concrete Resolution Types

`FileSystemResolution` owns a `FileSystem`, a decoded `Path`, and a canonical `Uri`.
`AsyncFileSystemResolution` has the same shape with an `AsyncFileSystem`.
Both provide a fallible `try_new`, borrowed getters and consuming `into_parts`.
Cloning a resolution clones facade handles while preserving their properties snapshot.

These public types are intentionally concrete: generic outputs would allow operation
SPI objects or arbitrary provider types to cross the registry boundary again.
A private generic implementation is possible only if future complexity justifies it.
The async resolution does not construct a separate async resource object.

## 5. Invariants and Validation Ownership

| Invariant | Owner |
| --- | --- |
| Returned facade provider ID equals the registered descriptor ID | Registry adapter, after factory success |
| Decoded path satisfies semantics, path-form constraints and limits | Resolution constructor delegates to facade properties |
| Canonical scheme is advertised; empty advertised schemes fail | Resolution constructor |
| Canonical URI has a credential-free structure | `Uri` construction contract |
| Backend-specific authority, canonicalization and URI/path relationship | Provider |
| Embedded selection agrees with explicit/default selection | Registry entry point |
| Connection secret and external credential reference do not coexist | Registry pre-creation validation |

Path and scheme validation are synchronous, local property checks. They perform no
`stat`, backend IO, or target-existence check. Providers may themselves perform IO to
create their configured facades. The constructor cannot infer backend-specific
URI/path relationships or the registered factory identity from its three inputs.

Fields stay private and cannot be replaced after validation. No resource convenience
or filesystem-only shortcut silently discards the decoded location. Callers needing
only the facade explicitly clone `resolution.file_system()`.

## 6. Synchronous API

`FileSystemRegistry` registers owned/shared self-described providers and offers
`resolve_config`, `resolve_uri`, `resolve_selected_config` and `resolve_default_config`.
Configuration is borrowed during synchronous creation. Convenience operations remain
inherent methods instead of creating another set of free functions.

## 7. Asynchronous API and Timing

Async entry points are ordinary functions accepting owned inputs and returning
`impl Future<Output = FileSystemRegistryResult<AsyncFileSystemResolution>> + Send + 'static`.
They validate and capture provider snapshots before returning. Factory creation starts
when the future is polled. An unpolled future can be dropped without creating a provider.

The future owns its configuration and provider handles, can outlive the registry,
and holds no catalog lock while awaiting creation. This API neither borrows secret-bearing
input for an uncontrolled lifetime nor relies on an implicit blocking adapter.
No custom public future type, runtime dependency, or `_async` method suffix is needed.

## 8. Selection and Fallback

`resolve_config` uses the embedded selection, otherwise a named URI-scheme selection.
`resolve_uri` uses the scheme. `resolve_selected_config` uses the supplied selection;
`resolve_default_config` uses the current default. Both reject a different embedded
selection. Conflicts fail before factory invocation; credential conflicts precede
selection conflicts. The initial registry default is `auto()` with `OnAbsence`, but
ordinary config/URI entry points never implicitly fall back to it.

Only the already parsed scheme is passed to `ProviderSelection::named`. Its parser
trims surrounding whitespace, lowercases ASCII, and requires a nonempty ASCII token
with alphanumeric endpoints and only letters, digits, `-`, `_`, `.`, `+` internally.
Authority, userinfo, query and raw URI text are never reinterpreted as selector text.
A URI-valid scheme outside that grammar fails before provider creation.

| Leaf failure | Meaning | OnAbsence continues |
| --- | --- | --- |
| Unsupported | Provider does not support the request | Yes |
| Unavailable | Provider/environment is unavailable | Yes |
| InvalidConfiguration | Provider-specific configuration is invalid | No |
| InitializationFailed | Initialization fails after accepting the request | No |

`Never` stops on any leaf failure; `OnAnyError` allows continuing after every class.
Named selections contain one candidate and never fall back. Unknown providers,
empty catalogs or no candidates are resolution errors, with no fabricated attempts.
Creation aggregates retain actual call order and the decisive, last attempted failure.
Registry identity mismatches are `InitializationFailed` leaves carrying
`FsErrorKind::ProviderContractViolation`, and follow the same explicit policy.

## 9. Catalog, Ownership and Future Caches

Registry clones share catalog and default selection. Every resolution keeps an owned,
point-in-time candidate snapshot. Default selection and candidates are captured together
by the atomic SPI snapshot API; later registrations/default changes cannot mix catalog
versions into an existing resolution. Created facades can outlive the registry.

This version adds no configured-filesystem cache. Providers decide whether their
existing contracts reuse internal resources. Any future cache must contain concrete
facades and satisfy these prerequisites:

- Keys include every non-sensitive dimension affecting identity, authority or capabilities.
- Inline/userinfo/query credentials make configurations uncacheable by default.
- External references require a provider-supplied safe identity distinguishing principal,
  scope and credential version or refresh epoch; references alone are insufficient.
- Secrets, including low-entropy secret hashes, never become visible keys.
- Without proof that authentication contexts are equivalent, sharing is forbidden.
- Rotation, revocation and scope changes invalidate eligibility for new resolutions.

The registry cannot invent cache eligibility by merely removing secret fields.
A canonical URI is a provider-scoped safe location for this resolution, not a universal
normalization, a cross-provider global identity, or a sufficient cache key. Two rooted
local filesystems can have identical canonical URIs but different authority and data.

## 10. Configuration and Credentials

`FileSystemConfig` contains a redacting `ConnectionUri`, optional selection,
validated non-sensitive options/metadata, and optional `CredentialRef`.
Options must not contain tokens/passwords; providers use controlled connection input
or external credential references instead.

Any detected embedded secret plus an external reference returns
`CredentialSourceConflict` before creation. There are no provider-specific exceptions.
Future support for multiple credential roles would require an explicit policy first.
The stable `reason_code()` is `credential_source_conflict`, with no payload or source.
Username-only userinfo is allowed with a reference because it contains no secret.

`CredentialRef` carries profile/environment/provider names, never actual secret values.
Providers may temporarily access connection input through explicitly named controlled
APIs, then return credential-free canonical URIs. Secrets cannot enter properties,
ordinary error formatting, or implicit serialization. Config clones retain protected
values; normal formatting delegates to redacting components. `ConnectionUri` is not
an ordinary cache key or a raw-string getter/deref abstraction.

## 11. Structured Errors

| Registry variant | Retained context | FsErrorKind conversion |
| --- | --- | --- |
| InvalidConfiguration | Static message | InvalidOptions |
| CredentialSourceConflict | No source | InvalidOptions |
| Registration | RegistrationError | Conflict |
| Selection | ProviderSelectionBuildError | InvalidUri |
| SelectionConflict | Requested and configured selections | InvalidOptions |
| Resolution | UnknownProviders, NoCandidates, EmptyRegistry, etc. | ProviderUnavailable |
| Creation | Ordered attempts, termination and decisive attempt | Decisive leaf kind |

Credential-resolution problems, unavailable providers and exhausted fallback are
represented by nested diagnostics, not additional top-level variants. Creation
conversion uses the decisive attempt's provider ID; other conversions do not invent one.
All conversions use the provider operation and retain the typed registry source.

`Display`/`Debug` format bounded safe context with immutable `Redactor::standard()`.
They do not read the application-default redactor, emit internal messages as raw text,
or recursively format provider sources. Static internal messages are treated as
sensitive fields. Explicit `Error::source()` access retains programmatic diagnostics;
callers control any further presentation. Operation-SPI contract violations remain
operation errors, rather than being rewritten as registry failures.

## 12. Boundary with qubit-spi

SPI owns IDs, aliases, priorities, registration, selection, provider lifecycle and
fallback mechanics. `FileSystemSpec` binds `Config = FileSystemConfig`, `Error = FsError`,
and concrete sync/async resolution outputs. No generic SPI catalog redesign is needed.

## 13. Local Provider Integration

`LocalFileSystemProvider` validates accepted `file:` configurations, rejects unsupported
credentials/authorities/options, decodes URI paths, assembles host/rooted facades and
returns canonical URI resolutions. Registry code does not directly construct local
operation SPI objects or invoke `qubit-local-files`.

## 14. Module Organization

```text
src/
├── lib.rs
├── credential_ref.rs
├── file_system_config.rs
├── file_system_resolution.rs
├── async_file_system_resolution.rs
├── file_system_registry.rs
├── async_file_system_registry.rs
├── file_system_provider.rs
├── async_file_system_provider.rs
├── file_system_spec.rs
├── file_system_registry_error.rs
└── internal/
    ├── mod.rs
    ├── registry_support.rs
    ├── provider_adapter.rs
    ├── validating_file_system_provider.rs
    └── validating_async_file_system_provider.rs
```

Shared stateless validation helpers stay private. Production/test visibility is not
expanded to accommodate tests. Public sync/async types stay distinct.

## 15. Verification Strategy

Public integration tests cover concrete output contracts, facade clone identity,
path semantics/constraints/exact limits, advertised and empty schemes, safe canonical
URIs/config formatting, all credential/selection precedence paths, fallback classification,
ordered/decisive failures, catalog sharing and snapshots during creation, genuinely
pending async creation, Send/static ownership, unpolled cancellation, registration-time
descriptor capture, and error conversion/context/source retention. No resource or
operation-SPI object is reintroduced into the public resolution API.

Cache tests become necessary only if a cache is introduced: inline credentials must
remain ineligible, distinct credential identities must be separated, and credential
rotation/revocation must invalidate reuse. No cache is introduced in this revision.

## 16. Documentation and Published Dependency Checks

Example versions come from manifest dependencies and
`package.metadata.documentation.dependencies`. README/guide Rust blocks are complete
programs. Async blocks are marked `<!-- registry-example: async -->` and checked with
an explicit minimal async dependency set. Unknown directives and unclosed fences fail.
Programs run in separate working directories and prepare their own input files.

`QUBIT_FS_REGISTRY_DOC_DEPS=local` allows declared local sources. `published` allows
only the tested crate locally, with other dependencies from package registries.
Both validate unique fs/registry/spi identities and the local provider's registry edge.
`check-published-docs.sh` requires Python 3.12 or newer and Rust 1.94. It packages and checks in an isolated Cargo environment without
inherited local patches; unavailable published requirements fail explicitly, never
silently switching to sibling source trees. Mechanical style and coverage checks do
not replace semantic test assertions or an item-by-item Rustdoc review.

[Migration](registry_contract_migration.md) · [User guide](user_guide.md)
