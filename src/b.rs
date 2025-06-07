#![no_main]
#![no_std]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_macros)]

#[macro_use]
pub mod nob;
#[macro_use]
pub mod flag;
#[macro_use]
pub mod crust;
pub mod arena;
pub mod codegen;
pub mod lexer;

use core::ffi::*;
use core::mem::zeroed;
use core::ptr;
use core::slice;
use nob::*;
use flag::*;
use crust::libc::*;
use arena::Arena;
use codegen::{Target, name_of_target, TARGET_NAMES, target_by_name, target_word_size};
use lexer::{Lexer, Loc, Token};

pub unsafe fn expect_tokens(l: *mut Lexer, tokens: *const [Token]) -> Option<()> {
    for i in 0..tokens.len() {
        if (*tokens)[i] == (*l).token {
            return Some(());
        }
    }

    let mut sb: String_Builder = zeroed();
    for i in 0..tokens.len() {
        if i > 0 {
            if i + 1 >= tokens.len() {
                sb_appendf(&mut sb, c!(", or "));
            } else {
                sb_appendf(&mut sb, c!(", "));
            }
        }
        sb_appendf(&mut sb, c!("%s"), lexer::display_token((*tokens)[i]));
    }
    da_append(&mut sb, 0);

    diagf!((*l).loc, c!("ERROR: expected %s, but got %s\n"), sb.items, lexer::display_token((*l).token));

    free(sb.items);
    None
}

pub unsafe fn expect_token(l: *mut Lexer, token: Token) -> Option<()> {
    expect_tokens(l, &[token])
}

pub unsafe fn get_and_expect_token(l: *mut Lexer, token: Token) -> Option<()> {
    lexer::get_token(l)?;
    expect_token(l, token)
}

pub unsafe fn get_and_expect_tokens(l: *mut Lexer, clexes: *const [Token]) -> Option<()> {
    lexer::get_token(l)?;
    expect_tokens(l, clexes)
}

pub unsafe fn expect_token_id(l: *mut Lexer, id: *const c_char) -> Option<()> {
    expect_token(l, Token::ID)?;
    if strcmp((*l).string, id) != 0 {
        diagf!((*l).loc, c!("ERROR: expected `%s`, but got `%s`\n"), id, (*l).string);
        return None;
    }
    Some(())
}

pub unsafe fn get_and_expect_token_id(l: *mut Lexer, id: *const c_char) -> Option<()> {
    lexer::get_token(l)?;
    expect_token_id(l, id)
}

#[repr(C)]
#[derive(Clone, Copy)]
pub enum Storage {
    External {name: *const c_char},
    Auto     {index: usize},
}

#[derive(Clone, Copy)]
pub struct Var {
    pub name: *const c_char,
    pub loc: Loc,
    pub storage: Storage,
}

pub unsafe fn scope_push(vars: *mut Array<Array<Var>>) {
    if (*vars).count < (*vars).capacity {
        // Reusing already allocated scopes
        (*vars).count += 1;
        (*da_last_mut(vars).expect("There should be always at least the global scope")).count = 0;
    } else {
        da_append(vars, zeroed());
    }
}

pub unsafe fn scope_pop(vars: *mut Array<Array<Var>>) {
    assert!((*vars).count > 0);
    (*vars).count -= 1;
}

pub unsafe fn find_var_near(vars: *const Array<Var>, name: *const c_char) -> *const Var {
    for i in 0..(*vars).count {
        let var = (*vars).items.add(i);
        if strcmp((*var).name, name) == 0 {
            return var
        }
    }
    ptr::null()
}

pub unsafe fn find_var_deep(vars: *const Array<Array<Var>>, name: *const c_char) -> *const Var {
    let mut i = (*vars).count;
    while i > 0 {
        let var = find_var_near((*vars).items.add(i-1), name);
        if !var.is_null() {
            return var;
        }
        i -= 1;
    }
    ptr::null()
}

// TODO: Don't need lexer as input anymore if we just use the provided loc for diagf!
pub unsafe fn declare_var(l: *mut Lexer, vars: *mut Array<Array<Var>>, name: *const c_char, loc: Loc, storage: Storage) -> Option<()> {
    let scope = da_last_mut(vars).expect("There should be always at least the global scope");
    let existing_var = find_var_near(scope, name);
    if !existing_var.is_null() {
        diagf!((*l).loc, c!("ERROR: redefinition of variable `%s`\n"), name);
        diagf!((*existing_var).loc, c!("NOTE: the first declaration is located here\n"));
        return None;
    }

    da_append(scope, Var {name, loc, storage});
    Some(())
}

#[derive(Clone, Copy)]
pub struct Label {
    name: *const c_char,
    loc: Loc,
    addr: usize,
}

pub unsafe fn find_label(labels: *const Array<Label>, name: *const c_char) -> *const Label {
    for i in 0..(*labels).count {
        let label = (*labels).items.add(i);
        if strcmp((*label).name, name) == 0 {
            return label
        }
    }
    ptr::null()
}

pub unsafe fn define_label(labels: *mut Array<Label>, name: *const c_char, loc: Loc, addr: usize) -> Option<()> {
    let existing_label = find_label(labels, name);
    if !existing_label.is_null() {
        diagf!(loc, c!("ERROR: duplicate label `%s`\n"), name);
        diagf!((*existing_label).loc, c!("NOTE: the first definition is located here\n"));
        return None;
    }

    da_append(labels, Label {name, loc, addr});
    Some(())
}

#[derive(Clone, Copy)]
pub enum Arg {
    AutoVar(usize),
    Deref(usize),
    /// Reference to the autovar with the specified index
    ///
    /// The autovars are currently expected to be layed out in memory from right to left,
    /// which is not particularly historically accurate.
    /// See TODO(2025-06-05 17:45:36)
    RefAutoVar(usize),
    RefExternal(*const c_char),
    External(*const c_char),
    Literal(u64),
    DataOffset(usize),
}

#[derive(Clone, Copy, PartialEq)]
pub enum Binop {
    Plus,
    Minus,
    Mult,
    Mod,
    Div,
    Less,
    Greater,
    Equal,
    NotEqual,
    GreaterEqual,
    LessEqual,
    BitOr,
    BitAnd,
    BitShl,
    BitShr,
}

