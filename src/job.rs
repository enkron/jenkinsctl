use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Serialize)]
pub struct JobInfo {
    pub jobs: Vec<Jobs>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Jobs {
    #[serde(rename = "_class")]
    pub class: String,
    pub name: String,
}
