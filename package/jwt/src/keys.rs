use std::sync::Once;

pub struct Keys {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

// This is a global variable that is initialized once
static INIT_KEYS: Once = Once::new();
static mut GLOBAL_KEYS: *const Keys = std::ptr::null();

pub fn get_keys() -> &'static Keys {
    INIT_KEYS.call_once(|| {
        let conf = config::get_config();
        let private = std::fs::read(&conf.jwt.private).unwrap();
        let public = std::fs::read(&conf.jwt.public).unwrap();
        // println!("Loaded JWT keys");
        unsafe {
            GLOBAL_KEYS = Box::into_raw(Box::new(Keys { private, public }));
        }
    });

    unsafe { &*GLOBAL_KEYS }
}
