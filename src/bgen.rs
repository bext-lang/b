//! The meta-program that analyses <root>/src/codegen/ folder and makes the custom pluggable codegens
//! available to b and btest.
//!
//! Rust's proc macros suck btw jfyi
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

pub unsafe fn aggregate_libb(folder_path: *const c_char) -> Option<()> {
    let mut children: File_Paths = zeroed();

    if !read_entire_dir(folder_path, &mut children) { return None; }

    for i in 0..children.count {
        let child = *children.items.add(i);
        if *child == '.' as c_char { continue; }
        if !copy_file(
            temp_sprintf(c!("%s/%s"), folder_path, child),
            temp_sprintf(c!("./build/libb/%s"), child),
        ) { return None; }
    }
    Some(())
}

pub unsafe fn main(mut _argc: i32, mut _argv: *mut*mut c_char) -> Option<()> {
    if !mkdir_if_not_exists(c!("./build/")) { return None; }
    if !mkdir_if_not_exists(c!("./build/libb/")) { return None; }

    aggregate_libb(c!("./libb/"))?;

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
        // TODO: skip the modules that have invalid Rust names.
        //   Or is there any way to accomodate them into the Rust module system too?
        //   In any case we should do something with the invalid module names.
        if let Some(child) = temp_strip_suffix(child, c!(".rs")) {
            sb_appendf(&mut sb, c!("    %s,\n"), child);
            log(Log_Level::INFO, c!("--- CODEGEN %s ---"), child);
        } else {
            sb_appendf(&mut sb, c!("    %s,\n"), child);
            log(Log_Level::INFO, c!("--- CODEGEN %s ---"), child);
            let codegen_libb = temp_sprintf(c!("%s/%s/libb/"), parent, child);
            if file_exists(codegen_libb)? {
                aggregate_libb(codegen_libb)?;
            }
        }
    }
    sb_appendf(&mut sb, c!("}\n"));
    let output_path = temp_sprintf(c!("%s/.INDEX.rs"), parent);
    write_entire_file(output_path, sb.items as _, sb.count)?;
    log(Log_Level::INFO, c!("Generated %s"), output_path);
    Some(())
}
