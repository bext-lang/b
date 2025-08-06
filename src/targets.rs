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
            if strcmp(target.name(), name) == 0 {
                return Some(target);
            }
        }
        None
    }
    pub unsafe fn name(self) -> *const c_char {
        self.api.name()
    }
    pub unsafe fn new(self, a: *mut arena::Arena, args: *const [*const c_char]) -> Option<*mut c_void> {
        match self.api {
            TargetAPI::V1 { new, .. } => new(a, args)
        }
    }
    pub unsafe fn build (
        self,
        gen: *mut c_void,
        program: *const Program,
        program_path: *const c_char,
        garbage_base: *const c_char,
        nostdlib: bool,
        debug: bool,
    ) -> Option<()> {
        match self.api {
            TargetAPI::V1 { build, .. } => build(gen, program, program_path, garbage_base, nostdlib, debug),
        }
    }
    pub unsafe fn run (
        self,
        gen: *mut c_void,
        program_path: *const c_char,
        run_args: *const [*const c_char],
    ) -> Option<()> {
        match self.api {
            TargetAPI::V1 { run, .. } => run(gen, program_path, run_args),
        }
    }
    pub unsafe fn file_ext(self) -> *const c_char {
        match self.api {
            TargetAPI::V1 { file_ext, .. } => file_ext,
        }
    }
}

pub unsafe fn register_apis(targets: *mut Array<Target>, apis: *const [TargetAPI], codegen_name: *const c_char) -> Option<()> {
    for i in 0..apis.len() {
        let api = (*apis)[i];
        for j in 0..(*targets).count {
            let target = *(*targets).items.add(j);
            if strcmp(target.name(), api.name()) == 0 {
                if strcmp(target.codegen_name, codegen_name) == 0 {
                    log(Log_Level::ERROR, c!("TARGET NAME CONFLICT: Codegen %s defines target %s more than once"), codegen_name, api.name());
                } else {
                    log(Log_Level::ERROR, c!("TARGET NAME CONFLICT: Codegens %s and %s define the same target %s"), codegen_name, target.codegen_name, api.name());
                }
                return None;
            }
        }
        da_append(targets, Target {codegen_name, api});
    }
    Some(())
}

#[derive(Clone, Copy)]
pub enum TargetAPI {
    V1 {
        name: *const c_char,
        file_ext: *const c_char,
        new: unsafe fn(
            a: *mut arena::Arena,
            args: *const [*const c_char]
        ) -> Option<*mut c_void>,
        build: unsafe fn(
            gen: *mut c_void,
            program: *const Program,
            program_path: *const c_char,
            garbage_base: *const c_char,
            nostdlib: bool,
            debug: bool,
        ) -> Option<()>,
        run: unsafe fn(
            gen: *mut c_void,
            program_path: *const c_char,
            run_args: *const [*const c_char],
        ) -> Option<()>,
    }
}

impl TargetAPI {
    pub unsafe fn name(self) -> *const c_char {
        match self {
            TargetAPI::V1 { name, .. } => name,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Linux,
    Windows,
    Darwin,
}
