#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::too_many_lines)]
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::{io::Write, str::FromStr};

use crate::{
    jenkins::{Jenkins, Tree},
    job::{self, BuildInfo},
    node, rec_walk, Result,
};

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
        state: ShutdownState,
    },
    #[command(about = "Restart Jenkins instance")]
    Restart {
        #[arg(long, help = "Reset Jenkins without waiting jobs completion")]
        hard: bool,
    },
    #[command(about = "Copy item (job/view)")]
    Copy {
        #[command(subcommand)]
        item: CopyItem,
        #[arg(index = 1, help = "Source", global = true, required = false)]
        src: String,
        #[arg(index = 2, help = "Destination", global = true, required = false)]
        dest: String,
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
    #[command(about = "Make a copy of specific job")]
    Job,
    #[command(about = "Make a copy of specific view")]
    View,
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
        #[arg(short, long, help = "Show node offline")]
        status: bool,
    },
    #[command(about = "Switch node state")]
    Set {
        #[arg(index = 1, help = "Node name")]
        node: String,
        #[command(subcommand)]
        state: NodeState,
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
        #[arg(
            index = 1,
            help = "Job path (format: path/to/jenkins/job)",
            global = true,
            required = false
        )]
        job: String,
        #[arg(
            index = 2,
            help = "Build number or build range (range is not implemented yet)",
            global = true,
            required = false
        )]
        build: String,
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
    #[command(about = "Rebuild specified job")]
    Rebuild {
        #[arg(index = 1, help = "Job path (format: path/to/jenkins/job)")]
        job: String,
        #[arg(index = 2, help = "Build number")]
        build: String,
    },
}

#[derive(Subcommand)]
enum BuildItem {
    #[command(about = "Download build artifacts if any")]
    Artifact,
    #[command(about = "Fetch build log")]
    Log,
}

#[derive(Subcommand)]
pub enum NodeState {
    #[command(about = "Disconnect a node")]
    Disconnect {
        #[arg(
            index = 1,
            help = "Optional reason",
            required = false,
            default_value = "",
            hide_default_value = true
        )]
        reason: String,
    },
    #[command(about = "Connect a node")]
    Connect,
    #[command(about = "Set a node offline")]
    Offline {
        #[arg(
            index = 1,
            help = "Optional reason",
            required = false,
            default_value = "",
            hide_default_value = true
        )]
        reason: String,
    },
    #[command(about = "Set a node online")]
    Online,
}

enum BuildParam {
    Range(u64, u64),
    Once(u64),
}

impl FromStr for BuildParam {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.contains("..") & !s.contains("..=") {
            let start = s.split_once('.').unwrap().0.parse::<u64>()?;
            let end = s.rsplit_once('.').unwrap().1.parse::<u64>()?;

            return Ok(Self::Range(start, end));
        } else if s.contains("..=") {
            let start = s.split_once('.').unwrap().0.parse::<u64>()?;
            let end = s.rsplit_once('=').unwrap().1.parse::<u64>()?;

            return Ok(Self::Range(start, end + 1));
        }
        let num = s.parse::<u64>()?;
        Ok(Self::Once(num))
    }
}

