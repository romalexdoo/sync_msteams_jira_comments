use envconfig::Envconfig;

#[derive(Envconfig, Clone)]
pub struct Config {
    #[envconfig(from = "MICROSOFT_TENANT_ID", default = "")]
    pub tenant_id: String,
    #[envconfig(from = "MICROSOFT_CLIENT_ID", default = "")]
    pub client_id: String,
    #[envconfig(from = "MICROSOFT_CLIENT_SECRET", default = "")]
    pub client_secret: String,
    #[envconfig(from = "MICROSOFT_SUBSCRIPTION_NOTIFICATION_URL", default = "")]
    pub notification_url: String,
    #[envconfig(from = "MICROSOFT_SUBSCRIPTION_LIFECYCLE_NOTIFICATION_URL", default = "")]
    pub lifecycle_notification_url: String,
    #[envconfig(from = "MICROSOFT_OAUTH_URL", default = "")]
    pub oauth_url: String,
    #[envconfig(from = "TEAMS_GROUP_ID", default = "")]
    pub group_id: String,
    #[envconfig(from = "TEAMS_CHANNEL_ID", default = "")]
    pub channel_id: String,
    #[envconfig(from = "TEAMS_USER", default = "")]
    pub teams_user: String,
}
