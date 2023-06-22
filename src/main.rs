#![warn(clippy::all, clippy::pedantic)]
use async_recursion::async_recursion;
use clap::{Parser, Subcommand};
use log;
use pretty_env_logger;

mod jenkins;
mod job;
mod node;
use crate::jenkins::{Jenkins, Result, Tree};

const JENKINS_URL: &str = "JENKINS_URL";
const JENKINS_USER: &str = "JENKINS_USER";
const JENKINS_TOKEN: &str = "JENKINS_TOKEN";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, required = false, default_value = "", hide_default_value = true)]
    url: String,
    #[arg(
        short,
        long,
        required = false,
        default_value = "",
        hide_default_value = true
    )]
    user: String,
    #[arg(
        short,
        long,
        required = false,
        default_value = "",
        hide_default_value = true
    )]
    token: String,
    #[command(subcommand)]
    commands: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Set 'prepare to shutdown' bunner with optional reason")]
    Shutdown {
        #[command(subcommand)]
        shutdown_commands: Option<ShutdownState>,
    },
    #[command(about = "Restart Jenkins instance")]
    Restart {
        #[arg(long, help = "Reset Jenkins without waiting jobs completion")]
        hard: bool,
    },
    #[command(about = "Copy job from the existing one")]
    Copy {
        #[command(subcommand)]
        copy_commands: Option<CopyItem>,
    },
    #[command(about = "Node actions")]
    #[command(arg_required_else_help(true))]
    Node {
        #[command(subcommand)]
        node_commands: Option<NodeAction>,
    },
    #[command(about = "Node actions")]
    #[command(arg_required_else_help(true))]
    Job {
        #[command(subcommand)]
        job_commands: Option<JobAction>,
    },
}

#[derive(Subcommand)]
pub enum ShutdownState {
    #[command(about = "Set shutdown banner")]
    On {
        #[arg(
            index = 1,
            help = "Optional reason",
            required = false,
            default_value = "",
            hide_default_value = true
        )]
        reason: String,
    },
    #[command(about = "Cancel shutdown")]
    Off,
}

#[derive(Subcommand)]
pub enum CopyItem {
    #[command(about = "Copy job")]
    Job {
        #[arg(index = 1, help = "Job copy from")]
        from: String,
        #[arg(index = 2, help = "Target job")]
        to: String,
    },
    #[command(about = "Copy view")]
    View {
        #[arg(index = 1, help = "View copy from")]
        from: String,
        #[arg(index = 2, help = "Target view")]
        to: String,
    },
}

#[derive(Subcommand)]
enum NodeAction {
    #[command(about = "Show node information")]
    Show {
        #[command(subcommand)]
        show_commands: Option<ShowAction>,
    },
    #[command(aliases = ["ls"], about = "List all nodes")]
    List {
        #[arg(long, help = "Show node offline")]
        status: bool,
    },
}

#[derive(Subcommand)]
enum ShowAction {
    #[command(about = "Show all nodes information")]
    Raw,
    #[command(about = "Show executors info")]
    Executors {
        #[arg(long, help = "Total number of executors")]
        total: bool,
        #[arg(long, help = "Busy executors")]
        busy: bool,
    },
}

#[derive(Subcommand)]
enum JobAction {
    #[command(aliases = ["ls"], about = "List all jobs")]
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    let args = Args::parse();
    let url = std::env::var(JENKINS_URL);
    let user = std::env::var(JENKINS_USER);
    let token = std::env::var(JENKINS_TOKEN);

    let url = if let Ok(v) = url { v } else { args.url };
    let user = if let Ok(v) = user { v } else { args.user };
    let token = if let Ok(v) = token { v } else { args.token };

    if url.is_empty() || user.is_empty() || token.is_empty() {
        log::error!(
            "missing argument: url={}, user={}, token={}",
            url,
            user,
            token
        );
        std::process::exit(1);
    }

    let jenkins = Jenkins::new(user.as_str(), token.as_str(), url.as_str());

