#![no_main]
#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_macros)]

#[macro_use]
pub mod crust;
#[macro_use]
pub mod nob;
pub mod targets;
pub mod runner;
pub mod flag;

use core::ffi::*;
use core::cmp;
use core::mem::{zeroed, size_of};
use crust::libc::*;
use nob::*;
use targets::*;
use runner::mos6502::{Config, DEFAULT_LOAD_OFFSET};
use flag::*;

const GARBAGE_FOLDER: *const c_char = c!("./build/tests/");

#[derive(Copy, Clone)]
pub enum Status {
    Ok,
    BuildFail,
    RunFail,
}

#[derive(Copy, Clone)]
struct Report {
    name: *const c_char,
    statuses: Array<Status>,
}

pub unsafe fn run_test(cmd: *mut Cmd, output: *mut String_Builder, test_folder: *const c_char, name: *const c_char, target: Target) -> Status {
    // TODO: add timeouts for running and building in case they go into infinite loop or something
    let input_path = temp_sprintf(c!("%s/%s.b"), test_folder, name);
    let output_path = temp_sprintf(c!("%s/%s.%s"), GARBAGE_FOLDER, name, match target {
        Target::Fasm_x86_64_Windows => c!("exe"),
        Target::Fasm_x86_64_Linux   => c!("fasm-x86_64-linux"),
        Target::Gas_AArch64_Linux   => c!("gas-aarch64-linux"),
        Target::Uxn                 => c!("rom"),
        Target::Mos6502             => c!("6502"),
    });
    cmd_append! {
        cmd,
        c!("./build/b"),
        input_path,
        c!("-t"), name_of_target(target).unwrap(),
        c!("-o"), output_path,
    }
    if !cmd_run_sync_and_reset(cmd) {
        return Status::BuildFail;
    }
    let run_result = match target {
        Target::Fasm_x86_64_Linux   => runner::fasm_x86_64_linux::run(cmd, output_path, &[]),
        Target::Fasm_x86_64_Windows => runner::fasm_x86_64_windows::run(cmd, output_path, &[]),
        Target::Gas_AArch64_Linux   => runner::gas_aarch64_linux::run(cmd, output_path, &[]),
        Target::Uxn                 => runner::uxn::run(cmd, c!("uxncli"), output_path, &[]),
        Target::Mos6502             => runner::mos6502::run(output, Config {
            load_offset: DEFAULT_LOAD_OFFSET
        }, output_path),
    };
    if let None = run_result {
        return Status::RunFail;
    }
    Status::Ok
}

pub unsafe fn usage() {
    fprintf(stderr(), c!("Usage: %s [OPTIONS]\n"), flag_program_name());
    fprintf(stderr(), c!("OPTIONS:\n"));
    flag_print_options(stderr());
}

pub unsafe extern "C" fn compar_cstr(a: *const c_void, b: *const c_void) -> c_int {
    strcmp(*(a as *const *const c_char), *(b as *const *const c_char))
}

