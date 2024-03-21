use envconfig::Envconfig;

#[derive(Envconfig, Clone)]
pub struct Config {
    #[envconfig(from = "API_ADDR", default = "0.0.0.0:8443")]
    pub addr: String,
    #[envconfig(from = "SHUTDOWN_TIMEOUT", default = "60")]
    pub shutdown_timeout: u64,
}
