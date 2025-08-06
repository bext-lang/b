#![no_main]
#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_macros)]

#[macro_use]
#[path = "./src/crust.rs"]
pub mod crust;
#[macro_use]
#[path = "./src/nob.rs"]
pub mod nob;

use core::mem::zeroed;
use core::ffi::c_char;
use crate::nob::*;
use crust::libc::*;

pub unsafe fn select_object_files(
    build_path: *const c_char,
    os_target: *const c_char,
    objects_to_build: &[*const c_char],
    object_file_names: &mut Array<*const c_char>,
    c_file_names: &mut Array<*const c_char>,
) -> bool {
    let thirdparty_path = c!("./thirdparty");
    for i in 0..objects_to_build.len() {
        da_append(c_file_names, temp_sprintf(c!("%s/%s.c"), thirdparty_path, objects_to_build[i]));
        if strcmp(os_target, c!("linux")) == 0 {
            da_append(object_file_names, temp_sprintf(c!("%s/%s.posix.o"), build_path, objects_to_build[i]));
        } else if strcmp(os_target, c!("windows")) == 0 {
            da_append(object_file_names, temp_sprintf(c!("%s/%s.mingw32.o"), build_path, objects_to_build[i]));
        } else {
            log(Log_Level::ERROR, c!("The target '%s' is not available. You should build it yourself I guess..."), os_target);
            return false;
        }
    }
    return true;
}

pub unsafe fn build_objects(
    cmd: &mut Cmd,
    os_target: *const c_char,
    object_file_names: &mut Array<*const c_char>,
    c_file_names: &mut Array<*const c_char>,
    ld_flags: *const c_char,
) -> bool {
    assert!(object_file_names.count == c_file_names.count);

    // TODO: This should easily be parallelizable - but I'm too lazy to do it right now.
    //   Nextness (2025-08-12 22:25:40)
    if strcmp(os_target, c!("linux")) == 0 {
        for i in 0..object_file_names.count {
            cmd_append! {
                cmd,
                c!("cc"), c!("-fPIC"), c!("-g"),
                c!("-c"), *c_file_names.items.add(i),
                c!("-o"), *object_file_names.items.add(i),
                ld_flags,
            }
            if !cmd_run_sync_and_reset(cmd) { return false };
        }
    } else if strcmp(os_target, c!("windows")) == 0 {
        for i in 0..object_file_names.count {
            cmd_append! {
                cmd,
                c!("x86_64-w64-mingw32-gcc"), c!("-fPIC"), c!("-g"),
                c!("-c"), *c_file_names.items.add(i),
                c!("-o"), *object_file_names.items.add(i),
            }
            if !cmd_run_sync_and_reset(cmd) { return false };
        }
    } else {
        log(Log_Level::ERROR, c!("The target '%s' is not available. You should build it yourself I guess..."), os_target);
        return false
    }

    return true;
}

pub unsafe fn build_b(
    cmd: &mut Cmd,
    src_path: *const c_char,
    build_path: *const c_char,
    object_file_names: &mut Array<*const c_char>,
    crust_flags: &[*const c_char],
    ld_flags: &[*const c_char],
) -> bool {

    da_append(cmd, c!("rustc"));
    da_append_many(cmd, crust_flags);

    da_append(cmd, temp_sprintf(c!("-L%s"), build_path));
    for i in 0..object_file_names.count {
        let link_args = temp_sprintf(c!("-Clink-args=%s"), *object_file_names.items.add(i));
        da_append(cmd, link_args);
    }

    da_append_many(cmd, ld_flags);
    da_append(cmd, temp_sprintf(c!("%s/b.rs"), src_path));
    da_append(cmd, c!("-o"));
    da_append(cmd, temp_sprintf(c!("%s/b"), build_path));
    if !cmd_run_sync_and_reset(cmd) { return false };

    return true;
}

pub unsafe fn main(mut _argc: i32, mut _argv: *mut*mut c_char) -> Option<()> {
    const src_path: *const c_char = c!("src");
    const build_path: *const c_char = c!("build");

    if !mkdir_if_not_exists(build_path) { return None }

    // TODO: memory leak
    let mut cmd: Cmd = zeroed();
    let mut object_file_names: Array<*const c_char> = zeroed();
    let mut c_file_names:  Array<*const c_char> = zeroed();

    let os_target = c!("linux");
    const objects_to_build: &[*const c_char] = c_many!(
        "nob", "flag", "glob", "libc", "arena", "time", "jim", "jimp", "shlex"
    );
    let crust_flags = c_many!("-g", "--edition=2021", "-Copt-level=0", "-Cpanic=abort");
    let ld_flags = c_many!("-lc", "-lgcc");

    if !select_object_files(build_path, os_target, objects_to_build, &mut object_file_names, &mut c_file_names) { return None; }
    if !build_objects(&mut cmd, os_target, &mut object_file_names, &mut c_file_names, c!("-lc -lgcc")) { return None; }
    build_b(&mut cmd, src_path, build_path, &mut object_file_names, crust_flags, ld_flags);

    return Some(());
}