pub async fn handle() -> Result<()> {
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
        Commands::Shutdown { state } => {
            jenkins.shutdown(state).await?;
        }
        Commands::Restart { hard } => {
            jenkins.restart(hard).await?;
        }
        Commands::Copy { item, src, dest } => {
            if let Err(e) = jenkins.copy(item, src, dest).await {
                log::error!(
                    "copy {} a directory is not enabled -> {e}",
                    "to".red().bold()
                );
            }
        }
        Commands::Node { node_commands } => match node_commands {
            NodeAction::Show { show_commands } => match show_commands {
                ShowAction::Raw => {
                    let tree = Tree::new("computer/api/json".to_string());
                    let json_data = jenkins.get_json_data(&tree).await?;
                    let node_info = Jenkins::system::<node::Info>(json_data.get_ref().as_slice())?;
                    println!("{node_info:#?}");
                }
                ShowAction::Executors { total, busy } => {
                    let tree = Tree::new("computer/api/json".to_string());
                    let json_data = jenkins.get_json_data(&tree).await?;

                    let node_info = Jenkins::system::<node::Info>(json_data.get_ref().as_slice())?;

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

                let node_info = Jenkins::system::<node::Info>(json_data.get_ref().as_slice())?;

                if status {
                    for node in node_info.computer {
                        if node.offline {
                            println!("{:.<40}{}", node.display_name, "offline".red());
                        } else {
                            println!("{:.<40}{}", node.display_name, "online".green());
                        }
                    }
                } else {
                    for node in node_info.computer {
                        println!("{}", node.display_name);
                    }
                }
            }
            NodeAction::Set { node, state } => {
                let tree = Tree::new(format!("computer/{node}"));
                jenkins.set(&tree, state).await?;
            }
        },
        Commands::Job { job_commands } => match job_commands {
            JobAction::List { job } => {
                if job.is_empty() {
                    let tree =
                        Tree::new("api/json?tree=jobs[fullDisplayName,fullName,name]".to_string());
                    let json_data = jenkins.get_json_data(&tree).await?;

                    let job_info = Jenkins::system::<job::Info>(json_data.get_ref().as_slice())?;

                    for job in job_info.jobs {
                        let class = job.class.rsplit_once('.').unwrap().1.to_lowercase();
                        let inner_job = String::new();

                        rec_walk(&class, &jenkins, job.full_name.as_str(), inner_job).await?;
                    }
                } else {
                    let tree =
                        Tree::new("api/json?tree=builds[number,url],nextBuildNumber".to_string())
                            .build_path(&job);

                    let json_data = jenkins.get_json_data(&tree).await?;
                    let build_info = Jenkins::system::<BuildInfo>(json_data.get_ref().as_slice())?;

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
                let build_info = Jenkins::system::<BuildInfo>(json_data.get_ref().as_slice())?;

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
            JobAction::Download { item, job, build } => match item {
                BuildItem::Artifact => {
                    let build_param = build.parse::<BuildParam>()?;
                    match build_param {
                        BuildParam::Range(start, end) => {
                            for build in start..end {
                                let tree = Tree::new(format!("{build}/artifact/*zip*/archive.zip"))
                                    .build_path(&job);

                                match jenkins.get_json_data(&tree).await {
                                    Ok(data) => {
                                        log::info!(
                                            "fetching build {build} artifacts from the {job}"
                                        );
                                        let job_base = std::path::Path::new(&job)
                                            .file_name()
                                            .unwrap()
                                            .to_str()
                                            .unwrap();

                                        let mut file = std::fs::File::create(format!(
                                            "{job_base}_{build}.zip"
                                        ))?;
                                        file.write_all(data.get_ref())?;
                                    }
                                    Err(e) => log::error!(
                                        "{}: artifacts not found for the build {build}",
                                        e.to_string().red().bold()
                                    ),
                                }
                            }
                        }
                        BuildParam::Once(n) => {
                            let tree = Tree::new(format!("{n}/artifact/*zip*/archive.zip"))
                                .build_path(&job);

                            match jenkins.get_json_data(&tree).await {
                                Ok(data) => {
                                    log::info!("fetching build {n} artifacts from the {job}");
                                    let job_base = std::path::Path::new(&job)
                                        .file_name()
                                        .unwrap()
                                        .to_str()
                                        .unwrap();

                                    let mut file =
                                        std::fs::File::create(format!("{job_base}_{n}.zip"))?;
                                    file.write_all(data.get_ref())?;
                                }
                                Err(e) => log::error!(
                                    "{}: artifacts not found for the build {build}",
                                    e.to_string().red().bold()
                                ),
                            }
                        }
                    }
                }
                BuildItem::Log => {
                    let tree = Tree::new(format!("{build}/consoleText")).build_path(&job);
                    let data = jenkins.get_json_data(&tree).await?;
                    let log = String::from_utf8(data.into_inner())?;
                    print!("{log}");
                }
            },
            JobAction::Kill { signal, job, build } => {
                let tree = Tree::new(build).build_path(&job);
                if let Err(e) = jenkins.kill(&tree, signal).await {
                    log::error!("{e}");
                }
            }
            JobAction::Rebuild { job, build } => {
                let tree = Tree::new(format!("{build}/api/json?tree=actions")).build_path(&job);

                let json_data = jenkins.get_json_data(&tree).await?;
                let action_obj = Jenkins::system::<job::ActionObj>(json_data.get_ref().as_slice())?;

                let actions_classes = action_obj.actions.as_array().unwrap();

                let mut param_actions_truth = std::collections::HashMap::new();
                for (idx, class) in actions_classes.iter().enumerate() {
                    param_actions_truth.insert(class.to_string().contains("ParametersAction"), idx);
                }

                let tree = Tree::new(format!(
                    "{build}/api/json?tree=actions[parameters[name,value]]{{{}}}",
                    param_actions_truth.get(&true).unwrap()
                ))
                .build_path(&job);

                let json_data = jenkins.get_json_data(&tree).await?;
                let build_params =
                    Jenkins::system::<job::BuildParams>(json_data.get_ref().as_slice())?;

                log::info!("rebuilding the build {build} with params:");

                let mut params = String::new();
                for params_action in build_params.actions {
                    for parameters in params_action.parameters {
                        params.push_str(
                            format!("&{}={}", parameters.name, parameters.value).as_str(),
                        );
                        log::info!("{:-<40}{}", parameters.name, parameters.value);
                    }
                }

                jenkins.rebuild(&job, params).await?;
            }
        },
        Commands::Info => println!("{url}"),
    }

    Ok(())
}
