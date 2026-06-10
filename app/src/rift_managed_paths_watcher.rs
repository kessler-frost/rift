use std::path::{Path, PathBuf};
#[cfg(not(target_family = "wasm"))]
use std::{fs, sync::Arc, time::Duration};

#[cfg(not(target_family = "wasm"))]
use notify_debouncer_full::notify::{RecursiveMode, WatchFilter};
use repo_metadata::RepositoryUpdate;
#[cfg(not(target_family = "wasm"))]
use repo_metadata::TargetFile;
#[cfg(not(target_family = "wasm"))]
use riftui::ModelHandle;
use riftui::{Entity, ModelContext, SingletonEntity};
#[cfg(not(target_family = "wasm"))]
use watcher::{BulkFilesystemWatcher, BulkFilesystemWatcherEvent};

/// Duration between filesystem watch events for the Warp managed paths watcher, in milliseconds.
#[cfg(not(target_family = "wasm"))]
const RIFT_MANAGED_PATHS_WATCHER_DEBOUNCE_MILLI_SECS: u64 = 500;

pub(crate) fn warp_data_dir() -> PathBuf {
    rift_core::paths::data_dir()
}

#[cfg(target_family = "wasm")]
pub(crate) fn ensure_rift_watch_roots_exist() {}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn ensure_rift_watch_roots_exist() {
    let data_dir = warp_data_dir();
    if let Err(err) = fs::create_dir_all(&data_dir) {
        log::warn!(
            "Failed to create Warp data directory {}: {err}",
            data_dir.display()
        );
    }

    let config_local_dir = rift_core::paths::config_local_dir();
    if config_local_dir != data_dir {
        if let Err(err) = fs::create_dir_all(&config_local_dir) {
            log::warn!(
                "Failed to create Warp config directory {}: {err}",
                config_local_dir.display()
            );
        }
    }
}

#[cfg_attr(target_family = "wasm", allow(dead_code))]
pub(crate) fn rift_home_skills_dir() -> Option<PathBuf> {
    rift_core::paths::rift_home_skills_dir()
}

#[cfg_attr(target_family = "wasm", allow(dead_code))]
pub(crate) fn repository_update_touches_path(update: &RepositoryUpdate, path: &Path) -> bool {
    repository_update_paths(update).any(|candidate| candidate == path)
}

#[cfg_attr(target_family = "wasm", allow(dead_code))]
pub(crate) fn repository_update_touches_prefix(update: &RepositoryUpdate, prefix: &Path) -> bool {
    repository_update_paths(update).any(|candidate| candidate.starts_with(prefix))
}

#[cfg_attr(target_family = "wasm", allow(dead_code))]
fn repository_update_paths(update: &RepositoryUpdate) -> impl Iterator<Item = &Path> {
    update
        .added
        .iter()
        .map(|target| target.path.as_path())
        .chain(update.modified.iter().map(|target| target.path.as_path()))
        .chain(update.deleted.iter().map(|target| target.path.as_path()))
        .chain(update.moved.iter().flat_map(|(to_target, from_target)| {
            [to_target.path.as_path(), from_target.path.as_path()]
        }))
}

#[cfg(not(target_family = "wasm"))]
fn filesystem_event_to_repository_update(event: &BulkFilesystemWatcherEvent) -> RepositoryUpdate {
    RepositoryUpdate {
        added: event
            .added
            .iter()
            .cloned()
            .map(|path| TargetFile::new(path, false))
            .collect(),
        modified: event
            .modified
            .iter()
            .cloned()
            .map(|path| TargetFile::new(path, false))
            .collect(),
        deleted: event
            .deleted
            .iter()
            .cloned()
            .map(|path| TargetFile::new(path, false))
            .collect(),
        moved: event
            .moved
            .iter()
            .map(|(to_path, from_path)| {
                (
                    TargetFile::new(to_path.clone(), false),
                    TargetFile::new(from_path.clone(), false),
                )
            })
            .collect(),
        commit_updated: false,
        index_lock_detected: false,
        remote_ref_updated: false,
    }
}

#[cfg(target_family = "wasm")]
#[allow(dead_code)]
pub(crate) enum RiftManagedPathsWatcherEvent {}

#[cfg(not(target_family = "wasm"))]
pub(crate) enum RiftManagedPathsWatcherEvent {
    FilesChanged(RepositoryUpdate),
}

