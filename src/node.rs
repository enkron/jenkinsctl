use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodesInfo {
    pub busy_executors: u32,
    pub computer: Vec<Computer>,
    display_name: String,
    pub total_executors: u16,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Computer {
    //action: Option<Action>,
    assigned_labels: Vec<AssignedLabels>,
    description: Option<String>,
    pub display_name: String,
    //executors: Vec<String>,
    icon: String,
    icon_class_name: String,
    idle: bool,
    jnlp_agent: bool,
    launch_supported: bool,
    manual_launch_allowed: bool,
    monitor_data: MonitorData,
    num_executors: u32,
    pub offline: bool,
    //offline_cause: Option<String>,
    offline_cause_reason: String,
    temporarily_offline: bool,
    //one_off_executors: Vec<String>,
    absolute_remote_path: Option<String>,
}

impl std::fmt::Display for Computer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

#[derive(Deserialize, Debug, Serialize)]
struct Action;

#[derive(Deserialize, Debug, Serialize)]
struct AssignedLabels {
    name: String,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorData {
    #[serde(rename = "hudson.node_monitors.SwapSpaceMonitor")]
    swap_space_monitor: Option<SwapSpaceMonitor>,
    #[serde(rename = "hudson.node_monitors.TemporarySpaceMonitor")]
    temporary_space_monitor: Option<TemporarySpaceMonitor>,
    #[serde(rename = "hudson.node_monitors.DiskSpaceMonitor")]
    disk_space_monitor: Option<DiskSpaceMonitor>,
    #[serde(rename = "hudson.node_monitors.ArchitectureMonitor")]
    architecture_monitor: Option<String>,
    #[serde(rename = "hudson.node_monitors.ResponseTimeMonitor")]
    response_time_monitor: Option<ResponseTimeMonitor>,
    #[serde(rename = "hudson.plugins.systemloadaverage_monitor.SystemLoadAverageMonitor")]
    system_load_average_monitor: Option<String>,
    #[serde(rename = "hudson.node_monitors.ClockMonitor")]
    clock_monitor: Option<ClockMonitor>,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SwapSpaceMonitor {
    available_physical_memory: u64,
    available_swap_space: u64,
    total_physical_memory: u64,
    total_swap_space: u64,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TemporarySpaceMonitor {
    timestamp: u64,
    path: String,
    size: u64,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiskSpaceMonitor {
    timestamp: u64,
    path: String,
    size: u64,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResponseTimeMonitor {
    timestamp: u64,
    average: u32,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClockMonitor {
    diff: i64,
}
