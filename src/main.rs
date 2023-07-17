#![warn(clippy::all, clippy::pedantic)]
use async_recursion::async_recursion;
use colored::Colorize;

mod args;
mod jenkins;
mod job;
mod node;
use crate::jenkins::{Jenkins, Tree};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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

        let nested_job_info = Jenkins::system::<job::Info>(json_data.get_ref().as_slice())?;
        for job in nested_job_info.jobs {
            let mut job_path = std::path::Path::new(job.full_name.as_str())
                .iter()
                .map(|e| e.to_str().unwrap())
                .collect::<Vec<_>>();
            job_path.pop().unwrap();
            let class = job.class.rsplit_once('.').unwrap().1.to_lowercase();

            for e in job_path {
                if class == "folder" {
                    continue;
                }
                print!("{} => ", e.blue().bold());
            }

            rec_walk(&class, jenkins, job.name.as_str(), inner_job.clone()).await?;
        }
    } else {
        println!("{job_name}");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();

    args::handle().await
}
