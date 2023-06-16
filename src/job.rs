use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Serialize)]
pub struct JobInfo {
    pub jobs: Vec<Jobs>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Jobs {
    pub name: String,
}
