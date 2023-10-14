pub mod models;

// use
use std::sync::Once;
use std::{fs::File, io::BufReader};

// This is a global variable that is initialized once
static INIT_CONFIG: Once = Once::new();
static mut GLOBAL_CONFIG: *const models::Config = std::ptr::null();

pub fn get_config() -> &'static models::Config {
    INIT_CONFIG.call_once(|| {
        let cwd_path = std::env::current_dir().unwrap();
        let cwd_path = cwd_path.join("config/easy_proxy.yaml");
        let f = File::open(cwd_path).expect("Unable to open file");
        let rdr = BufReader::new(f);
        let mut conf: models::Config = serde_yaml::from_reader(rdr).unwrap();

        // prefix relative path with current dir
        if !conf.database.file.starts_with('/') {
            let current_dir = std::env::current_dir().unwrap();
            conf.database.file = current_dir
                .join(conf.database.file)
                .to_str()
                .unwrap()
                .to_string();
        }
        if !conf.jwt.private.starts_with('/') {
            let current_dir = std::env::current_dir().unwrap();
            conf.jwt.private = current_dir
                .join(conf.jwt.private)
                .to_str()
                .unwrap()
                .to_string();
        }
        if !conf.jwt.public.starts_with('/') {
            let current_dir = std::env::current_dir().unwrap();
            conf.jwt.public = current_dir
                .join(conf.jwt.public)
                .to_str()
                .unwrap()
                .to_string();
        }
        // println!("config: {:#?}", conf);
        unsafe {
            GLOBAL_CONFIG = Box::into_raw(Box::new(conf));
        }
    });

    unsafe { &*GLOBAL_CONFIG }
}
