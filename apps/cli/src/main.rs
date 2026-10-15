use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use voids3_app::App;
use voids3_core::{Credentials, Profile};
use voids3_transfer::{Direction, Status};

#[derive(Parser)]
#[command(version, about = "VoidS3 — fully vibecoded S3 storage client")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// List non-secret profiles, or save a profile JSON using environment-provided credentials.
    Connections {
        #[command(subcommand)]
        command: Connections,
    },
    /// List one page; pass the returned continuation token to continue.
    Ls {
        connection: String,
        #[arg(default_value = "")]
        prefix: String,
        #[arg(long)]
        token: Option<String>,
    },
    Buckets {
        connection: String,
    },
    Upload {
        connection: String,
        local: PathBuf,
        key: String,
    },
    Download {
        connection: String,
        key: String,
        local: PathBuf,
    },
    Presign {
        connection: String,
        key: String,
        #[arg(long, default_value_t = 3600)]
        seconds: u64,
        #[arg(long)]
        upload: bool,
    },
    /// Delete a single object. Requires --yes; directories are not recursively deleted.
    Rm {
        connection: String,
        key: String,
        #[arg(long)]
        yes: bool,
    },
    /// Copy without deleting the source; destination must be absent at preflight.
    Cp {
        connection: String,
        source: String,
        destination: String,
    },
    /// Preview conservatively. --apply-additions copies missing files only, preserving conflicts.
    Sync {
        connection: String,
        local: PathBuf,
        #[arg(default_value = "")]
        prefix: String,
        #[arg(long,value_enum,default_value_t=SyncMode::Upload)]
        mode: SyncMode,
        #[arg(long)]
        apply_additions: bool,
    },
    Queue {
        #[command(subcommand)]
        command: Queue,
    },
}
#[derive(Subcommand)]
enum Connections {
    List,
    Save {
        file: PathBuf,
    },
    Delete {
        id: String,
        #[arg(long)]
        yes: bool,
    },
    Test {
        id: String,
    },
}
#[derive(Subcommand)]
enum Queue {
    List,
    Resume { id: String },
    Cancel { id: String },
    Clear,
}
#[derive(Clone, Copy, ValueEnum)]
enum SyncMode {
    Upload,
    Download,
    Bidirectional,
}
impl From<SyncMode> for voids3_sync::Mode {
    fn from(value: SyncMode) -> Self {
        match value {
            SyncMode::Upload => Self::Upload,
            SyncMode::Download => Self::Download,
            SyncMode::Bidirectional => Self::Bidirectional,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let app = App::open()?;
    match cli.command {
        Command::Connections { command } => match command {
            Connections::List => {
                println!("{}", serde_json::to_string_pretty(&app.store.profiles()?)?)
            }
            Connections::Save { file } => {
                let profile: Profile = serde_json::from_slice(&tokio::fs::read(file).await?)?;
                let credentials = Credentials {
                    access_key: std::env::var("VOIDS3_ACCESS_KEY")
                        .context("Set VOIDS3_ACCESS_KEY")?,
                    secret_key: std::env::var("VOIDS3_SECRET_KEY")
                        .context("Set VOIDS3_SECRET_KEY")?,
                    session_token: std::env::var("VOIDS3_SESSION_TOKEN").ok(),
                };
                app.store.save_profile(&profile, Some(&credentials))?;
                println!("Saved {} ({})", profile.name, profile.id);
            }
            Connections::Delete { id, yes } => {
                if !yes {
                    bail!("Pass --yes to remove the profile and OS keychain entry");
                }
                app.delete_profile(&id)?;
            }
            Connections::Test { id } => {
                app.client(&id).await?.test().await?;
                println!("Bucket access verified");
            }
        },
        Command::Ls {
            connection,
            prefix,
            token,
        } => println!(
            "{}",
            serde_json::to_string_pretty(
                &app.client(&connection)
                    .await?
                    .list(&prefix, token, false)
                    .await?
            )?
        ),
        Command::Buckets { connection } => println!(
            "{}",
            serde_json::to_string_pretty(&app.client(&connection).await?.buckets().await?)?
        ),
        Command::Upload {
            connection,
            local,
            key,
        } => {
            let client = app.client(&connection).await?;
            let id = app
                .engine
                .enqueue(client, Direction::Upload, local, key, app.settings()?)
                .await?;
            wait(&app, &[id]).await?;
        }
        Command::Download {
            connection,
            key,
            local,
        } => {
            let client = app.client(&connection).await?;
            let id = app
                .engine
                .enqueue(client, Direction::Download, local, key, app.settings()?)
                .await?;
            wait(&app, &[id]).await?;
        }
        Command::Presign {
            connection,
            key,
            seconds,
            upload,
        } => println!(
            "{}",
            app.client(&connection)
                .await?
                .presign(&key, upload, seconds)
                .await?
        ),
        Command::Rm {
            connection,
            key,
            yes,
        } => {
            if !yes {
                bail!("Pass --yes to delete this object");
            }
            let client = app.client(&connection).await?;
            let info = client.head(&key).await?;
            client.delete(&key, Some(&info.etag)).await?;
        }
        Command::Cp {
            connection,
            source,
            destination,
        } => {
            let client = app.client(&connection).await?;
            let info = client.head(&source).await?;
            client.copy(&source, &destination, &info.etag).await?;
        }
        Command::Sync {
            connection,
            local,
            prefix,
            mode,
            apply_additions,
        } => {
            let preview = app
                .sync_preview(&connection, local.clone(), &prefix, mode.into())
                .await?;
            println!("{}", serde_json::to_string_pretty(&preview)?);
            if apply_additions {
                let ids = app
                    .sync_additions(&connection, local, &prefix, mode.into())
                    .await?;
                wait(&app, &ids).await?;
            }
        }
        Command::Queue { command } => match command {
            Queue::List => println!("{}", serde_json::to_string_pretty(&app.engine.list()?)?),
            Queue::Resume { id } => {
                let job = app
                    .engine
                    .list()?
                    .into_iter()
                    .find(|j| j.id == id)
                    .context("Transfer not found")?;
                app.engine.resume(app.client(&job.profile_id).await?, &id)?;
                wait(&app, &[id]).await?;
            }
            Queue::Cancel { id } => {
                let job = app
                    .engine
                    .list()?
                    .into_iter()
                    .find(|j| j.id == id)
                    .context("Transfer not found")?;
                app.engine.cancel(app.client(&job.profile_id).await?, &id)?;
                wait(&app, &[id]).await?;
            }
            Queue::Clear => app.engine.clear_finished()?,
        },
    }
    Ok(())
}
async fn wait(app: &App, ids: &[String]) -> Result<()> {
    loop {
        let jobs = app.engine.list()?;
        let selected: Vec<_> = jobs.iter().filter(|j| ids.contains(&j.id)).collect();
        if selected.iter().all(|j| {
            matches!(
                j.status,
                Status::Completed | Status::Cancelled | Status::Failed | Status::Paused
            )
        }) {
            for job in &selected {
                eprintln!(
                    "{}: {:?} {}/{}",
                    job.key, job.status, job.bytes_done, job.size
                );
            }
            if selected.iter().any(|j| j.status == Status::Failed) {
                bail!(
                    "{}",
                    selected
                        .iter()
                        .filter_map(|j| j.error.clone())
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
            return Ok(());
        }
        tokio::select! { _=tokio::signal::ctrl_c()=> { for id in ids { if let Err(e)=app.engine.pause(id) { eprintln!("Pause: {e}"); } } }, _=tokio::time::sleep(std::time::Duration::from_millis(250))=>{} }
    }
}
