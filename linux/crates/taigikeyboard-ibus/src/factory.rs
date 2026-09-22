//! `org.freedesktop.IBus.Factory` at `/org/freedesktop/IBus/Factory`
//! (roadmap L1): the daemon calls `CreateEngine(name)` once per client
//! context that selects this input method and gets the object path of a
//! fresh engine, `/org/freedesktop/IBus/Engine/<n>` (ibus
//! `src/ibusfactory.c`, `ibus_factory_real_create_engine`).

use crate::engine::{Engine, Service};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use taigi_linux_core::Runtime;
use zbus::zvariant::OwnedObjectPath;
use zbus::{fdo, interface, ObjectServer};

/// The engine name the component XML declares; every other name is refused
/// the way ibus refuses it (`Cannot find engine`).
pub const ENGINE_NAME: &str = "taigikeyboard";
pub const FACTORY_PATH: &str = "/org/freedesktop/IBus/Factory";
pub const ENGINE_PATH_PREFIX: &str = "/org/freedesktop/IBus/Engine/";

pub struct Factory {
    runtime: Arc<Runtime>,
    created: AtomicUsize,
}

impl Factory {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            runtime,
            created: AtomicUsize::new(0),
        }
    }
}

#[interface(name = "org.freedesktop.IBus.Factory", spawn = false)]
impl Factory {
    async fn create_engine(
        &self,
        name: String,
        #[zbus(object_server)] server: &ObjectServer,
    ) -> fdo::Result<OwnedObjectPath> {
        if name != ENGINE_NAME {
            return Err(fdo::Error::Failed(format!("Cannot find engine {name}")));
        }
        let ordinal = self.created.fetch_add(1, Ordering::Relaxed) + 1;
        let path = OwnedObjectPath::try_from(format!("{ENGINE_PATH_PREFIX}{ordinal}"))
            .map_err(|error| fdo::Error::Failed(error.to_string()))?;
        let token = self.runtime.allocate_token();
        server
            .at(
                &path,
                Engine::new(Arc::clone(&self.runtime), token, path.clone()),
            )
            .await
            .map_err(|error| fdo::Error::Failed(error.to_string()))?;
        server
            .at(
                &path,
                Service::new(Arc::clone(&self.runtime), token, path.clone()),
            )
            .await
            .map_err(|error| fdo::Error::Failed(error.to_string()))?;
        log::info!("factory.create_engine path={path} token={token:?}");
        Ok(path)
    }
}
