use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Serialize)]
pub struct JobInfo {
    pub jobs: Vec<Jobs>,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Jobs {
    #[serde(rename = "_class")]
    pub class: String,
    pub full_display_name: String,
    pub full_name: String,
    pub name: String,
}
