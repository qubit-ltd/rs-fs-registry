use qubit_fs::ConnectionUri;
use qubit_fs::{FsError, FsErrorKind, FsOperation};
use qubit_fs_registry::{
    AsyncFileSystemRegistry, AsyncFileSystemResolution, FileSystemConfig, FileSystemRegistryError,
    FileSystemSpec,
};
use qubit_spi::error::ProviderFailure;
use qubit_spi::{
    AsyncServiceProvider, ProviderDescriptor, ProviderFuture, ProviderId, ProviderMetadata,
};
use std::{
    future::Future,
    pin::pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
#[test]
fn test_async_registry_accepts_owned_config_without_borrowing_the_registry() {
    let future = {
        let registry = AsyncFileSystemRegistry::default();
        registry.resolve_config(FileSystemConfig::new(
            ConnectionUri::parse("missing:///resource").expect("URI should parse"),
        ))
    };
    drop(future);
}

#[test]
fn test_async_registry_future_is_static_and_polls_after_registry_is_dropped() {
    let future = {
        let registry = AsyncFileSystemRegistry::default();
        registry
            .register(AsyncFailingProvider)
            .expect("register provider");
        registry.resolve_config(FileSystemConfig::new(
            ConnectionUri::parse("async-failing:///resource").expect("URI should parse"),
        ))
    };
    let error = block_on(future).expect_err("provider should fail");
    assert!(matches!(error, FileSystemRegistryError::Creation(_)));
}

struct AsyncFailingProvider;
impl ProviderMetadata for AsyncFailingProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(ProviderId::new("async-failing").expect("provider id"))
    }
}
impl AsyncServiceProvider<FileSystemSpec> for AsyncFailingProvider {
    fn create_configured<'a>(
        &'a self,
        _: &'a FileSystemConfig,
    ) -> ProviderFuture<'a, Result<AsyncFileSystemResolution, ProviderFailure<FsError>>> {
        Box::pin(async {
            Err(ProviderFailure::unavailable(FsError::new(
                FsErrorKind::ProviderUnavailable,
                FsOperation::Provider,
                "unavailable",
            )))
        })
    }
}
fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(Noop));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
struct Noop;
impl Wake for Noop {
    fn wake(self: Arc<Self>) {}
}
