//! The writer task: runs queued writes in order, retrying while the database is unavailable.

use std::{sync::Arc, time::Duration};

use ro_persistence::RepositoryError;
use tokio::sync::mpsc;
use tracing::{error, warn};

use super::{
    map_repository::MapRepository,
    recent_writes::RecentWrites,
    write_dto::{Write, WriteKind},
};

/// First and longest wait between retries of a failed write.
const RETRY_FIRST: Duration = Duration::from_millis(50);
const RETRY_MAX: Duration = Duration::from_secs(5);

async fn apply(repo: &dyn MapRepository, write: &Write) -> Result<(), RepositoryError> {
    match &write.kind {
        WriteKind::Position { map, pos } => repo.update_position(write.char_id, map, *pos).await,
        WriteKind::Play(snapshot) => {
            repo.save_play_state(
                write.char_id,
                &snapshot.inventory,
                &snapshot.looks,
                &snapshot.progress,
                &snapshot.skills,
            )
            .await
        }
    }
}

/// Runs writes in queue order. After a write lands, its recent entry is cleared unless a newer
/// write of the same kind replaced it.
///
/// Only `Unavailable` (the database is down or busy) is retried, with backoff, until the write
/// lands or a newer write of the same kind for the same character supersedes it: the
/// char-server reads the stored map, so a lost write after a warp would send the client to the
/// wrong map. Any other error can't succeed on retry and would stall every queued write behind
/// it: `NotFound` (the character was deleted) is done, anything else is logged and dropped. A
/// dropped write keeps its recent entry, so a re-entry in this process still sees it.
pub(super) async fn run_writer(
    repo: Arc<dyn MapRepository>,
    recent: RecentWrites,
    mut rx: mpsc::UnboundedReceiver<Write>,
) {
    while let Some(write) = rx.recv().await {
        let mut wait = RETRY_FIRST;
        loop {
            match apply(repo.as_ref(), &write).await {
                Ok(()) | Err(RepositoryError::NotFound) => {
                    recent.landed(&write);
                    break;
                }
                Err(RepositoryError::Unavailable(err)) => {
                    warn!(
                        "Failed to persist char {} (retrying in {:?}): {}",
                        write.char_id.0, wait, err
                    );
                    tokio::time::sleep(wait).await;
                    wait = (wait * 2).min(RETRY_MAX);
                    if !recent.is_newest(&write) {
                        // A newer write of this kind for this character is queued behind it.
                        break;
                    }
                }
                Err(err) => {
                    error!(
                        "Dropping write for char {} that can't succeed: {}",
                        write.char_id.0, err
                    );
                    break;
                }
            }
        }
    }
}
