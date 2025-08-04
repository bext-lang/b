//! Smallest possible codegen that compiles

use crate::targets::TargetAPI;
use crate::nob::Array;

// Compiler expects only get_apis()
pub unsafe fn get_apis(_targets: *mut Array<TargetAPI>) {
    // Export no APIs
}