// The higher the index of the row in this table the higher the precedence of the Binop
pub const PRECEDENCE: *const [*const [Binop]] = &[
    &[Binop::BitOr],
    &[Binop::BitAnd],
    &[Binop::BitShl, Binop::BitShr],
    &[Binop::Equal, Binop::NotEqual],
    &[Binop::Less, Binop::Greater, Binop::GreaterEqual, Binop::LessEqual],
    &[Binop::Plus, Binop::Minus],
    &[Binop::Mult, Binop::Mod, Binop::Div],
];

impl Binop {
    // The outer Option indicates success.
    // The inner Option indicates whether the assign has binop associated with it.
    // It's kinda confusing but I don't know how to make it "prettier"
    pub fn from_assign_token(token: Token) -> Option<Option<Self>> {
        match token {
            Token::Eq      => Some(None),
            Token::ShlEq   => Some(Some(Binop::BitShl)),
            Token::ShrEq   => Some(Some(Binop::BitShr)),
            Token::ModEq   => Some(Some(Binop::Mod)),
            Token::OrEq    => Some(Some(Binop::BitOr)),
            Token::AndEq   => Some(Some(Binop::BitAnd)),
            Token::PlusEq  => Some(Some(Binop::Plus)),
            Token::MinusEq => Some(Some(Binop::Minus)),
            Token::MulEq   => Some(Some(Binop::Mult)),
            Token::DivEq   => Some(Some(Binop::Div)),
            _              => None,
        }
    }

    pub fn from_token(token: Token) -> Option<Self> {
        match token {
            Token::Plus      => Some(Binop::Plus),
            Token::Minus     => Some(Binop::Minus),
            Token::Mul       => Some(Binop::Mult),
            Token::Div       => Some(Binop::Div),
            Token::Mod       => Some(Binop::Mod),
            Token::Less      => Some(Binop::Less),
            Token::Greater   => Some(Binop::Greater),
            Token::GreaterEq => Some(Binop::GreaterEqual),
            Token::LessEq    => Some(Binop::LessEqual),
            Token::Or        => Some(Binop::BitOr),
            Token::And       => Some(Binop::BitAnd),
            Token::Shl       => Some(Binop::BitShl),
            Token::Shr       => Some(Binop::BitShr),
            Token::EqEq      => Some(Binop::Equal),
            Token::NotEq     => Some(Binop::NotEqual),
            _ => None,
        }
    }

    pub const MAX_PRECEDENCE: usize = PRECEDENCE.len();
    pub unsafe fn precedence(self) -> usize {
        for precedence in 0..PRECEDENCE.len() {
            for i in 0..(*PRECEDENCE)[precedence].len() {
                if self == (*(*PRECEDENCE)[precedence])[i] {
                    return precedence
                }
            }
        }
        unreachable!()
    }
}

#[derive(Clone, Copy)]
pub enum Op {
    UnaryNot       {result: usize, arg: Arg},
    Negate         {result: usize, arg: Arg},
    Asm            {args: Array<*const c_char>},
    Binop          {binop: Binop, index: usize, lhs: Arg, rhs: Arg},
    AutoAssign     {index: usize, arg: Arg},
    ExternalAssign {name: *const c_char, arg: Arg},
    Store          {index: usize, arg: Arg},
    Funcall        {result: usize, fun: Arg, args: Array<Arg>},
    Jmp            {addr: usize},
    JmpIfNot       {addr: usize, arg: Arg},
    Return         {arg: Option<Arg>},
}

#[derive(Clone, Copy)]
pub struct OpWithLocation {
    pub opcode: Op,
    pub loc: Loc,
}

pub unsafe fn push_opcode(opcode: Op, loc: Loc, c: *mut Compiler) {
    da_append(&mut (*c).func_body, OpWithLocation {opcode, loc});
}

pub unsafe fn align_bytes(bytes: usize, alignment: usize) -> usize {
    let rem = bytes%alignment;
    if rem > 0 {
        bytes + alignment - rem
    } else {
        bytes
    }
}

/// Allocator of Auto Vars
#[derive(Clone, Copy)]
pub struct AutoVarsAtor {
    /// How many autovars currently allocated
    pub count: usize,
    /// Maximum allocated autovars throughout the function body
    pub max: usize,
}

pub unsafe fn allocate_auto_var(t: *mut AutoVarsAtor) -> usize {
    (*t).count += 1;
    if (*t).count > (*t).max {
        (*t).max = (*t).count;
    }
    (*t).count
}


pub unsafe fn compile_string(l: *mut Lexer, c: *mut Compiler) -> usize {
     // TODO: communicate this assumption to the caller of the function
    assert!((*l).token == Token::String);

    let offset = (*c).data.count;
    let string_len = strlen((*l).string);
    da_append_many(&mut (*c).data, slice::from_raw_parts((*l).string as *const u8, string_len));
    // TODO: Strings in B are not NULL-terminated.
    // They are terminated with symbol '*e' ('*' is escape character akin to '\' in C) which according to the
    // spec is called just "end-of-file" without any elaboration on what its value is. Maybe it had a specific
    // value on PDP that was a common knowledge at the time? In any case that breaks compatibility with
    // libc. While the language is still in development we gonna terminate it with 0. We will make it
    // "spec complaint" later.
    da_append(&mut (*c).data, 0); // NULL-terminator
    offset
}

