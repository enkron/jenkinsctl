#![warn(clippy::all, clippy::pedantic)]
use async_recursion::async_recursion;
use clap::{Parser, Subcommand};
use log;
use pretty_env_logger;
use std::io::Write;

mod jenkins;
mod job;
mod node;
use crate::jenkins::{Jenkins, Result, Tree};
use crate::job::{BuildInfo, JobInfo};
use crate::node::NodeInfo;

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
    commands: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Set 'prepare to shutdown' bunner with optional reason")]
    Shutdown {
        #[command(subcommand)]
        shutdown_commands: ShutdownState,
    },
    #[command(about = "Restart Jenkins instance")]
    Restart {
        #[arg(long, help = "Reset Jenkins without waiting jobs completion")]
        hard: bool,
    },
    #[command(about = "Copy job from the existing one")]
    Copy {
        #[command(subcommand)]
        copy_commands: CopyItem,
    },
    #[command(about = "Node actions")]
    #[command(arg_required_else_help(true))]
    Node {
        #[command(subcommand)]
        node_commands: NodeAction,
    },
    #[command(about = "Node actions")]
    #[command(arg_required_else_help(true))]
    Job {
        #[command(subcommand)]
        job_commands: JobAction,
    },
    #[command(about = "Display system-wide information")]
    Info,
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
        show_commands: ShowAction,
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
    List {
        #[arg(
            index = 1,
            help = "List the builds for specific job",
            required = false,
            default_value = "",
            hide_default_value = true
        )]
        job: String,
    },
    #[command(
        aliases = ["b"],
        about = "Build a job (use '-' as param list to build with defaults)"
    )]
    Build {
        #[arg(index = 1, help = "Job path (format: path/to/jenkins/job)")]
        job: String,
        #[arg(
            index = 2,
            help = "List of parameters (format: param=value,...,param=value)",
            required = false,
            default_value = "",
            hide_default_value = true
        )]
        params: String,
        #[arg(short, long, help = "Follow the console output")]
        follow: bool,
    },
    #[command(
        aliases = ["rm", "delete", "del"],
        about = "Remove a job (use with caution, the action is permanent)"
    )]
    Remove {
        #[arg(index = 1, help = "Job path (format: path/to/jenkins/job)")]
        job: String,
    },
    #[command(
        aliases = ["fetch"],
        about = "Download an item from a particular build(s)"
    )]
    Download {
        #[command(subcommand)]
        item: BuildItem,
    },
    #[command(about = "Interrupt a build execution")]
    Kill {
        #[arg(
            short,
            long,
            help = "Send a signal to the job process (HUP, TERM, KILL)",
            default_value = "TERM"
        )]
        signal: String,
        #[arg(index = 1, help = "Job path (format: path/to/jenkins/job)")]
        job: String,
        #[arg(
            index = 2,
            help = "Build number or build range (range is not implemented yet)"
        )]
        build: String,
    },
}

