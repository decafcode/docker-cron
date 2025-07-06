mod crontab;

use std::{env, path::Path, rc::Rc};

use bollard::{
    errors::Error::DockerContainerWaitError,
    query_parameters::{StartContainerOptions, WaitContainerOptions},
    Docker,
};
use chrono::prelude::*;
use cron::Schedule;
use tokio::{
    runtime::{self, LocalOptions},
    signal::unix::{signal, SignalKind},
    task::JoinSet,
    time::sleep,
    time::Duration,
};
use tokio_stream::StreamExt;
use tracing::{debug, info, level_filters::LevelFilter, warn};
use tracing_subscriber::EnvFilter;

use crate::crontab::{load_crontab, CronJob};

#[tracing::instrument(
    skip_all,
    fields(schedule = schedule.to_string(), container = container)
)]
async fn schedule_job(schedule: Schedule, container: String, docker: Rc<Docker>) {
    debug!("Scheduling job");

    loop {
        let now = Utc::now();
        let next = schedule.after(&now).next().unwrap();
        let dt = next - now;
        let dt_millis: u64 = dt.num_milliseconds().try_into().unwrap();

        // Assume that the clock isn't being manipulated while we're asleep.

        debug!(dt_millis, "Sleeping until next launch");
        sleep(Duration::from_millis(dt_millis)).await;
        debug!("Wakeup");

        let result = docker
            .start_container(&container, None::<StartContainerOptions>)
            .await;

        if let Err(error) = result {
            warn!(error = ?error, "Failed to start container");

            continue;
        }

        let result = docker
            .wait_container(&container, None::<WaitContainerOptions>)
            .next()
            .await;

        // Overly elaborate scheme of potential failure responses...

        match result {
            None => warn!("No response to poll request on Docker API"),
            Some(result) => match result {
                Err(error) => match error {
                    DockerContainerWaitError {
                        error: error_msg,
                        code: status_code,
                    } => {
                        if error_msg.is_empty() {
                            warn!(status_code, "Job did not succeed")
                        } else {
                            warn!(error_msg, "Container wait request returned error message")
                        }
                    }
                    _ => warn!(error = ?error, "Error waiting for container completion"),
                },
                Ok(_) => debug!("Successful exit"),
            },
        }
    }
}

async fn async_main(jobs: Vec<CronJob>) -> Result<(), anyhow::Error> {
    // Connect to Docker daemon

    let docker = Rc::new(Docker::connect_with_defaults()?);

    info!("Connecting to Docker");
    docker.ping().await?;
    info!("Docker connection OK, starting scheduler");

    // Start scheduled tasks

    let mut signal = signal(SignalKind::terminate())?;
    let mut join_set: JoinSet<()> = JoinSet::new();

    for job in jobs {
        join_set.spawn_local(schedule_job(job.schedule, job.command, docker.clone()));
    }

    // Wait for SIGTERM

    signal.recv().await;
    info!("Stopping due to SIGTERM");

    Ok(())

    // join_set drops here and this aborts all the tasks
}

fn main() -> Result<(), anyhow::Error> {
    let log_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?;

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(log_filter)
        .init();

    let filename = env::args().nth(1).expect("Crontab path was not supplied");
    let path = Path::new(&filename);
    let jobs = load_crontab(path)?;

    // Nothing about our work is CPU-bound, so we don't need multi-threading.
    // Local scheduler requires the tokio_unstable build flag.

    let rt = runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build_local(LocalOptions::default())?;

    rt.block_on(async_main(jobs))
}
