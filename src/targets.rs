use core::ffi::*;
use crate::strcmp;
use crate::ir::Program;
use crate::arena;
use crate::nob::*;

// TODO: add wasm target
//   Don't touch this TODO! @rexim wants to stream it!

#[derive(Clone, Copy)]
pub struct Target {
    pub codegen_name: *const c_char,
    pub api: TargetAPI,
}

impl Target {
    pub unsafe fn by_name(targets: *const [Target], name: *const c_char) -> Option<Target> {
        for i in 0..targets.len() {
            let target = (*targets)[i];
            if strcmp(target.api.name, name) == 0 {
                return Some(target);
            }
        }
        None
    }
}

pub unsafe fn register_apis(targets: *mut Array<Target>, apis: *const [TargetAPI], codegen_name: *const c_char) -> Option<()> {
    for i in 0..apis.len() {
        let api = (*apis)[i];
        for j in 0..(*targets).count {
            let target = *(*targets).items.add(j);
            if strcmp(target.api.name, api.name) == 0 {
                if strcmp(target.codegen_name, codegen_name) == 0 {
                    log(Log_Level::ERROR, c!("TARGET NAME CONFLICT: Codegen %s defines target %s more than once"), codegen_name, api.name);
                } else {
                    log(Log_Level::ERROR, c!("TARGET NAME CONFLICT: Codegens %s and %s define the same target %s"), codegen_name, target.codegen_name, api.name);
                }
                return None;
            }
        }
        da_append(targets, Target {codegen_name, api});
    }
    Some(())
}

#[derive(Clone, Copy)]
pub struct TargetAPI {
    pub name: *const c_char,
    pub file_ext: *const c_char,
    pub new: unsafe fn(
        a: *mut arena::Arena,
        args: *const [*const c_char]
    ) -> Option<*mut c_void>,
    pub build: unsafe fn(
        gen: *mut c_void,
        program: *const Program,
        program_path: *const c_char,
        garbage_base: *const c_char,
        nostdlib: bool,
        debug: bool,
    ) -> Option<()>,
    pub run: unsafe fn(
        gen: *mut c_void,
        program_path: *const c_char,
        run_args: *const [*const c_char],
    ) -> Option<()>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Windows,
    Darwin,
}
