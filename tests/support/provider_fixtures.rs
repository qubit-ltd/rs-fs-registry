// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Observable provider fixtures for registration and creation contracts.

use std::sync::Arc;
use std::sync::Mutex;

#[cfg(feature = "async")]
use futures::channel::oneshot;
use qubit_fs::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
#[cfg(feature = "async")]
use qubit_fs_registry::AsyncFileSystemResolution;
use qubit_fs_registry::FileSystemConfig;
use qubit_fs_registry::FileSystemResolution;
use qubit_fs_registry::FileSystemSpec;
#[cfg(feature = "async")]
use qubit_spi::AsyncServiceProvider;
use qubit_spi::ProviderDescriptor;
#[cfg(feature = "async")]
use qubit_spi::ProviderFuture;
use qubit_spi::ProviderId;
use qubit_spi::ProviderMetadata;
use qubit_spi::ServiceProvider;
use qubit_spi::error::ProviderFailure;

use crate::common;

/// Shared controls record exact configurations and permit deterministic
/// scheduling.
#[derive(Clone)]
pub struct ObservedProvider {
    pub descriptor: Arc<Mutex<ProviderDescriptor>>,
    pub calls: Arc<Mutex<Vec<FileSystemConfig>>>,
    pub output_id: &'static str,
    pub fail: bool,
    pub on_create: Option<Arc<dyn Fn() + Send + Sync>>,
    #[cfg(feature = "async")]
    pub release: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
}

impl ObservedProvider {
    /// Creates a successful provider whose output identity initially matches
    /// registration.
    pub fn new(id: &'static str) -> Self {
        Self {
            descriptor: Arc::new(Mutex::new(ProviderDescriptor::new(
                ProviderId::new(id).expect("fixture ID"),
            ))),
            calls: Arc::new(Mutex::new(Vec::new())),
            output_id: id,
            fail: false,
            on_create: None,
            #[cfg(feature = "async")]
            release: Arc::new(Mutex::new(None)),
        }
    }

    /// Records one creation invocation before any controlled pause.
    fn record(&self, config: &FileSystemConfig) {
        self.calls.lock().expect("call log").push(config.clone());
        if let Some(callback) = &self.on_create {
            callback();
        }
    }

    /// Builds an unavailable leaf with a deliberately sensitive message and
    /// source.
    fn failure(&self) -> ProviderFailure<FsError> {
        ProviderFailure::unavailable(FsError::with_source(
            FsErrorKind::ProviderUnavailable,
            FsOperation::Provider,
            "token=leaf-secret",
            std::io::Error::other("password=source-secret"),
        ))
    }
}

impl ProviderMetadata for ObservedProvider {
    /// Returns the mutable fixture descriptor to detect improper recapture
    /// after registration.
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.lock().expect("descriptor").clone()
    }
}

impl ServiceProvider<FileSystemSpec> for ObservedProvider {
    /// Records the input and returns either a failure or properties-only
    /// facade.
    fn create_configured(
        &self,
        config: &FileSystemConfig,
    ) -> Result<FileSystemResolution, ProviderFailure<FsError>> {
        self.record(config);
        if self.fail {
            Err(self.failure())
        } else {
            Ok(common::sync_resolution(self.output_id))
        }
    }
}

#[cfg(feature = "async")]
impl AsyncServiceProvider<FileSystemSpec> for ObservedProvider {
    /// Records creation when polled and optionally waits for an explicit
    /// release signal.
    fn create_configured<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> ProviderFuture<'a, Result<AsyncFileSystemResolution, ProviderFailure<FsError>>> {
        Box::pin(async move {
            self.record(config);
            let release = self.release.lock().expect("release control").take();
            if let Some(release) = release {
                release.await.expect("release provider");
            }
            if self.fail {
                Err(self.failure())
            } else {
                Ok(common::async_resolution(self.output_id))
            }
        })
    }
}
