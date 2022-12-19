use std::{ffi::{CStr, CString}, os::raw::c_char, str::FromStr};

#[no_mangle]
pub extern "C" fn clash_run_with_config_string(rt_id: u16, config: *const c_char) {
    let mut r = String::from_str("ok").expect("");
    if let Ok(config) = unsafe { CStr::from_ptr(config).to_str() } {
        let opts = leaf::StartOptions {
            config: leaf::Config::Str(config.to_string()),
            auto_reload: false,
            runtime_opt: leaf::RuntimeOption::SingleThread,
        };
        hpts::start_with_json(config.to_string());
        if let Err(e) = leaf::start(rt_id, opts) {
            r = e.to_string()
        }
    } else {
        r = "config error".to_string();
    }
    log::info!("error:{}",r);
   
}




#[no_mangle]
pub extern "C" fn clash_reload(rt_id: u16) -> *const c_char {
    let mut r = String::from_str("ok").expect("");
    if let Err(e) = leaf::reload(rt_id) {
        r = e.to_string();
    }
    return CString::new(r).unwrap().into_raw();
}


#[no_mangle]
pub extern "C" fn clash_shutdown(rt_id: u16) -> bool {
    leaf::shutdown(rt_id)
}