#[derive(Subcommand)]
enum BuildItem {
    #[command(about = "Download build artifacts if any")]
    Artifact {
        #[arg(index = 1, help = "Job path (format: path/to/jenkins/job)")]
        job: String,
        #[arg(
            index = 2,
            help = "Build number or build range (range is not implemented yet)"
        )]
        build: String,
    },
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

        query.insert_str(0, &inner_job);
        let tree = Tree::new(query);
        let json_data = jenkins.get_json_data(&tree).await?;

        let nested_job_info = jenkins
            .system::<JobInfo>(json_data.get_ref().as_slice())
            .await?;
        for job in nested_job_info.jobs {
            let mut job_path = std::path::Path::new(job.full_name.as_str())
                .iter()
                .map(|e| e.to_str().unwrap())
                .collect::<Vec<_>>();
            job_path.pop().unwrap();
            let class = job.class.rsplit_once('.').unwrap().1.to_lowercase();

            for e in &job_path {
                if class != "folder" {
                    print!("\x1b[94;1m{e}\x1b[0m => ");
                } else {
                    continue;
                }
            }

            rec_walk(&class, &jenkins, job.name.as_str(), inner_job.clone()).await?;
        }
    } else {
        println!("{job_name}");
    }

    Ok(())
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
            "missing argument(s): url={}, user={}, token={}",
            !url.is_empty(),
            !user.is_empty(),
            !token.is_empty()
        );
        std::process::exit(1);
    }

    let jenkins = Jenkins::new(&user, &token, &url);

    match args.commands {
        Commands::Shutdown { shutdown_commands } => match shutdown_commands {
            ShutdownState::On { reason } => {
                jenkins.shutdown(ShutdownState::On { reason }).await?;
            }
            ShutdownState::Off => {
                jenkins.shutdown(ShutdownState::Off).await?;
            }
        },
        Commands::Restart { hard } => {
            jenkins.restart(hard).await?;
        }
        Commands::Copy { copy_commands } => match copy_commands {
            CopyItem::Job { from, to } => {
                jenkins.copy(CopyItem::Job { from, to }).await?;
            }
            CopyItem::View { from, to } => {
                jenkins.copy(CopyItem::View { from, to }).await?;
            }
        },
        Commands::Node { node_commands } => match node_commands {
            NodeAction::Show { show_commands } => match show_commands {
                ShowAction::Raw => {
                    let tree = Tree::new("computer/api/json".to_string());
                    let json_data = jenkins.get_json_data(&tree).await?;
                    let node_info = jenkins
                        .system::<NodeInfo>(json_data.get_ref().as_slice())
                        .await?;
                    println!("{:#?}", node_info);
                }
                ShowAction::Executors { total, busy } => {
                    let tree = Tree::new("computer/api/json".to_string());
                    let json_data = jenkins.get_json_data(&tree).await?;

                    let node_info = jenkins
                        .system::<NodeInfo>(json_data.get_ref().as_slice())
                        .await?;

                    if total && !busy {
                        println!("Total number of executors: {}", node_info.total_executors);
                    }

                    if busy && !total {
                        println!("Busy executors: {}", node_info.busy_executors);
                    }

                    if !total && !busy {
                        println!("Total number of executors: {}", node_info.total_executors);
                        println!("Busy executors: {}", node_info.busy_executors);
                    }
                }
            },
            NodeAction::List { status } => {
                let tree = Tree::new("computer/api/json".to_string());
                let json_data = jenkins.get_json_data(&tree).await?;

                let node_info = jenkins
                    .system::<NodeInfo>(json_data.get_ref().as_slice())
                    .await?;

                if status {
                    for node in node_info.computer {
                        if node.offline {
                            println!("{:.<40}\x1b[31moffline\x1b[0m", node.display_name);
                        } else {
                            println!("{:.<40}\x1b[32monline\x1b[0m", node.display_name);
                        }
                    }
                } else {
                    for node in node_info.computer {
                        println!("{}", node.display_name);
                    }
                }
            }
        },
        Commands::Job { job_commands } => match job_commands {
            JobAction::List { job } => {
                if job.is_empty() {
                    let tree =
                        Tree::new("api/json?tree=jobs[fullDisplayName,fullName,name]".to_string());
                    let json_data = jenkins.get_json_data(&tree).await?;

                    let job_info = jenkins
                        .system::<JobInfo>(json_data.get_ref().as_slice())
                        .await?;

                    for job in job_info.jobs {
                        let class = job.class.rsplit_once('.').unwrap().1.to_lowercase();
                        let inner_job = "".to_string();

                        rec_walk(&class, &jenkins, job.full_name.as_str(), inner_job).await?;
                    }
                } else {
                    let tree =
                        Tree::new("api/json?tree=builds[number,url],nextBuildNumber".to_string())
                            .build_path(&job);

                    let json_data = jenkins.get_json_data(&tree).await?;
                    let build_info = jenkins
                        .system::<BuildInfo>(json_data.get_ref().as_slice())
                        .await?;

                    for build in build_info.builds {
                        println!("{}", build.number);
                    }
                }
            }
            JobAction::Build {
                job,
                params,
                follow,
            } => {
                let tree =
                    Tree::new("api/json?tree=builds[number,url],nextBuildNumber".to_string())
                        .build_path(&job);

                let json_data = jenkins.get_json_data(&tree).await?;
                let build_info = jenkins
                    .system::<BuildInfo>(json_data.get_ref().as_slice())
                    .await?;

                log::info!("started build {}", build_info.next_build_number);

                jenkins.build(&job, params).await?;

                if follow {
                    let mut offset: usize = 0;
                    loop {
                        let tree = Tree::new(format!(
                            "{}/logText/progressiveText?start={offset}",
                            build_info.next_build_number
                        ))
                        .build_path(&job);

                        match jenkins.get_console_log(&tree).await {
                            Some((data, current_offset)) => {
                                if !data.get_ref().is_empty() {
                                    print!("{}", String::from_utf8_lossy(data.get_ref()));
                                    offset = current_offset;
                                }
                            }
                            None => {
                                break;
                            }
                        }
                    }
                }
            }
            JobAction::Remove { job } => {
                jenkins.remove(&job).await?;
            }
            JobAction::Download { item } => match item {
                BuildItem::Artifact { job, build } => {
                    let tree =
                        Tree::new(format!("{build}/artifact/*zip*/archive.zip")).build_path(&job);

                    match jenkins.get_json_data(&tree).await {
                        Ok(data) => {
                            log::info!("fetching build {build} artifacts from the {job}");
                            let job_base = std::path::Path::new(&job)
                                .file_name()
                                .unwrap()
                                .to_str()
                                .unwrap();

                            let mut file =
                                std::fs::File::create(format!("{job_base}_{build}.zip"))?;
                            file.write_all(data.get_ref())?;
                        }
                        Err(e) => log::error!(
                            "\x1b[30;1m{e}\x1b[m: artifacts not found for the build {build}"
                        ),
                    }
                }
            },
            JobAction::Kill { signal, job, build } => {
                let tree = Tree::new(format!("{build}")).build_path(&job);
                if let Err(e) = jenkins.kill(&tree, signal).await {
                    log::error!("{e}");
                }
            }
        },
        Commands::Info => println!("{url}"),
    }

    Ok(())
}
