extern "C" {
    fn links_sys_static_value() -> i32;
    fn links_sys_shared_value() -> i32;
}

pub fn value() -> &'static str {
    "links-sys"
}

pub fn native_sum() -> i32 {
    unsafe { links_sys_static_value() + links_sys_shared_value() }
}
