#![no_main]
#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_macros)]

#[macro_use]
pub mod crust;
pub mod nob;

use core::ffi::*;
use core::mem::zeroed;
use nob::*;
use crust::libc::*;
use crust::*;

pub unsafe fn main(mut _argc: i32, mut _argv: *mut*mut c_char) -> Option<()> {
    let parent = c!("./src/codegen");
    let mut children: File_Paths = zeroed();
    if !read_entire_dir(parent, &mut children) { return None; }
    qsort(children.items as *mut c_void, children.count, size_of::<*const c_char>(), compar_cstr);
    let mut sb: String_Builder = zeroed();
    sb_appendf(&mut sb, c!("codegens! {\n"));
    for i in 0..children.count {
        let child = *children.items.add(i);
        if *child == '.' as c_char { continue; }
        if strcmp(child, c!("mod.rs")) == 0 { continue; }
        let child = temp_strip_suffix(child, c!(".rs")).unwrap_or(child);
        sb_appendf(&mut sb, c!("    %s,\n"), child);
    }
    sb_appendf(&mut sb, c!("}\n"));
    let output_path = temp_sprintf(c!("%s/.INDEX.rs"), parent);
    write_entire_file(output_path, sb.items as _, sb.count)?;
    log(Log_Level::INFO, c!("Generated %s"), output_path);
    Some(())
}