pub unsafe fn compile_primary_expression(l: *mut Lexer, c: *mut Compiler) -> Option<(Arg, bool)> {
    lexer::get_token(l)?;
    let arg = match (*l).token {
        Token::OParen => {
            let result = compile_expression(l, c)?;
            get_and_expect_token(l, Token::CParen)?;
            Some(result)
        }
        Token::Not => {
            let (arg, _) = compile_primary_expression(l, c)?;
            let result = allocate_auto_var(&mut (*c).auto_vars_ator);
            push_opcode(Op::UnaryNot{result, arg}, (*l).loc, c);
            Some((Arg::AutoVar(result), false))
        }
        Token::Mul => {
            let (arg, _) = compile_primary_expression(l, c)?;
            let index = allocate_auto_var(&mut (*c).auto_vars_ator);
            push_opcode(Op::AutoAssign {index, arg}, (*l).loc, c);
            Some((Arg::Deref(index), true))
        }
        Token::Minus => {
            let (arg, _) = compile_primary_expression(l, c)?;
            let index = allocate_auto_var(&mut (*c).auto_vars_ator);
            push_opcode(Op::Negate {result: index, arg}, (*l).loc, c);
            Some((Arg::AutoVar(index), false))
        }
        Token::And => {
            let loc = (*l).loc;
            let (arg, is_lvalue) = compile_primary_expression(l, c)?;

            if !is_lvalue {
                diagf!(loc, c!("ERROR: cannot take the address of an rvalue\n"));
                return None;
            }

            match arg {
                Arg::Deref(index)   =>  Some((Arg::AutoVar(index), false)), // "&*x is identically x"
                Arg::External(name) =>  Some((Arg::RefExternal(name), false)),
                Arg::AutoVar(index) =>  Some((Arg::RefAutoVar(index), false)),
                Arg::Literal(_) | Arg::DataOffset(_) | Arg::RefAutoVar(_) | Arg::RefExternal(_) => unreachable!(),
            }
        }
        Token::PlusPlus => {
            let loc = (*l).loc;
            let (arg, is_lvalue) = compile_primary_expression(l, c)?;

            if !is_lvalue {
                diagf!(loc, c!("ERROR: cannot increment an rvalue\n"));
                return None;
            }

            compile_binop(arg, Arg::Literal(1), Binop::Plus, loc, c);
            Some((arg, false))
        }
        Token::MinusMinus => {
            let loc = (*l).loc;
            let (arg, is_lvalue) = compile_primary_expression(l, c)?;

            if !is_lvalue {
                diagf!(loc, c!("ERROR: cannot decrement an rvalue\n"));
                return None;
            }

            compile_binop(arg, Arg::Literal(1), Binop::Minus, loc, c);
            Some((arg, false))
        }
        Token::CharLit | Token::IntLit => Some((Arg::Literal((*l).int_number), false)),
        Token::ID => {
            let name = arena::strdup(&mut (*c).arena_names, (*l).string);

            let var_def = find_var_deep(&mut (*c).vars, name);
            if var_def.is_null() {
                diagf!((*l).loc, c!("ERROR: could not find name `%s`\n"), name);
                return None;
            }

            let saved_point = (*l).parse_point;
            lexer::get_token(l)?;

            (*l).parse_point = saved_point;
            match (*var_def).storage {
                Storage::Auto{index} => Some((Arg::AutoVar(index), true)),
                Storage::External{name} => Some((Arg::External(name), true)),
            }
        }
        Token::String => {
            let offset = compile_string(l, c);
            Some((Arg::DataOffset(offset), false))
        }
        _ => {
            diagf!((*l).loc, c!("Expected start of a primary expression but got %s\n"), lexer::display_token((*l).token));
            None
        }
    };

    let (mut arg, mut is_lvalue) = arg?;

    loop {
        let saved_point = (*l).parse_point;
        lexer::get_token(l)?;

        (arg, is_lvalue) = match (*l).token {
            Token::OParen => Some((compile_function_call(l, c, arg)?, false)),
            Token::OBracket => {
                let (offset, _) = compile_expression(l, c)?;
                get_and_expect_token(l, Token::CBracket)?;

                let result = allocate_auto_var(&mut (*c).auto_vars_ator);
                let word_size = Arg::Literal(target_word_size((*c).target));
                push_opcode(Op::Binop {binop: Binop::Mult, index: result, lhs: offset, rhs: word_size}, (*l).loc, c);
                push_opcode(Op::Binop {binop: Binop::Plus, index: result, lhs: arg, rhs: Arg::AutoVar(result)}, (*l).loc, c);

                Some((Arg::Deref(result), true))
            }
            Token::PlusPlus => {
                let loc = (*l).loc;
                if !is_lvalue {
                    diagf!(loc, c!("ERROR: cannot increment an rvalue\n"));
                    return None;
                }

                let pre = allocate_auto_var(&mut (*c).auto_vars_ator);
                push_opcode(Op::AutoAssign {index: pre, arg}, loc, c);
                compile_binop(arg, Arg::Literal(1), Binop::Plus, loc, c);

                Some((Arg::AutoVar(pre), false))
            }
            Token::MinusMinus => {
                let loc = (*l).loc;
                if !is_lvalue {
                    diagf!(loc, c!("ERROR: cannot decrement an rvalue\n"));
                    return None;
                }

                let pre = allocate_auto_var(&mut (*c).auto_vars_ator);
                push_opcode(Op::AutoAssign {index: pre, arg}, loc, c);
                compile_binop(arg, Arg::Literal(1), Binop::Minus, loc, c);

                Some((Arg::AutoVar(pre), false))
            }
            _ => {
                (*l).parse_point = saved_point;
                return Some((arg, is_lvalue));
            }
        }?;
    }
}

// TODO: communicate to the caller of this function that it expects `lhs` to be an lvalue
pub unsafe fn compile_binop(lhs: Arg, rhs: Arg, binop: Binop, loc: Loc, c: *mut Compiler) {
    match lhs {
        Arg::Deref(index) => {
            let tmp = allocate_auto_var(&mut (*c).auto_vars_ator);
            push_opcode(Op::Binop {binop, index: tmp, lhs, rhs}, loc, c);
            push_opcode(Op::Store {index, arg: Arg::AutoVar(tmp)}, loc, c);
        },
        Arg::External(name) => {
            let tmp = allocate_auto_var(&mut (*c).auto_vars_ator);
            push_opcode(Op::Binop {binop, index: tmp, lhs, rhs}, loc, c);
            push_opcode(Op::ExternalAssign {name, arg: Arg::AutoVar(tmp)}, loc, c)
        }
        Arg::AutoVar(index) => {
            push_opcode(Op::Binop {binop, index, lhs, rhs}, loc, c)
        }
        Arg::Literal(_) | Arg::DataOffset(_) | Arg::RefAutoVar(_) | Arg::RefExternal(_) => unreachable!(),
    }
}

pub unsafe fn compile_binop_expression(l: *mut Lexer, c: *mut Compiler, precedence: usize) -> Option<(Arg, bool)> {
    if precedence >= Binop::MAX_PRECEDENCE {
        return compile_primary_expression(l, c);
    }

    let (mut lhs, mut lvalue) = compile_binop_expression(l, c, precedence + 1)?;

    let mut saved_point = (*l).parse_point;
    lexer::get_token(l)?;

    if let Some(binop) = Binop::from_token((*l).token) {
        if binop.precedence() == precedence {
            while let Some(binop) = Binop::from_token((*l).token) {
                if binop.precedence() != precedence { break; }

                let (rhs, _) = compile_binop_expression(l, c, precedence + 1)?;

                let index = allocate_auto_var(&mut (*c).auto_vars_ator);
                push_opcode(Op::Binop {binop, index, lhs, rhs}, (*l).loc, c);
                lhs = Arg::AutoVar(index);

                lvalue = false;

                saved_point = (*l).parse_point;
                lexer::get_token(l)?;
            }
        }
    }

    (*l).parse_point = saved_point;
    Some((lhs, lvalue))
}

