use core::ffi::*;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Jim {
    pub sink: *mut c_char,
    pub sink_count: usize,
    pub sink_capacity: usize,
    pub scopes: *mut c_void,
    pub scopes_count: usize,
    pub scopes_capacity: usize,
    pub pp: usize,
}

extern "C" {
    pub fn jim_begin(jim: *mut Jim);
    pub fn jim_object_begin(jim: *mut Jim);
    pub fn jim_member_key(jim: *mut Jim, s: *const c_char);
    pub fn jim_object_end(jim: *mut Jim);
    pub fn jim_string(jim: *mut Jim, s: *const c_char);
    pub fn jim_array_begin(jim: *mut Jim);
    pub fn jim_array_end(jim: *mut Jim);
}
