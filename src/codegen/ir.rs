use core::ffi::*;
use crate::nob::*;
use crate::{Op, Binop, OpWithLocation, Arg, Func, Compiler};

pub unsafe fn dump_arg(output: *mut String_Builder, arg: Arg) {
    match arg {
        Arg::External(name)     => sb_appendf(output, c!("%s"), name),
        Arg::Deref(index)       => sb_appendf(output, c!("deref[%zu]"), index),
        Arg::RefAutoVar(index)  => sb_appendf(output, c!("ref auto[%zu]"), index),
        Arg::RefExternal(name)  => sb_appendf(output, c!("ref %s"), name),
        Arg::Literal(value)     => sb_appendf(output, c!("%ld"), value),
        Arg::AutoVar(index)     => sb_appendf(output, c!("auto[%zu]"), index),
        Arg::DataOffset(offset) => sb_appendf(output, c!("data[%zu]"), offset),
    };
}

pub unsafe fn generate_function(name: *const c_char, params_count: usize, auto_vars_count: usize, body: *const [OpWithLocation], output: *mut String_Builder) {
    sb_appendf(output, c!("%s(%zu, %zu):\n"), name, params_count, auto_vars_count);
    for i in 0..body.len() {
        sb_appendf(output, c!("%8zu:"), i);
        match (*body)[i].opcode {
            Op::Return {arg} => {
                sb_appendf(output, c!("    return "));
                if let Some(arg) = arg {
                    dump_arg(output, arg);
                }
                sb_appendf(output, c!("\n"));
            },
            Op::Store{index, arg} => {
                sb_appendf(output, c!("    store deref[%zu], "), index);
                dump_arg(output, arg);
                sb_appendf(output, c!("\n"));
            }
            Op::ExternalAssign{name, arg} => {
                sb_appendf(output, c!("    %s = "), name);
                dump_arg(output, arg);
                sb_appendf(output, c!("\n"));
            }
            Op::AutoAssign{index, arg} => {
                sb_appendf(output, c!("    auto[%zu] = "), index);
                dump_arg(output, arg);
                sb_appendf(output, c!("\n"));
            }
            Op::Negate{result, arg} => {
                sb_appendf(output, c!("    auto[%zu] = -"), result);
                dump_arg(output, arg);
                sb_appendf(output, c!("\n"));
            }
            Op::UnaryNot{result, arg} => {
                sb_appendf(output, c!("    auto[%zu] = !"), result);
                dump_arg(output, arg);
                sb_appendf(output, c!("\n"));
            }
            Op::Binop {binop, index, lhs, rhs} => {
                sb_appendf(output, c!("    auto[%zu] = "), index);
                dump_arg(output, lhs);
                match binop {
                    Binop::BitOr        => sb_appendf(output, c!(" | ")),
                    Binop::BitAnd       => sb_appendf(output, c!(" & ")),
                    Binop::BitShl       => sb_appendf(output, c!(" << ")),
                    Binop::BitShr       => sb_appendf(output, c!(" >> ")),
                    Binop::Plus         => sb_appendf(output, c!(" + ")),
                    Binop::Minus        => sb_appendf(output, c!(" - ")),
                    Binop::Mod          => sb_appendf(output, c!(" %% ")),
                    Binop::Div          => sb_appendf(output, c!(" / ")),
                    Binop::Mult         => sb_appendf(output, c!(" * ")),
                    Binop::Less         => sb_appendf(output, c!(" < ")),
                    Binop::Greater      => sb_appendf(output, c!(" > ")),
                    Binop::Equal        => sb_appendf(output, c!(" == ")),
                    Binop::NotEqual     => sb_appendf(output, c!(" != ")),
                    Binop::GreaterEqual => sb_appendf(output, c!(" >= ")),
                    Binop::LessEqual    => sb_appendf(output, c!(" < ")),
                };
                dump_arg(output, rhs);
                sb_appendf(output, c!("\n"));
            }
            Op::Funcall{result, name, args} => {
                sb_appendf(output, c!("    auto[%zu] = call(\"%s\""), result, name);
                for i in 0..args.count {
                    sb_appendf(output, c!(", "));
                    dump_arg(output, *args.items.add(i));
                }
                sb_appendf(output, c!(")\n"));
            }
            Op::Asm {args} => {
                sb_appendf(output, c!("   __asm__(\n"));
                for i in 0..args.count {
                    let arg = *args.items.add(i);
                    sb_appendf(output, c!("    %s\n"), arg);
                }
                sb_appendf(output, c!(")\n"));
            }

            Op::JmpIfNot{addr, arg} => {
                sb_appendf(output, c!("    jmp_if_not %zu:, "), addr);
                dump_arg(output, arg);
                sb_appendf(output, c!("\n"));
            }
            Op::Jmp{addr} => {
                sb_appendf(output, c!("    jmp %zu:\n"), addr);
            }
        }
    }
}

pub unsafe fn generate_funcs(output: *mut String_Builder, funcs: *const [Func]) {
    sb_appendf(output, c!("-- Functions --\n"));
    sb_appendf(output, c!("\n"));
    for i in 0..funcs.len() {
        generate_function((*funcs)[i].name, (*funcs)[i].params_count, (*funcs)[i].auto_vars_count, da_slice((*funcs)[i].body), output);
    }
}

pub unsafe fn generate_extrns(output: *mut String_Builder, extrns: *const [*const c_char]) {
    sb_appendf(output, c!("\n"));
    sb_appendf(output, c!("-- External Symbols --\n\n"));
    for i in 0..extrns.len() {
        sb_appendf(output, c!("    %s\n"), (*extrns)[i]);
    }
}

pub unsafe fn generate_globals(output: *mut String_Builder, globals: *const [*const c_char]) {
    sb_appendf(output, c!("\n"));
    sb_appendf(output, c!("-- Global Variables --\n\n"));
    for i in 0..globals.len() {
        sb_appendf(output, c!("    %s\n"), (*globals)[i]);
    }
}

pub unsafe fn generate_data_section(output: *mut String_Builder, data: *const [u8]) {
    if data.len() > 0 {
        sb_appendf(output, c!("\n"));
        sb_appendf(output, c!("-- Data Section --\n"));
        sb_appendf(output, c!("\n"));

        const ROW_SIZE: usize = 12;
        for i in (0..data.len()).step_by(ROW_SIZE) {
            sb_appendf(output, c!("%04X:"), i as c_uint);
            for j in i..(i+ROW_SIZE) {
                if j < data.len() {
                    sb_appendf(output, c!(" "));
                    sb_appendf(output, c!("%02X"), (*data)[j] as c_uint);
                } else {
                    sb_appendf(output, c!("   "));
                }
            }

            sb_appendf(output, c!(" | "));
            for j in i..(i+ROW_SIZE).min(data.len()) {
                let ch = (*data)[j] as char;
                let c = if ch.is_ascii_whitespace() {
                    // display all whitespace as a regular space
                    // stops '\t', '\n', '\b' from messing up the formatting
                    ' '
                } else if ch.is_ascii_graphic() {
                    ch
                } else {
                    // display all non-printable characters as '.' (eg. NULL)
                    '.'
                };
                sb_appendf(output, c!("%c"), c as c_uint);
            }

            sb_appendf(output, c!("\n"));
        }
    }
}

pub unsafe fn generate_program(output: *mut String_Builder, c: *const Compiler) {
    generate_funcs(output, da_slice((*c).funcs));
    generate_extrns(output, da_slice((*c).extrns));
    generate_globals(output, da_slice((*c).globals));
    generate_data_section(output, da_slice((*c).data));
}