pub unsafe fn compile_assign_expression(l: *mut Lexer, c: *mut Compiler) -> Option<(Arg, bool)> {
    let (lhs, mut lvalue) = compile_binop_expression(l, c, 0)?;

    let mut saved_point = (*l).parse_point;
    lexer::get_token(l)?;

    while let Some(binop) = Binop::from_assign_token((*l).token) {
        let binop_loc = (*l).loc;
        let (rhs, _) = compile_assign_expression(l, c)?;

        if !lvalue {
            diagf!(binop_loc, c!("ERROR: cannot assign to rvalue\n"));
            return None;
        }

        if let Some(binop) = binop {
            compile_binop(lhs, rhs, binop, binop_loc, c);
        } else {
            match lhs {
                Arg::Deref(index) => {
                    push_opcode(Op::Store {index, arg: rhs}, binop_loc, c);
                }
                Arg::External(name) => {
                    push_opcode(Op::ExternalAssign {name, arg: rhs}, binop_loc, c);
                }
                Arg::AutoVar(index) => {
                    push_opcode(Op::AutoAssign {index, arg: rhs}, binop_loc, c);
                }
                Arg::Literal(_) | Arg::DataOffset(_) | Arg::RefAutoVar(_) | Arg::RefExternal(_) => unreachable!(),
            }
        }

        lvalue = false;

        saved_point = (*l).parse_point;
        lexer::get_token(l)?;
    }

    if (*l).token == Token::Question {
        let result = allocate_auto_var(&mut (*c).auto_vars_ator);

        let addr_condition = (*c).func_body.count;
        push_opcode(Op::JmpIfNot{addr: 0, arg: lhs}, (*l).loc, c);

        let (if_true, _) = compile_expression(l, c)?;
        push_opcode(Op::AutoAssign {index: result, arg: if_true}, (*l).loc, c);

        let addr_skips_true = (*c).func_body.count;
        push_opcode(Op::Jmp{addr: 0}, (*l).loc, c);

        let addr_false = (*c).func_body.count;
        get_and_expect_token(l, Token::Colon)?;

        let (if_false, _) = compile_expression(l, c)?;
        push_opcode(Op::AutoAssign {index: result, arg: if_false}, (*l).loc, c);

        let addr_after_false = (*c).func_body.count;
        (*(*c).func_body.items.add(addr_condition)).opcode  = Op::JmpIfNot {addr: addr_false, arg: lhs};
        (*(*c).func_body.items.add(addr_skips_true)).opcode = Op::Jmp      {addr: addr_after_false};

        Some((Arg::AutoVar(result), false))
    } else {
        (*l).parse_point = saved_point;
        Some((lhs, lvalue))
    }
}

pub unsafe fn compile_expression(l: *mut Lexer, c: *mut Compiler) -> Option<(Arg, bool)> {
    compile_assign_expression(l, c)
}

pub unsafe fn compile_block(l: *mut Lexer, c: *mut Compiler) -> Option<()> {
    loop {
        let saved_point = (*l).parse_point;
        lexer::get_token(l)?;
        if (*l).token == Token::CCurly { return Some(()); }
        (*l).parse_point = saved_point;

        compile_statement(l, c)?
    }
}
 unsafe fn compile_function_call(l: *mut Lexer, c: *mut Compiler, fun: Arg) -> Option<Arg> {
    let mut args: Array<Arg> = zeroed();
    let saved_point = (*l).parse_point;
    lexer::get_token(l)?;
    if (*l).token != Token::CParen {
        (*l).parse_point = saved_point;
        loop {
            let (expr, _) = compile_expression(l, c)?;
            da_append(&mut args, expr);
            get_and_expect_tokens(l, &[Token::CParen, Token::Comma])?;
            match (*l).token {
                Token::CParen => break,
                Token::Comma => continue,
                _ => unreachable!(),
            }
        }
    }

    let result = allocate_auto_var(&mut (*c).auto_vars_ator);
    push_opcode(Op::Funcall {result, fun, args}, (*l).loc, c);
    Some(Arg::AutoVar(result))
}

pub unsafe fn name_declare_if_not_exists(names: *mut Array<*const c_char>, name: *const c_char) {
    for i in 0..(*names).count {
        if strcmp(*(*names).items.add(i), name) == 0 {
            return;
        }
    }
    da_append(names, name)
}

pub unsafe fn backpatch_jmp(backpatch: *mut OpWithLocation, addr: usize) {
    match (*backpatch).opcode {
        Op::Jmp{..}           => (*backpatch).opcode = Op::Jmp { addr },
        Op::JmpIfNot{arg, ..} => (*backpatch).opcode = Op::JmpIfNot { addr, arg },
        _                     => unreachable!()
    }
}

