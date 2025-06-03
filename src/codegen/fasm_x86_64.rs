use core::ffi::*;
use core::cmp;
use crate::{Op, Binop, OpWithLocation, Arg, Func, Compiler, align_bytes};
use crate::nob::*;
use crate::crust::libc::*;
use crate::{missingf, Loc};
use crate::codegen::Os;

pub unsafe fn load_arg_to_reg(arg: Arg, reg: *const c_char, output: *mut String_Builder) {
    match arg {
        Arg::Deref(index) => {
            sb_appendf(output, c!("    mov %s, [rbp-%zu]\n"), reg, index*8);
            sb_appendf(output, c!("    mov %s, [%s]\n"), reg, reg)
        }
        Arg::RefAutoVar(index)  => sb_appendf(output, c!("    lea %s, [rbp-%zu]\n"), reg, index*8),
        Arg::RefExternal(name)  => sb_appendf(output, c!("    lea %s, [_%s]\n"), reg, name),
        Arg::External(name)     => sb_appendf(output, c!("    mov %s, [_%s]\n"), reg, name),
        Arg::AutoVar(index)     => sb_appendf(output, c!("    mov %s, [rbp-%zu]\n"), reg, index*8),
        Arg::Literal(value)     => sb_appendf(output, c!("    mov %s, %ld\n"), reg, value),
        Arg::DataOffset(offset) => sb_appendf(output, c!("    mov %s, dat+%zu\n"), reg, offset),
    };
}

