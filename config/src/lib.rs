pub mod models;

// use
use std::sync::Once;
use std::{fs::File, io::BufReader};

// This is a global variable that is initialized once
static INIT_CONFIG: Once = Once::new();
static mut GLOBAL_CONFIG: *const models::AppConfig = std::ptr::null();

pub fn app_config() -> &'static models::AppConfig {
    INIT_CONFIG.call_once(|| {
        // FROM ENV
        let cwd_path = std::env::var("EASY_PROXY_CONF");
        let cwd_path = match cwd_path {
            Ok(val) => val,
            Err(_e) => {
                let cwd_path = std::env::current_dir().expect("Unable to get current dir");
                cwd_path
                    .join(".config/easy_proxy.yaml")
                    .to_str()
                    .expect("Unable to convert path")
                    .to_string()
            }
        };

        let open_conf = File::open(cwd_path).expect("Unable to open file");
        let read_conf = BufReader::new(open_conf);
        let conf: models::AppConfig =
            serde_yaml::from_reader(read_conf).expect("Unable to read conf file");

        unsafe {
            GLOBAL_CONFIG = Box::into_raw(Box::new(conf));
        }
    });

    unsafe { &*GLOBAL_CONFIG }
}
