// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs_registry::CredentialRef;

/// Verifies credential references retain only selection data.
#[test]
fn test_credential_reference_variants_are_comparable() {
    assert_eq!(CredentialRef::DefaultChain, CredentialRef::DefaultChain);
    assert_eq!(
        CredentialRef::Profile("production".to_owned()),
        CredentialRef::Profile("production".to_owned()),
    );
    assert_eq!(
        CredentialRef::Environment {
            access_key: "ACCESS_KEY".to_owned(),
            secret_key: "SECRET_KEY".to_owned(),
        },
        CredentialRef::Environment {
            access_key: "ACCESS_KEY".to_owned(),
            secret_key: "SECRET_KEY".to_owned(),
        },
    );
    assert_eq!(
        CredentialRef::Provider("vault".to_owned()),
        CredentialRef::Provider("vault".to_owned()),
    );
}
