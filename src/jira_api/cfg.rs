use envconfig::Envconfig;

#[derive(Envconfig, Clone)]
pub struct Config {
    #[envconfig(from = "JIRA_USER", default = "")]
    pub(crate) user: String,
    #[envconfig(from = "JIRA_SECRET", default = "")]
    pub(crate) secret: String,
    #[envconfig(from = "JIRA_TOKEN", default = "")]
    pub(crate) token: String,
    #[envconfig(from = "JIRA_BASE_URL", default = "")]
    pub(crate) base_url: String,
    #[envconfig(from = "JIRA_PROJECT_KEY", default = "")]
    pub(crate) project_key: String,    
    #[envconfig(from = "JIRA_MSTEAMS_LINK_FIELD_NAME", default = "")]
    pub(crate) msteams_link_field_name: String,    
    #[envconfig(from = "JIRA_MSTEAMS_LINK_FIELD_JQL_NAME", default = "")]
    pub(crate) msteams_link_field_jql_name: String,    
}