pub unsafe fn generate_function(name: *const c_char, name_loc: Loc, params_count: usize, auto_vars_count: usize, body: *const [OpWithLocation], output: *mut String_Builder, os: Os) {
    let stack_size = align_bytes(auto_vars_count*8, 16);
    sb_appendf(output, c!("public _%s as '%s'\n"), name, name);
    sb_appendf(output, c!("_%s:\n"), name);
    sb_appendf(output, c!("    push rbp\n"));
    sb_appendf(output, c!("    mov rbp, rsp\n"));
    if stack_size > 0 {
        sb_appendf(output, c!("    sub rsp, %zu\n"), stack_size);
    }
    assert!(auto_vars_count >= params_count);
    let registers: *const[*const c_char] = match os {
        Os::Linux   => &[c!("rdi"), c!("rsi"), c!("rdx"), c!("rcx"), c!("r8"), c!("r9")],
        Os::Windows => &[c!("rcx"), c!("rdx"), c!("r8"), c!("r9")], // https://en.wikipedia.org/wiki/X86_calling_conventions#Microsoft_x64_calling_convention
    };
    if params_count > registers.len() {
        missingf!(name_loc, c!("Too many parameters in function definition. We support only %zu but %zu were provided\n"), registers.len(), params_count);
    }
    for i in 0..params_count {
        let reg = (*registers)[i];
        sb_appendf(output, c!("    mov QWORD [rbp-%zu], %s\n"), (i + 1)*8, reg);
    }
    for i in 0..body.len() {
        sb_appendf(output, c!(".op_%zu:\n"), i);
        let op = (*body)[i];
        match op.opcode {
            Op::Return {arg} => {
                if let Some(arg) = arg {
                    load_arg_to_reg(arg, c!("rax"), output);
                }
                sb_appendf(output, c!("    mov rsp, rbp\n"));
                sb_appendf(output, c!("    pop rbp\n"));
                sb_appendf(output, c!("    ret\n"));
            }
            Op::Store {index, arg} => {
                sb_appendf(output, c!("    mov rax, [rbp-%zu]\n"), index*8);
                load_arg_to_reg(arg, c!("rbx"), output);
                sb_appendf(output, c!("    mov [rax], rbx\n"));
            }
            Op::ExternalAssign{name, arg} => {
                load_arg_to_reg(arg, c!("rax"), output);
                sb_appendf(output, c!("    mov [_%s], rax\n"), name);
            },
            Op::AutoAssign{index, arg} => {
                load_arg_to_reg(arg, c!("rax"), output);
                sb_appendf(output, c!("    mov QWORD [rbp-%zu], rax\n"), index*8);
            },
            Op::Negate {result, arg} => {
                load_arg_to_reg(arg, c!("rax"), output);
                sb_appendf(output, c!("    neg rax\n"));
                sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), result*8);
            }
            Op::UnaryNot{result, arg} => {
                sb_appendf(output, c!("    xor rbx, rbx\n"));
                load_arg_to_reg(arg, c!("rax"), output);
                sb_appendf(output, c!("    test rax, rax\n"));
                sb_appendf(output, c!("    setz bl\n"));
                sb_appendf(output, c!("    mov [rbp-%zu], rbx\n"), result*8);
            },
            Op::Binop {binop, index, lhs, rhs} => {
                match binop {
                    Binop::BitOr => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    or rax, rbx\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), index*8);
                    }
                    Binop::BitAnd => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    and rax, rbx\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), index*8);
                    }
                    Binop::BitShl => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rcx"), output);
                        sb_appendf(output, c!("    shl rax, cl\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), index*8);
                    }
                    Binop::BitShr => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rcx"), output);
                        sb_appendf(output, c!("    shr rax, cl\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), index*8);
                    }
                    Binop::Plus => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    add rax, rbx\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), index*8);
                    }
                    Binop::Minus  => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    sub rax, rbx\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), index*8);
                    }
                    Binop::Mod => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    cqo\n"));
                        sb_appendf(output, c!("    idiv rbx\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rdx\n"), index*8);
                    }
                    Binop::Div => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    cqo\n"));
                        sb_appendf(output, c!("    idiv rbx\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), index*8);
                    }
                    Binop::Mult => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    xor rdx, rdx\n"));
                        sb_appendf(output, c!("    imul rbx\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), index*8);
                    }
                    Binop::Less => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    xor rdx, rdx\n"));
                        sb_appendf(output, c!("    cmp rax, rbx\n"));
                        sb_appendf(output, c!("    setl dl\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rdx\n"), index*8);
                    }
                    Binop::Greater => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    xor rdx, rdx\n"));
                        sb_appendf(output, c!("    cmp rax, rbx\n"));
                        sb_appendf(output, c!("    setg dl\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rdx\n"), index*8);
                    }
                    Binop::Equal => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    xor rdx, rdx\n"));
                        sb_appendf(output, c!("    cmp rax, rbx\n"));
                        sb_appendf(output, c!("    sete dl\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rdx\n"), index*8);
                    }
                    Binop::NotEqual => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    xor rdx, rdx\n"));
                        sb_appendf(output, c!("    cmp rax, rbx\n"));
                        sb_appendf(output, c!("    setne dl\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rdx\n"), index*8);
                    }
                    Binop::GreaterEqual => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    xor rdx, rdx\n"));
                        sb_appendf(output, c!("    cmp rax, rbx\n"));
                        sb_appendf(output, c!("    setge dl\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rdx\n"), index*8);
                    }
                    Binop::LessEqual => {
                        load_arg_to_reg(lhs, c!("rax"), output);
                        load_arg_to_reg(rhs, c!("rbx"), output);
                        sb_appendf(output, c!("    xor rdx, rdx\n"));
                        sb_appendf(output, c!("    cmp rax, rbx\n"));
                        sb_appendf(output, c!("    setle dl\n"));
                        sb_appendf(output, c!("    mov [rbp-%zu], rdx\n"), index*8);
                    }
                }
            }
            Op::Funcall{result, name, args} => {
                match os {
                    Os::Linux => {
                        if args.count > registers.len() {
                            missingf!(op.loc, c!("Too many function call arguments. We support only %d but %zu were provided\n"), registers.len(), args.count);
                        }
                        // TODO: implement similar additional pushing argument to stack for Os::Linux as for Os::Windows
                        for i in 0..args.count {
                            let reg = (*registers)[i];
                            load_arg_to_reg(*args.items.add(i), reg, output);
                        }
                        sb_appendf(output, c!("    mov al, 0\n")); // x86_64 Linux ABI passes the amount of
                                                                   // floating point args via al. Since B
                                                                   // does not distinguish regular and
                                                                   // variadic functions we set al to 0 just
                                                                   // in case.
                        sb_appendf(output, c!("    call _%s\n"), name);
                    }
                    Os::Windows => {
                        let mut i = 0;
                        while i < cmp::min(args.count, registers.len()) {
                            let reg = (*registers)[i];
                            load_arg_to_reg(*args.items.add(i), reg, output);
                            i += 1;
                        }

                        // args on the stack are push right to left
                        // so we need to iterate them in reverse
                        for j in (i..args.count).rev() {
                            load_arg_to_reg(*args.items.add(j), c!("rax"), output);
                            sb_appendf(output, c!("    push rax\n"));
                        }

                        // allocate 32 bytes for "shadow space"
                        // it must be allocated at the top of the stack after all arguments are pushed
                        // so we can't allocate it at function prologue
                        sb_appendf(output, c!("    sub rsp, 32\n"));
                        sb_appendf(output, c!("    call _%s\n"), name);
                        sb_appendf(output, c!("    add rsp, %zu\n"), (args.count-i)*8+32); // deallocate stack args & "shadow space"
                    }
                }
                sb_appendf(output, c!("    mov [rbp-%zu], rax\n"), result*8);
            },
            Op::Asm {args} => {
                for i in 0..args.count {
                    let arg = *args.items.add(i);
                    sb_appendf(output, c!("    %s\n"), arg);
                }
            }
            Op::JmpIfNot{addr, arg} => {
                load_arg_to_reg(arg, c!("rax"), output);
                sb_appendf(output, c!("    test rax, rax\n"));
                sb_appendf(output, c!("    jz .op_%zu\n"), addr);
            },
            Op::Jmp{addr} => {
                sb_appendf(output, c!("    jmp .op_%zu\n"), addr);
            },
        }
    }
    sb_appendf(output, c!(".op_%zu:\n"), body.len());
    sb_appendf(output, c!("    mov rax, 0\n"));
    sb_appendf(output, c!("    mov rsp, rbp\n"));
    sb_appendf(output, c!("    pop rbp\n"));
    sb_appendf(output, c!("    ret\n"));
}

