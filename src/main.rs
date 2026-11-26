//! CLI for sigmaos-iloveapi.

use clap::{Parser, Subcommand, ValueEnum};
use sigmaos_iloveapi::{
    default_output_dir, default_store_path, CopyProvider, JobManager, JobStatus,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "sigmaos-iloveapi",
    about = "SigmaOS file conversion job manager"
)]
struct Cli {
    #[arg(long, global = true)]
    store: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, Copy, ValueEnum)]
enum StatusArg {
    Queued,
    Processing,
    Done,
    Failed,
}

impl From<StatusArg> for JobStatus {
    fn from(s: StatusArg) -> Self {
        match s {
            StatusArg::Queued => JobStatus::Queued,
            StatusArg::Processing => JobStatus::Processing,
            StatusArg::Done => JobStatus::Done,
            StatusArg::Failed => JobStatus::Failed,
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Create a conversion job.
    Create {
        source: PathBuf,
        /// Target format (e.g. pdf, png, docx).
        #[arg(long)]
        to: String,
        /// Provider name (only "copy" mock supported locally).
        #[arg(long, default_value = "copy")]
        provider: String,
    },
    /// Run a queued job.
    Run { id: String },
    /// List jobs.
    List {
        #[arg(long, value_enum)]
        status: Option<StatusArg>,
    },
    /// Show a job.
    Show { id: String },
    /// Remove a job.
    Remove { id: String },
    /// Print store path.
    Path,
}

fn store_path(s: &Option<PathBuf>) -> Result<PathBuf, Box<dyn std::error::Error>> {
    s.clone()
        .or_else(default_store_path)
        .ok_or_else(|| "cannot determine store path".into())
}

fn load(s: &Option<PathBuf>) -> Result<JobManager, Box<dyn std::error::Error>> {
    let p = store_path(s)?;
    if p.exists() {
        Ok(JobManager::load(&p)?)
    } else {
        let m = JobManager::default();
        m.save(&p)?;
        Ok(m)
    }
}

fn save(m: &JobManager, path: &Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    m.save(&store_path(path)?)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let cli = Cli::parse();
    let mut mgr = load(&cli.store)?;
    match cli.command {
        Commands::Create {
            source,
            to,
            provider,
        } => {
            let job = mgr.create(source, &to, &provider)?;
            save(&mgr, &cli.store)?;
            println!(
                "created job {} ({} -> {})",
                job.id,
                job.source.display(),
                to
            );
        }
        Commands::Run { id } => {
            let out_dir = default_output_dir().unwrap_or_else(|| PathBuf::from("./converted"));
            mgr.run(&id, &CopyProvider, &out_dir)?;
            save(&mgr, &cli.store)?;
            let job = mgr.get(&id).unwrap();
            println!("job {} -> {:?} {:?}", id, job.status, job.output);
        }
        Commands::List { status } => {
            for j in mgr.list(status.map(Into::into)) {
                println!(
                    "{} [{:?}] {} -> {} ({})",
                    j.id,
                    j.status,
                    j.source.display(),
                    j.target_format,
                    j.provider
                );
            }
        }
        Commands::Show { id } => match mgr.get(&id) {
            Some(j) => println!("{}", serde_json::to_string_pretty(j)?),
            None => return Err(format!("job not found: {id}").into()),
        },
        Commands::Remove { id } => {
            mgr.remove(&id)?;
            save(&mgr, &cli.store)?;
            println!("removed {id}");
        }
        Commands::Path => println!("{}", store_path(&cli.store)?.display()),
    }
    Ok(())
}
