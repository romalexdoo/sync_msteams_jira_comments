use envconfig::Envconfig;

#[derive(Envconfig, Clone)]
pub struct Config {
    #[envconfig(from = "MICROSOFT_TENANT_ID", default = "")]
    pub(crate) tenant_id: String,
    #[envconfig(from = "MICROSOFT_CLIENT_ID", default = "")]
    pub(crate) client_id: String,
    #[envconfig(from = "MICROSOFT_CLIENT_SECRET", default = "")]
    pub(crate) client_secret: String,
    #[envconfig(from = "MICROSOFT_SUBSCRIPTION_NOTIFICATION_URL", default = "")]
    pub(crate) notification_url: String,
    #[envconfig(from = "MICROSOFT_SUBSCRIPTION_LIFECYCLE_NOTIFICATION_URL", default = "")]
    pub(crate) lifecycle_notification_url: String,
    #[envconfig(from = "MICROSOFT_OAUTH_URL", default = "")]
    pub(crate) oauth_url: String,
    #[envconfig(from = "TEAMS_GROUP_ID", default = "")]
    pub(crate) group_id: String,
    #[envconfig(from = "TEAMS_CHANNEL_ID", default = "")]
    pub(crate) channel_id: String,
    #[envconfig(from = "TEAMS_USER", default = "")]
    pub(crate) teams_user: String,
}
