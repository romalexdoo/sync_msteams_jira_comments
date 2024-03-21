use crate::server::cfg::Config as ServerConfig;
use crate::ms_graph_api::cfg::Config as MsGraphApiConfig;
use crate::jira_api::cfg::Config as JiraConfig;
use envconfig::Envconfig;

/// Generic configuration for any module.
/// Configuration of particular modules is stored in DBMS and managed by
/// configuration module.
#[derive(Envconfig, Clone)]
pub struct Config {
    #[envconfig(nested = true)]
    pub server: ServerConfig,
    #[envconfig(nested = true)]
    pub ms_graph_api: MsGraphApiConfig,
    #[envconfig(nested = true)]
    pub jira: JiraConfig,
}
