macro_rules! codegens {
    ($($codegen:ident,)*) => {
        pub unsafe fn load_targets() -> Option<crate::nob::Array<crate::targets::Target>> {
            use crate::crust::libc::*;
            use crate::nob::*;
            use crate::targets::*;
            use core::mem::zeroed;
            use core::ffi::*;

            let mut targets: Array<Target> = zeroed();
            let mut apis: Array<TargetAPI> = zeroed();

            $(
                crate::codegen::$codegen::get_apis(&mut apis);
                register_apis(&mut targets, da_slice(apis), c!(stringify!($codegen)))?;
                apis.count = 0;
            )*

            free(apis.items as _);
            Some(targets)
        }

        $(pub mod $codegen;)*
    };
}

codegens! {
    // TODO: maybe instead of gas_ the prefix should be gnu_, 'cause that makes more sense.
    gas_x86_64,
    gas_aarch64,
    uxn,
    mos6502,
    ilasm_mono,
    noop,
}