#[cfg(not(target_family = "wasm"))]
pub(crate) struct RiftManagedPathsWatcher {
    _watcher: ModelHandle<BulkFilesystemWatcher>,
}

#[cfg(target_family = "wasm")]
pub(crate) struct RiftManagedPathsWatcher;

#[cfg(not(target_family = "wasm"))]
impl RiftManagedPathsWatcher {
    pub(crate) fn new(ctx: &mut ModelContext<Self>) -> Self {
        Self::new_internal(ctx, true)
    }

    #[cfg(test)]
    pub(crate) fn new_for_testing(ctx: &mut ModelContext<Self>) -> Self {
        Self::new_internal(ctx, false)
    }

    fn new_internal(ctx: &mut ModelContext<Self>, should_register_watcher: bool) -> Self {
        let watcher = if should_register_watcher {
            ctx.add_model(|ctx| {
                BulkFilesystemWatcher::new(
                    Duration::from_millis(RIFT_MANAGED_PATHS_WATCHER_DEBOUNCE_MILLI_SECS),
                    ctx,
                )
            })
        } else {
            ctx.add_model(|_| BulkFilesystemWatcher::new_for_test())
        };
        ctx.subscribe_to_model(&watcher, Self::handle_fs_event);

        if should_register_watcher {
            let data_dir = warp_data_dir();
            let config_local_dir = rift_core::paths::config_local_dir();
            let should_register_config_local_dir = config_local_dir != data_dir;
            let worktrees_dir = data_dir.join("worktrees");
            // Safe to use for both directory registration and event emission.
            // If this rejects `worktrees_dir`, every descendant should be rejected too,
            // so the recursive watcher never prunes an ancestor needed to reach an allowed path.
            let filter = Arc::new(move |path: &Path| !path.starts_with(&worktrees_dir));
            Self::register_path(
                ctx,
                &watcher,
                data_dir.clone(),
                WatchFilter::with_filter(filter.clone(), filter),
                RecursiveMode::Recursive,
                "Warp data directory",
            );
            if should_register_config_local_dir {
                Self::register_path(
                    ctx,
                    &watcher,
                    config_local_dir.clone(),
                    WatchFilter::accept_all(),
                    RecursiveMode::Recursive,
                    "Warp config directory",
                );
            }
            if let Some(rift_home_skills_dir) = rift_home_skills_dir() {
                if rift_home_skills_dir.exists()
                    && !rift_home_skills_dir.starts_with(&data_dir)
                    && (!should_register_config_local_dir
                        || !rift_home_skills_dir.starts_with(&config_local_dir))
                {
                    Self::register_path(
                        ctx,
                        &watcher,
                        rift_home_skills_dir,
                        WatchFilter::accept_all(),
                        RecursiveMode::Recursive,
                        "Warp home skills directory",
                    );
                }
            }
        }

        Self { _watcher: watcher }
    }

    fn register_path(
        ctx: &mut ModelContext<Self>,
        watcher: &ModelHandle<BulkFilesystemWatcher>,
        directory_path: PathBuf,
        watch_filter: WatchFilter,
        recursive_mode: RecursiveMode,
        description: &'static str,
    ) {
        let registration_path = directory_path.clone();
        let registration = watcher.update(ctx, |watcher, _ctx| {
            watcher.register_path(&registration_path, watch_filter, recursive_mode)
        });

        ctx.spawn(registration, move |_, result, _ctx| {
            if let Err(err) = result {
                log::warn!(
                    "Failed to start watching {description} {}: {err}",
                    directory_path.display()
                );
            }
        });
    }

    fn handle_fs_event(
        &mut self,
        event: &BulkFilesystemWatcherEvent,
        ctx: &mut ModelContext<Self>,
    ) {
        let update = filesystem_event_to_repository_update(event);
        if !update.is_empty() {
            ctx.emit(RiftManagedPathsWatcherEvent::FilesChanged(update));
        }
    }
}

#[cfg(target_family = "wasm")]
impl RiftManagedPathsWatcher {
    pub(crate) fn new(_ctx: &mut ModelContext<Self>) -> Self {
        Self
    }

    #[cfg(test)]
    pub(crate) fn new_for_testing(_ctx: &mut ModelContext<Self>) -> Self {
        Self
    }
}

impl Entity for RiftManagedPathsWatcher {
    type Event = RiftManagedPathsWatcherEvent;
}

impl SingletonEntity for RiftManagedPathsWatcher {}
