//! Background data download worker.
//!
//! Processes queued download jobs asynchronously. Currently a placeholder
//! for future background data-fetch operations.

use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};

/// Background worker that processes data download requests.
pub struct DownloadWorker {
    /// Application context providing database and config access.
    pub ctx: AppContext,
}

/// Arguments passed to a download job via the background queue.
#[derive(Deserialize, Debug, Serialize)]
pub struct DownloadWorkerArgs {
    /// The unique identifier of the user who triggered the download.
    pub user_guid: String,
}

#[async_trait]
impl BackgroundWorker<DownloadWorkerArgs> for DownloadWorker {
    fn build(ctx: &AppContext) -> Self {
        Self { ctx: ctx.clone() }
    }
    async fn perform(&self, _args: DownloadWorkerArgs) -> Result<()> {
        // TODO: Some actual work goes here...

        Ok(())
    }
}