pub unsafe fn main(argc: i32, argv: *mut*mut c_char) -> Option<()> {
    let target_flags = flag_list(c!("t"), c!("Compilation targets to test on."));
    let cases_flags  = flag_list(c!("c"), c!("Test cases"));
    let test_folder  = flag_str(c!("dir"), c!("./tests/"), c!("Test folder"));
    let help         = flag_bool(c!("help"), false, c!("Print this help message"));
    // TODO: flag to print the list of tests
    // TODO: flag to print the list of targets
    // TODO: select test cases and targets by a glob pattern
    // See if https://github.com/tsoding/glob.h can be used here

    if !flag_parse(argc, argv) {
        usage();
        flag_print_error(stderr());
        return None;
    }

    if *help {
        usage();
        return Some(());
    }

    let mut output: String_Builder = zeroed();
    let mut cmd: Cmd = zeroed();
    let mut reports: Array<Report> = zeroed();

    let mut targets: Array<Target> = zeroed();
    if (*target_flags).count == 0 {
        for j in 0..TARGET_NAMES.len() {
            let Target_Name { name: _, target } = (*TARGET_NAMES)[j];
            da_append(&mut targets, target);
        }
    } else {
        for j in 0..(*target_flags).count {
            let target_name = *(*target_flags).items.add(j);
            if let Some(target) = target_by_name(target_name) {
                da_append(&mut targets, target);
            } else {
                fprintf(stderr(), c!("ERRRO: unknown target `%s`\n"), target_name);
                return None;
            }
        }
    }

    let mut cases: Array<*const c_char> = zeroed();
    if (*cases_flags).count == 0 {
        let mut test_files: File_Paths = zeroed();
        if !read_entire_dir(*test_folder, &mut test_files) { return None; }
        qsort(test_files.items as *mut c_void, test_files.count, size_of::<*const c_char>(), compar_cstr);

        for i in 0..test_files.count {
            let test_file = *test_files.items.add(i);
            if *test_file == '.' as c_char { continue; }
            let Some(case_name) = temp_strip_suffix(test_file, c!(".b")) else { continue; };
            da_append(&mut cases, case_name);
        }
    } else {
        for i in 0..(*cases_flags).count {
            let case_name = *(*cases_flags).items.add(i);
            da_append(&mut cases, case_name);
        }
    }

    if !mkdir_if_not_exists(GARBAGE_FOLDER) { return None; }

    // TODO: Parallelize the test runner.
    // Probably using `cmd_run_async_and_reset`.
    // Also don't forget to add the `-j` flag.
    for i in 0..cases.count {
        let test_name = *cases.items.add(i);
        let mut report = Report {
            name: test_name,
            statuses: zeroed(),
        };
        for j in 0..targets.count {
            let target = *targets.items.add(j);
            da_append(&mut report.statuses, run_test(&mut cmd, &mut output, *test_folder, test_name, target));
        }
        da_append(&mut reports, report);
    }

    // TODO: generate HTML reports and deploy them somewhere automatically

    let mut width = 0;
    for i in 0..reports.count {
        let report = *reports.items.add(i);
        width = cmp::max(width, strlen(report.name));
    }

    printf(c!("%*s\x1b[32mK\x1b[0m - success\n"), width + 2, c!(""));
    printf(c!("%*s\x1b[33mB\x1b[0m - failed to build\n"), width + 2, c!(""));
    printf(c!("%*s\x1b[31mR\x1b[0m - runtime error\n"), width + 2, c!(""));
    printf(c!("\n"));

    for j in 0..targets.count {
        let target = *targets.items.add(j);
        printf(c!("%*s"), width + 2, c!(""));
        for _ in 0..j {
            printf(c!("│ "));
        }
        printf(c!("┌─%s\n"), name_of_target(target).unwrap(), j);
    }

    for i in 0..reports.count {
        let report = *reports.items.add(i);
        printf(c!("%*s:"), width, report.name);
        for j in 0..report.statuses.count {
            let status = *report.statuses.items.add(j);
            match status {
                Status::Ok        => printf(c!(" \x1b[32mK\x1b[0m")),
                Status::BuildFail => printf(c!(" \x1b[33mB\x1b[0m")),
                Status::RunFail   => printf(c!(" \x1b[31mR\x1b[0m")),
            };
        }
        printf(c!("\n"));
    }

    for j in (0..targets.count).rev() {
        let target = *targets.items.add(j);
        printf(c!("%*s"), width + 2, c!(""));
        for _ in 0..j {
            printf(c!("│ "));
        }
        printf(c!("└─%s\n"), name_of_target(target).unwrap(), j);
    }

    printf(c!("\n"));
    printf(c!("%*s\x1b[32mK\x1b[0m - success\n"), width + 2, c!(""));
    printf(c!("%*s\x1b[33mB\x1b[0m - failed to build\n"), width + 2, c!(""));
    printf(c!("%*s\x1b[31mR\x1b[0m - runtime error\n"), width + 2, c!(""));

    Some(())
}
