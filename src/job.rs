#![allow(clippy::struct_excessive_bools)]
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Serialize)]
pub struct Info {
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

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    #[serde(rename = "_class")]
    class: String,
    pub builds: Vec<Build>,
    pub next_build_number: u32,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Build {
    #[serde(rename = "_class")]
    class: String,
    pub number: u32,
    url: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct ActionObj {
    #[serde(rename = "_class")]
    class: String,
    pub actions: serde_json::value::Value,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct BuildParams {
    #[serde(rename = "_class")]
    class: String,
    pub actions: Vec<Params>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct Params {
    #[serde(rename = "_class")]
    class: String,
    pub parameters: Vec<ParamsAction>,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct ParamsAction {
    #[serde(rename = "_class")]
    class: String,
    pub name: String,
    pub value: serde_json::value::Value,
}
