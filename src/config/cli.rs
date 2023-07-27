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
}