pub unsafe fn compile_statement(l: *mut Lexer, c: *mut Compiler) -> Option<()> {
    let saved_point = (*l).parse_point;
    lexer::get_token(l)?;

    match (*l).token {
        Token::OCurly => {
            scope_push(&mut (*c).vars);
            let saved_auto_vars_count = (*c).auto_vars_ator.count;
            compile_block(l, c)?;
            (*c).auto_vars_ator.count = saved_auto_vars_count;
            scope_pop(&mut (*c).vars);
            Some(())
        }
        Token::Extrn => {
            while (*l).token != Token::SemiColon {
                get_and_expect_token(l, Token::ID)?;
                let name = arena::strdup(&mut (*c).arena_names, (*l).string);
                name_declare_if_not_exists(&mut (*c).extrns, name);
                declare_var(l, &mut (*c).vars, name, (*l).loc, Storage::External {name})?;
                get_and_expect_tokens(l, &[Token::SemiColon, Token::Comma])?;
            }
            Some(())
        }
        Token::Auto => {
            while (*l).token != Token::SemiColon {
                get_and_expect_token(l, Token::ID)?;
                // TODO: Automatic variable names should only need function lifetime.
                //   Could use .arena_labels here but naming would be confusing.
                //   Rename .arena_labels to indicate function lifetime first?
                let name = arena::strdup(&mut (*c).arena_names, (*l).string);
                let index = allocate_auto_var(&mut (*c).auto_vars_ator);
                declare_var(l, &mut (*c).vars, name, (*l).loc, Storage::Auto {index})?;
                get_and_expect_tokens(l, &[Token::SemiColon, Token::Comma, Token::IntLit, Token::CharLit])?;
                if (*l).token == Token::IntLit || (*l).token == Token::CharLit {
                    let size = (*l).int_number as usize;
                    if size == 0 {
                        missingf!((*l).loc, c!("It's unclear how to compile automatic vector of size 0\n"));
                    }
                    for _ in 0..size {
                        allocate_auto_var(&mut (*c).auto_vars_ator);
                    }
                    // TODO: Here we assume the stack grows down. Should we
                    //   instead find a way for the target to decide that?
                    //   See TODO(2025-06-05 17:45:36)
                    let arg = Arg::RefAutoVar(index + size);
                    push_opcode(Op::AutoAssign {index, arg}, (*l).loc, c);
                    get_and_expect_tokens(l, &[Token::SemiColon, Token::Comma])?;
                }
            }
            Some(())
        }
        Token::If => {
            get_and_expect_token(l, Token::OParen)?;
            let saved_auto_vars_count = (*c).auto_vars_ator.count;
            let (cond, _) = compile_expression(l, c)?;
            get_and_expect_token(l, Token::CParen)?;

            let addr_condition = (*c).func_body.count;
            push_opcode(Op::JmpIfNot{addr: 0, arg: cond}, (*l).loc, c);
            (*c).auto_vars_ator.count = saved_auto_vars_count;

            compile_statement(l, c)?;

            let saved_point = (*l).parse_point;
            lexer::get_token(l)?;

            if (*l).token == Token::Else {
                let addr_skips_else = (*c).func_body.count;
                push_opcode(Op::Jmp{addr: 0}, (*l).loc, c);
                let addr_else = (*c).func_body.count;
                compile_statement(l, c)?;
                let addr_after_else = (*c).func_body.count;
                (*(*c).func_body.items.add(addr_condition)).opcode  = Op::JmpIfNot {addr: addr_else, arg: cond};
                (*(*c).func_body.items.add(addr_skips_else)).opcode = Op::Jmp      {addr: addr_after_else};
            } else {
                (*l).parse_point = saved_point;
                let addr_after_if = (*c).func_body.count;
                (*(*c).func_body.items.add(addr_condition)).opcode  = Op::JmpIfNot {addr: addr_after_if , arg: cond};
            }

            Some(())
        }
        Token::While => {
            let begin = (*c).func_body.count;
            get_and_expect_token(l, Token::OParen)?;
            let saved_auto_vars_count = (*c).auto_vars_ator.count;
            let (arg, _) = compile_expression(l, c)?;

            get_and_expect_token(l, Token::CParen)?;
            let condition_jump = (*c).func_body.count;
            push_opcode(Op::JmpIfNot{addr: 0, arg}, (*l).loc, c);
            (*c).auto_vars_ator.count = saved_auto_vars_count;

            compile_statement(l, c)?;
            push_opcode(Op::Jmp{addr: begin}, (*l).loc, c);
            let end = (*c).func_body.count;
            (*(*c).func_body.items.add(condition_jump)).opcode = Op::JmpIfNot{addr: end, arg};
            Some(())
        }
        Token::Return => {
            get_and_expect_tokens(l, &[Token::SemiColon, Token::OParen])?;
            if (*l).token == Token::SemiColon {
                push_opcode(Op::Return {arg: None}, (*l).loc, c);
            } else if (*l).token == Token::OParen {
                let (arg, _) = compile_expression(l, c)?;
                get_and_expect_token(l, Token::CParen)?;
                get_and_expect_token(l, Token::SemiColon)?;
                push_opcode(Op::Return {arg: Some(arg)}, (*l).loc, c);
            } else {
                unreachable!();
            }
            Some(())
        }
        Token::Goto => {
            get_and_expect_token(l, Token::ID)?;
            let name = arena::strdup(&mut (*c).arena_labels, (*l).string);
            let loc = (*l).loc;
            let addr = (*c).func_body.count;
            da_append(&mut (*c).func_labels_used, Label {name, loc, addr});
            get_and_expect_token(l, Token::SemiColon)?;
            push_opcode(Op::Jmp {addr: 0}, (*l).loc, c);
            Some(())
        }
        Token::Asm => {
            get_and_expect_token(l, Token::OParen)?;

            let mut args: Array<*const c_char> = zeroed();

            while (*l).token != Token::CParen {
                lexer::get_token(l)?;
                match (*l).token {
                    Token::String => {
                        da_append(&mut args, strdup((*l).string));
                    }
                    _ => {
                        diagf!((*l).loc, c!("ERROR: %s only takes strings"), (*l).string);
                    }
                }

                get_and_expect_tokens(l, &[Token::Comma, Token::CParen])?;
            }
            get_and_expect_token(l, Token::SemiColon)?;

            push_opcode(Op::Asm {args}, (*l).loc, c);
            Some(())
        }
        Token::Case => {
            let Some(switch_frame) = da_last_mut(&mut (*c).switch_stack) else {
                diagf!((*l).loc, c!("case label outside of switch\n"));
                return None;
            };

            let case_loc = (*l).loc;
            lexer::get_token(l);
            expect_tokens(l, &[Token::IntLit, Token::CharLit])?; // TODO: String ??!
            let case_value = (*l).int_number;
            get_and_expect_token(l, Token::Colon)?;

            let addr = (*c).func_body.count;
            push_opcode(Op::Jmp{addr: addr + 3}, case_loc, c);
            push_opcode(Op::Binop{
                binop: Binop::Equal,
                index: (*switch_frame).cond,
                lhs: (*switch_frame).value,
                rhs: Arg::Literal(case_value)
            }, case_loc, c);
            push_opcode(Op::JmpIfNot {
                addr: 0,
                arg: Arg::AutoVar((*switch_frame).cond)
            }, case_loc, c);

            let backpatch = (*c).func_body.items.add((*switch_frame).jmp_addr);
            backpatch_jmp(backpatch, addr + 1);
            (*switch_frame).jmp_addr = addr + 2;

            Some(())
        }
        Token::Switch => {
            let saved_auto_vars_count = (*c).auto_vars_ator.count;

            let switch_loc = (*l).loc;
            let (value, _) = compile_expression(l, c)?;
            let cond = allocate_auto_var(&mut (*c).auto_vars_ator);
            let jmp_addr = (*c).func_body.count;
            da_append(&mut (*c).switch_stack, Switch {jmp_addr, value, cond});
            push_opcode(Op::Jmp {addr: 0}, switch_loc, c);
            compile_statement(l, c)?;

            let switch_frame = da_last_mut(&mut (*c).switch_stack).expect("Switch stack was modified by somebody else");
            let backpatch = (*c).func_body.items.add((*switch_frame).jmp_addr);
            backpatch_jmp(backpatch, (*c).func_body.count);
            (*c).switch_stack.count -= 1;

            (*c).auto_vars_ator.count = saved_auto_vars_count;

            Some(())
        }
        _ => {
            if (*l).token == Token::ID {
                let name = arena::strdup(&mut (*c).arena_labels, (*l).string);
                let name_loc = (*l).loc;
                let addr = (*c).func_body.count;
                lexer::get_token(l)?;
                if (*l).token == Token::Colon {
                    define_label(&mut (*c).func_labels, name, name_loc, addr)?;
                    return Some(());
                }
            }
            (*l).parse_point = saved_point;
            let saved_auto_vars_count = (*c).auto_vars_ator.count;
            compile_expression(l, c)?;
            (*c).auto_vars_ator.count = saved_auto_vars_count;
            get_and_expect_token(l, Token::SemiColon)?;
            Some(())
        }
    }
}