    match args.commands {
        Some(Commands::Shutdown { shutdown_commands }) => match shutdown_commands {
            Some(ShutdownState::On { reason }) => {
                jenkins.shutdown(ShutdownState::On { reason }).await?;
            }
            Some(ShutdownState::Off) => {
                jenkins.shutdown(ShutdownState::Off).await?;
            }
            None => todo!(),
        },
        Some(Commands::Restart { hard }) => {
            jenkins.restart(hard).await?;
        }
        Some(Commands::Copy { copy_commands }) => match copy_commands {
            Some(CopyItem::Job { from, to }) => {
                jenkins.copy(CopyItem::Job { from, to }).await?;
            }
            Some(CopyItem::View { from, to }) => {
                jenkins.copy(CopyItem::View { from, to }).await?;
            }
            None => todo!(),
        },
        Some(Commands::Node { node_commands }) => match node_commands {
            Some(NodeAction::Show { show_commands }) => match show_commands {
                Some(ShowAction::Raw) => {
                    let node_info = jenkins.node().await?;
                    println!("{:#?}", node_info);
                }
                Some(ShowAction::Executors { total, busy }) => {
                    if total && !busy {
                        let node_info = jenkins.node().await?;
                        println!("Total number of executors: {}", node_info.total_executors);
                    }

                    if busy && !total {
                        let node_info = jenkins.node().await?;
                        println!("Busy executors: {}", node_info.busy_executors);
                    }

                    if !total && !busy {
                        let node_info = jenkins.node().await?;
                        println!("Total number of executors: {}", node_info.total_executors);
                        println!("Busy executors: {}", node_info.busy_executors);
                    }
                }
                None => todo!(),
            },
            Some(NodeAction::List { status }) => {
                if status {
                    let node_info = jenkins.node().await?;
                    for node in node_info.computer {
                        if node.offline {
                            println!("{:.<40}\x1b[31moffline\x1b[0m", node.display_name);
                        } else {
                            println!("{:.<40}\x1b[32monline\x1b[0m", node.display_name);
                        }
                    }
                } else {
                    let node_info = jenkins.node().await?;
                    for node in node_info.computer {
                        println!("{}", node.display_name);
                    }
                }
            }
            None => todo!(),
        },
        Some(Commands::Job { job_commands }) => match job_commands {
            Some(JobAction::List) => {
                let tree = Tree::new("api/json?tree=jobs[fullDisplayName,fullName,name]");
                let job_info = jenkins.job(tree).await?;
                for job in job_info.jobs {
                    let class = job.class.rsplit_once('.').unwrap().1.to_lowercase();
                    let inner_job = "".to_string();

                    rec_walk(class.as_str(), &jenkins, job.full_name.as_str(), inner_job).await?;
                }
            }
            None => todo!(),
        },
        None => todo!(),
    }

    Ok(())
}

#[async_recursion]
async fn rec_walk<'t>(
    class: &str,
    jenkins: &Jenkins<'t>,
    job_name: &str,
    mut inner_job: String,
) -> Result<()> {
    if class == "folder" {
        inner_job.push_str(format!("/job/{}", job_name).as_str());
        let mut query = "/api/json?tree=jobs[fullDisplayName,fullName,name]".to_string();

        query.insert_str(0, inner_job.as_str());
        let tree = Tree::new(query.as_str());

        let nested_job_info = jenkins.job(tree).await?;
        for job in nested_job_info.jobs {
            let mut job_path = std::path::Path::new(job.full_name.as_str())
                .iter()
                .map(|e| e.to_str().unwrap())
                .collect::<Vec<_>>();
            job_path.pop().unwrap();
            let class = job.class.rsplit_once('.').unwrap().1.to_lowercase();

            for e in &job_path {
                if job_path.len() > 1 {
                    print!("\x1b[94;1m{e}\x1b[0m => ");
                } else {
                    let root_job_path = std::path::Path::new(job.full_name.as_str());
                    let root_class = job.class.rsplit_once('.').unwrap().1.to_lowercase();
                    if root_class == "folder" {
                        println!(
                            "\x1b[94;1m{}\x1b[0m",
                            root_job_path.parent().unwrap().display()
                        );
                    } else {
                        print!(
                            "\x1b[94;1m{}\x1b[0m => ",
                            root_job_path.parent().unwrap().display()
                        );
                    }
                }
            }

            rec_walk(
                class.as_str(),
                &jenkins,
                job.name.as_str(),
                inner_job.clone(),
            )
            .await?;
        }
    } else {
        println!("{job_name}");
    }

    Ok(())
}
