use argh::FromArgs;

#[derive(FromArgs, Clone, Debug)]
/// Easy proxy server
pub struct CliConfig {
    /// authen header
    #[argh(option)]
    pub authen: Option<String>,

    #[argh(option)]
    /// host server default 0.0.0.0:8100
    pub host: Option<String>,

    #[argh(option)]
    /// web api and ui server default 0.0.0.0:3100
    pub api_host: Option<String>,

    #[argh(option)]
    /// jwt cert file default cert/jwt.pem
    pub jwt_cert: Option<String>,
    
}
