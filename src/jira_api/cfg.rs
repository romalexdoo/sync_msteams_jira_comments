use envconfig::Envconfig;

#[derive(Envconfig, Clone)]
pub struct Config {
    #[envconfig(from = "JIRA_CLIENT_SECRET", default = "")]
    pub client_secret: String,
    #[envconfig(from = "JIRA_NOTIFICATION_URL", default = "")]
    pub notification_url: String,
    #[envconfig(from = "JIRA_USER", default = "")]
    pub user: String,
    #[envconfig(from = "JIRA_SECRET", default = "")]
    pub secret: String,
    #[envconfig(from = "JIRA_TOKEN", default = "")]
    pub token: String,
    #[envconfig(from = "JIRA_BASE_URL", default = "")]
    pub base_url: String,
    #[envconfig(from = "JIRA_PROJECT_KEY", default = "")]
    pub project_key: String,    
    #[envconfig(from = "JIRA_MSTEAMS_LINK_FIELD_NAME", default = "")]
    pub msteams_link_field_name: String,    
    #[envconfig(from = "JIRA_MSTEAMS_LINK_FIELD_JQL_NAME", default = "")]
    pub msteams_link_field_jql_name: String,    
}