pub unsafe fn generate_funcs(output: *mut String_Builder, funcs: *const [Func], os: Os) {
    sb_appendf(output, c!("section \".text\" executable\n"));
    for i in 0..funcs.len() {
        generate_function((*funcs)[i].name, (*funcs)[i].name_loc, (*funcs)[i].params_count, (*funcs)[i].auto_vars_count, da_slice((*funcs)[i].body), output, os);
    }
}

pub unsafe fn generate_extrns(output: *mut String_Builder, extrns: *const [*const c_char], funcs: *const [Func], globals: *const [*const c_char]) {
    'skip: for i in 0..extrns.len() {
        let name = (*extrns)[i];

        for j in 0..funcs.len() {
            let func = (*funcs)[j];
            if strcmp(func.name, name) == 0 {
                continue 'skip
            }
        }

        for j in 0..globals.len() {
            let global = (*globals)[j];
            if strcmp(global, name) == 0 {
                continue 'skip
            }
        }

        sb_appendf(output, c!("extrn '%s' as _%s\n"), name, name);
    }
}

pub unsafe fn generate_globals(output: *mut String_Builder, globals: *const [*const c_char]) {
    for i in 0..globals.len() {
        let name = (*globals)[i];
        sb_appendf(output, c!("public _%s as '%s'\n"), name, name);
        sb_appendf(output, c!("_%s: rq 1\n"), name);
    }
}

pub unsafe fn generate_data_section(output: *mut String_Builder, data: *const [u8]) {
    if data.len() > 0 {
        sb_appendf(output, c!("section \".data\"\n"));
        sb_appendf(output, c!("dat: db "));
        for i in 0..data.len() {
            if i > 0 {
                sb_appendf(output, c!(","));
            }
            sb_appendf(output, c!("0x%02X"), (*data)[i] as c_uint);
        }
        sb_appendf(output, c!("\n"));
    }
}

pub unsafe fn generate_program(output: *mut String_Builder, c: *const Compiler, os: Os) {
    match os {
        Os::Linux   => sb_appendf(output, c!("format ELF64\n")),
        Os::Windows => sb_appendf(output, c!("format MS64 COFF\n")),
    };
    generate_funcs(output, da_slice((*c).funcs), os);
    generate_extrns(output, da_slice((*c).extrns), da_slice((*c).funcs), da_slice((*c).globals));
    generate_data_section(output, da_slice((*c).data));
    generate_globals(output, da_slice((*c).globals));
}