pub unsafe fn temp_strip_suffix(s: *const c_char, suffix: *const c_char) -> Option<*const c_char> {
    let mut sv = sv_from_cstr(s);
    let suffix_len = strlen(suffix);
    if sv_end_with(sv, suffix) {
        sv.count -= suffix_len;
        Some(temp_sv_to_cstr(sv))
    } else {
        None
    }
}

pub unsafe fn usage() {
    fprintf(stderr(), c!("Usage: %s [OPTIONS] <inputs...> [--] [run arguments]\n"), flag_program_name());
    fprintf(stderr(), c!("OPTIONS:\n"));
    flag_print_options(stderr());
}

#[derive(Clone, Copy)]
pub struct Func {
    name: *const c_char,
    name_loc: Loc,
    body: Array<OpWithLocation>,
    params_count: usize,
    auto_vars_count: usize,
}

#[derive(Clone, Copy)]
pub struct Global {
    name: *const c_char,
    values: Array<ImmediateValue>,
    is_vec: bool,
    minimum_size: usize,
}

#[derive(Clone, Copy)]
pub enum ImmediateValue {
    Name(*const c_char),
    Literal(u64),
    DataOffset(usize),
}

#[derive(Clone, Copy)]
pub struct Switch {
    pub jmp_addr: usize,
    pub value: Arg,
    pub cond: usize,
}

#[derive(Clone, Copy)]
pub struct Compiler {
    pub vars: Array<Array<Var>>,
    pub auto_vars_ator: AutoVarsAtor,
    pub funcs: Array<Func>,
    pub func_body: Array<OpWithLocation>,
    pub func_labels: Array<Label>,
    pub func_labels_used: Array<Label>,
    pub switch_stack: Array<Switch>,
    pub data: Array<u8>,
    pub extrns: Array<*const c_char>,
    pub globals: Array<Global>,
    pub arena_names: Arena,
    pub arena_labels: Arena,
    pub target: Target,
}

pub unsafe fn compile_program(l: *mut Lexer, c: *mut Compiler) -> Option<()> {
    'def: loop {
        lexer::get_token(l)?;
        if (*l).token == Token::EOF { break 'def }

        expect_token(l, Token::ID)?;

        let name = arena::strdup(&mut (*c).arena_names, (*l).string);
        let name_loc = (*l).loc;

        let saved_point = (*l).parse_point;
        lexer::get_token(l)?;

        if (*l).token == Token::OParen { // Function definition
            declare_var(l, &mut (*c).vars, name, name_loc, Storage::External{name})?;
            scope_push(&mut (*c).vars); // begin function scope
            let mut params_count = 0;
            let saved_point = (*l).parse_point;
            lexer::get_token(l)?;
            if (*l).token != Token::CParen {
                (*l).parse_point = saved_point;
                'params: loop {
                    get_and_expect_token(l, Token::ID)?;
                    let name = arena::strdup(&mut (*c).arena_names, (*l).string);
                    let name_loc = (*l).loc;
                    let index = allocate_auto_var(&mut (*c).auto_vars_ator);
                    declare_var(l, &mut (*c).vars, name, name_loc, Storage::Auto{index})?;
                    params_count += 1;
                    get_and_expect_tokens(l, &[Token::CParen, Token::Comma])?;
                    match (*l).token {
                        Token::CParen => break 'params,
                        Token::Comma => continue 'params,
                        _ => unreachable!(),
                    }
                }
            }
            compile_statement(l, c)?;
            scope_pop(&mut (*c).vars); // end function scope

            for i in 0..(*c).func_labels_used.count {
                let used_label = *(*c).func_labels_used.items.add(i);
                let existing_label = find_label(&(*c).func_labels, used_label.name);
                if existing_label.is_null() {
                    diagf!(used_label.loc, c!("ERROR: label `%s` used but not defined\n"), used_label.name);
                    return None;
                }
                (*(*c).func_body.items.add(used_label.addr)).opcode = Op::Jmp {addr: (*existing_label).addr};
            }
            arena::reset(&mut (*c).arena_labels);

            da_append(&mut (*c).funcs, Func {
                name,
                name_loc,
                body: (*c).func_body,
                params_count,
                auto_vars_count: (*c).auto_vars_ator.max,
            });
            (*c).func_body = zeroed();
            (*c).func_labels.count = 0;
            (*c).func_labels_used.count = 0;
            (*c).auto_vars_ator = zeroed();
        } else { // Variable definition
            (*l).parse_point = saved_point;
            declare_var(l, &mut (*c).vars, name, name_loc, Storage::External{name})?;

            let mut global = Global {
                name,
                values: zeroed(),
                is_vec: false,
                minimum_size: 0,
            };

            // TODO: This code is ugly
            // couldn't find a better way to write it while keeping accurate error messages
            get_and_expect_tokens(l, &[Token::IntLit, Token::CharLit, Token::String, Token::ID, Token::SemiColon, Token::OBracket])?;

            if (*l).token == Token::OBracket {
                global.is_vec = true;
                get_and_expect_tokens(l, &[Token::IntLit, Token::CBracket])?;
                if (*l).token == Token::IntLit {
                    global.minimum_size = (*l).int_number as usize;
                    get_and_expect_token(l, Token::CBracket)?;
                }
                get_and_expect_tokens(l, &[Token::IntLit, Token::CharLit, Token::String, Token::ID, Token::SemiColon])?;
            }

            while (*l).token != Token::SemiColon {
                let value = match (*l).token {
                    Token::IntLit | Token::CharLit => ImmediateValue::Literal((*l).int_number),
                    Token::String => ImmediateValue::DataOffset(compile_string(l, c)),
                    Token::ID => {
                        let name = arena::strdup(&mut (*c).arena_names, (*l).string);
                        let scope = da_last_mut(&mut (*c).vars).expect("There should be always at least the global scope");
                        let var = find_var_near(scope, name);
                        if var.is_null() {
                            diagf!((*l).loc, c!("ERROR: could not find name `%s`\n"), name);
                            return None;
                        }
                        ImmediateValue::Name(name)
                    }
                    _ => unreachable!()
                };
                da_append(&mut global.values, value);

                get_and_expect_tokens(l, &[Token::SemiColon, Token::Comma])?;
                if (*l).token == Token::Comma {
                    get_and_expect_tokens(l, &[Token::IntLit, Token::CharLit, Token::String, Token::ID])?;
                } else {
                    break;
                }
            }

            if !global.is_vec && global.values.count == 0 {
                da_append(&mut global.values, ImmediateValue::Literal(0));
            }
            da_append(&mut (*c).globals, global)

        }
    }

    Some(())
}

