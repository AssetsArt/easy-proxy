use std::sync::Once;

pub mod cli;

pub struct Config {
    pub authen: Option<String>,
    pub host: String,
}

pub fn load_global_config() -> &'static Config {
    static INIT_CONFIG: Once = Once::new();
    static mut GLOBAL_CONFIG: *const Config = std::ptr::null();

    INIT_CONFIG.call_once(|| {
        println!("load config");
        let cli_config: cli::CliConfig = argh::from_env();
        let config = Config {
            host: cli_config.host.unwrap_or("0.0.0.0:8100".to_string()),
            authen: match cli_config.authen {
                Some(authen) => Some(authen),
                None => None,
            },
        };
        unsafe {
            GLOBAL_CONFIG = Box::into_raw(Box::new(config));
        }
    });

    unsafe { &*GLOBAL_CONFIG }
}