pub unsafe fn main(mut argc: i32, mut argv: *mut*mut c_char) -> Option<()> {
    let default_target;
    if cfg!(target_arch = "aarch64") && cfg!(target_os = "linux") {
        default_target = Some(Target::Gas_AArch64_Linux);
    } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "linux") {
        default_target = Some(Target::Fasm_x86_64_Linux);
    } else if cfg!(target_arch = "x86_64") && cfg!(target_os = "windows") {
        default_target = Some(Target::Fasm_x86_64_Windows);
    } else {
        default_target = None;
    }

    let default_target_name = if let Some(default_target) = default_target {
        name_of_target(default_target).expect("default target name not found")
    } else {
        ptr::null()
    };

    let target_name = flag_str(c!("t"), default_target_name, c!("Compilation target. Pass \"list\" to get the list of available targets."));
    let output_path = flag_str(c!("o"), ptr::null(), c!("Output path"));
    let run         = flag_bool(c!("run"), false, c!("Run the compiled program (if applicable for the target)"));
    let help        = flag_bool(c!("help"), false, c!("Print this help message"));
    let linker      = flag_list(c!("L"), c!("Append a flag to the linker of the target platform"));

    let mut input_paths: Array<*mut c_char> = zeroed();
    let mut run_args: Array<*mut c_char> = zeroed();
    'args: while argc > 0 {
        if !flag_parse(argc, argv) {
            usage();
            flag_print_error(stderr());
            return None;
        }
        argc = flag_rest_argc();
        argv = flag_rest_argv();
        if argc > 0 {
            if strcmp(*argv, c!("--")) == 0 {
                da_append_many(&mut run_args, slice::from_raw_parts_mut(argv.add(1), (argc - 1) as usize));
                break 'args;
            } else {
                da_append(&mut input_paths, shift!(argv, argc));
            }
        }
    }

    if *help {
        usage();
        return Some(());
    }

    if (*target_name).is_null() {
        usage();
        fprintf(stderr(), c!("ERROR: no value is provided for -%s flag."), flag_name(target_name));
        return None;
    }

    if strcmp(*target_name, c!("list")) == 0 {
        fprintf(stderr(), c!("Compilation targets:\n"));
        for i in 0..TARGET_NAMES.len() {
            fprintf(stderr(), c!("    %s\n"), (*TARGET_NAMES)[i].name);
        }
        return Some(());
    }

    if input_paths.count == 0 {
        usage();
        fprintf(stderr(), c!("ERROR: no input is provided\n"));
        return None;
    }

    let Some(target) = target_by_name(*target_name) else {
        usage();
        fprintf(stderr(), c!("ERROR: unknown target `%s`\n"), *target_name);
        return None;
    };

    if input_paths.count == 0 {
        usage();
        fprintf(stderr(), c!("ERROR: no inputs are provided\n"));
        return None;
    }

    let mut c: Compiler = zeroed();
    c.target = target;
    let mut input: String_Builder = zeroed();

    scope_push(&mut c.vars);          // begin global scope
    for i in 0..input_paths.count {
        let input_path = *input_paths.items.add(i);

        input.count = 0;
        if !read_entire_file(input_path, &mut input) { return None; }

        let mut l: Lexer = lexer::new(input_path, input.items, input.items.add(input.count));

        compile_program(&mut l, &mut c)?;
    }
    scope_pop(&mut c.vars);          // end global scope

    let mut output: String_Builder = zeroed();
    let mut cmd: Cmd = zeroed();
    match target {
        Target::Gas_AArch64_Linux => {
            codegen::gas_aarch64_linux::generate_program(&mut output, &c);

            let effective_output_path;
            if (*output_path).is_null() {
                if let Some(base_path) = temp_strip_suffix(*input_paths.items, c!(".b")) {
                    effective_output_path = base_path;
                } else {
                    effective_output_path = temp_sprintf(c!("%s.out"), *input_paths.items);
                }
            } else {
                effective_output_path = *output_path;
            }

            let output_asm_path = temp_sprintf(c!("%s.s"), effective_output_path);
            if !write_entire_file(output_asm_path, output.items as *const c_void, output.count) { return None; }
            printf(c!("Generated %s\n"), output_asm_path);

            if !(cfg!(target_arch = "aarch64") && cfg!(target_os = "linux")) {
                // TODO: think how to approach cross-compilation
                fprintf(stderr(), c!("ERROR: Cross-compilation of aarch64 linux is not supported for now\n"));
                return None;
            }

            let output_obj_path = temp_sprintf(c!("%s.o"), effective_output_path);
            cmd_append! {
                &mut cmd,
                c!("as"), c!("-o"), output_obj_path, output_asm_path,
            }
            if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            cmd_append! {
                &mut cmd,
                c!("cc"), c!("-no-pie"), c!("-o"), effective_output_path, output_obj_path,
            }
            for i in 0..(*linker).count {
                cmd_append!{
                    &mut cmd,
                    *(*linker).items.add(i),
                }
            }
            if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            if *run {
                // if the user does `b program.b -run` the compiler tries to run `program` which is not possible on Linux. It has to be `./program`.
                let run_path: *const c_char;
                if (strchr(effective_output_path, '/' as c_int)).is_null() {
                    run_path = temp_sprintf(c!("./%s"), effective_output_path);
                } else {
                    run_path = effective_output_path;
                }

                cmd_append! {
                    &mut cmd,
                    run_path,
                }

                for i in 0..run_args.count {
                    cmd_append! {
                        &mut cmd,
                        *(run_args).items.add(i),
                    }
                }

                if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            }
        },
        Target::Fasm_x86_64_Linux => {
            codegen::fasm_x86_64::generate_program(&mut output, &c, codegen::Os::Linux);

            let effective_output_path;
            if (*output_path).is_null() {
                if let Some(base_path) = temp_strip_suffix(*input_paths.items, c!(".b")) {
                    effective_output_path = base_path;
                } else {
                    effective_output_path = temp_sprintf(c!("%s.out"), *input_paths.items);
                }
            } else {
                effective_output_path = *output_path;
            }

            let output_asm_path = temp_sprintf(c!("%s.asm"), effective_output_path);
            if !write_entire_file(output_asm_path, output.items as *const c_void, output.count) { return None; }
            printf(c!("Generated %s\n"), output_asm_path);

            if !(cfg!(target_arch = "x86_64") && cfg!(target_os = "linux")) {
                // TODO: think how to approach cross-compilation
                fprintf(stderr(), c!("ERROR: Cross-compilation of x86_64 linux is not supported for now\n"));
                return None;
            }

            let output_obj_path = temp_sprintf(c!("%s.o"), effective_output_path);
            cmd_append! {
                &mut cmd,
                c!("fasm"), output_asm_path, output_obj_path,
            }
            if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            cmd_append! {
                &mut cmd,
                c!("cc"), c!("-no-pie"), c!("-o"), effective_output_path, output_obj_path,
            }
            for i in 0..(*linker).count {
                cmd_append!{
                    &mut cmd,
                    *(*linker).items.add(i),
                }
            }
            if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            if *run {
                // if the user does `b program.b -run` the compiler tries to run `program` which is not possible on Linux. It has to be `./program`.
                let run_path: *const c_char;
                if (strchr(effective_output_path, '/' as c_int)).is_null() {
                    run_path = temp_sprintf(c!("./%s"), effective_output_path);
                } else {
                    run_path = effective_output_path;
                }

                cmd_append! {
                    &mut cmd,
                    run_path,
                }

                for i in 0..run_args.count {
                    cmd_append! {
                        &mut cmd,
                        *(run_args).items.add(i),
                    }
                }

                if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            }
        }
        Target::Fasm_x86_64_Windows => {
            codegen::fasm_x86_64::generate_program(&mut output, &c, codegen::Os::Windows);

            let base_path;
            if (*output_path).is_null() {
                if let Some(path) = temp_strip_suffix(*input_paths.items, c!(".b")) {
                    base_path = path;
                } else {
                    base_path = *input_paths.items;
                }
            } else {
                if let Some(path) = temp_strip_suffix(*output_path, c!(".exe")) {
                    base_path = path;
                } else {
                    base_path = *output_path;
                }
            }

            let effective_output_path = temp_sprintf(c!("%s.exe"), base_path);

            let output_asm_path = temp_sprintf(c!("%s.asm"), base_path);
            if !write_entire_file(output_asm_path, output.items as *const c_void, output.count) { return None; }
            printf(c!("Generated %s\n"), output_asm_path);

            let cc = if cfg!(target_arch = "x86_64") && cfg!(target_os = "windows") {
                c!("cc")
            } else {
                c!("x86_64-w64-mingw32-gcc")
            };

            let output_obj_path = temp_sprintf(c!("%s.obj"), base_path);
            cmd_append! {
                &mut cmd,
                c!("fasm"), output_asm_path, output_obj_path,
            }
            if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            cmd_append! {
                &mut cmd,
                cc, c!("-no-pie"), c!("-o"), effective_output_path, output_obj_path,
            }
            for i in 0..(*linker).count {
                cmd_append!{
                    &mut cmd,
                    *(*linker).items.add(i),
                }
            }
            if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            if *run {
                if !cfg!(target_os = "windows") {
                    cmd_append! {
                        &mut cmd,
                        c!("wine"),
                    }
                }

                cmd_append! {
                    &mut cmd,
                    effective_output_path,
                }

                for i in 0..run_args.count {
                    cmd_append! {
                        &mut cmd,
                        *(run_args).items.add(i),
                    }
                }

                if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            }
        }
        Target::Uxn => {
            codegen::uxn::generate_program(&mut output, &c);

            let effective_output_path;
            if (*output_path).is_null() {
                let input_path = *input_paths.items;
                let base_path = temp_strip_suffix(input_path, c!(".b")).unwrap_or(input_path);
                effective_output_path = temp_sprintf(c!("%s.rom"), base_path);
            } else {
                effective_output_path = *output_path;
            }

            if !write_entire_file(effective_output_path, output.items as *const c_void, output.count) { return None; }
            printf(c!("Generated %s\n"), effective_output_path);
            if *run {
                cmd_append! {
                    &mut cmd,
                    c!("uxnemu"), effective_output_path,
                }
                for i in 0..run_args.count {
                    cmd_append! {
                        &mut cmd,
                        *(run_args).items.add(i),
                    }
                }
                if !cmd_run_sync_and_reset(&mut cmd) { return None; }
            }
        }
        Target::IR => {
            codegen::ir::generate_program(&mut output, &c);

            let effective_output_path;
            if (*output_path).is_null() {
                let input_path = *input_paths.items;
                let base_path = temp_strip_suffix(input_path, c!(".b")).unwrap_or(input_path);
                effective_output_path = temp_sprintf(c!("%s.ir"), base_path);
            } else {
                effective_output_path = *output_path;
            }

            if !write_entire_file(effective_output_path, output.items as *const c_void, output.count) { return None; }
            printf(c!("Generated %s\n"), effective_output_path);
            if *run {
                todo!("Interpret the IR?");
            }
        }
    }
    Some(())
}

// TODO: Continue compilation for as long as possible
//   even if you encounter semantical errors like unknown variables, functions, etc.
//   If we couldn't find some names just report the error, generate some bogus IR, and
//   continue compiling.
//
//   Every time an error occurs during the compilation we should increment some sort of error counter within
//   struct Compiler and at the end of the compilation if the counter is greater than 0, fail the compilation
//   without even trying to generate any assembly. We could also define some maximum error count after which
//   the compilation fails instantaneously because of "too many errors" (similar to how Go compiler does this).
//
//   This idea might be extended to syntactical errors like missing semicolons. We could
//   probably just assume the semicolons where we expect them, report the error and continue
//   parsing and compiling.
