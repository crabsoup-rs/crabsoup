extern crate crabsoup_luau_sys;

use std::ffi::{c_char, c_uint, c_void};
use std::mem;

#[repr(C)]
struct RustString {
    data: *const c_char,
    len: usize,
}
impl RustString {
    unsafe fn decode(&self) -> String {
        let slice =
            std::str::from_utf8(std::slice::from_raw_parts(self.data as *const _, self.len))
                .expect("Received invalid UTF-8 from Luau?");
        slice.to_string()
    }
    unsafe fn encode(str: &str) -> Self {
        RustString { data: str.as_ptr() as *const _, len: str.len() }
    }
}

#[repr(C)]
struct RustLineColumn {
    line: c_uint,
    column: c_uint,
}

#[derive(Debug)]
struct RustCheckResultReceiver {
    results: Vec<AnalyzeResult>,
}

#[derive(Clone, Debug)]
pub struct AnalyzeResult {
    pub location: String,
    pub location_start: AnalyzeLocation,
    pub location_end: AnalyzeLocation,
    pub is_error: bool,
    pub is_lint: bool,
    pub message: String,
}

#[derive(Copy, Clone, Debug)]
pub struct AnalyzeLocation {
    pub line: usize,
    pub column: usize,
}
impl From<RustLineColumn> for AnalyzeLocation {
    fn from(value: RustLineColumn) -> Self {
        AnalyzeLocation { line: value.line as usize, column: value.column as usize }
    }
}

#[repr(transparent)]
struct FrontendWrapper(c_void);

extern "C-unwind" {
    fn luauAnalyze_new_frontend() -> *mut FrontendWrapper;
    fn luauAnalyze_register_definitions(
        wrapper: *mut FrontendWrapper,
        module_name: RustString,
        definitions: RustString,
    ) -> bool;
    fn luauAnalyze_set_deprecation(
        wrapper: *mut FrontendWrapper,
        module_path: RustString,
        replacement: RustString,
    ) -> bool;
    fn luauAnalyze_freeze_definitions(wrapper: *mut FrontendWrapper);
    #[allow(improper_ctypes)] // only used as an opaque reference
    fn luauAnalyze_check(
        receiver: *mut RustCheckResultReceiver,
        wrapper: *mut FrontendWrapper,
        name: RustString,
        contents: RustString,
        is_module: bool,
    );
    fn luauAnalyze_free_frontend(wrapper: *mut FrontendWrapper);
}

mod exports {
    use super::*;

    #[no_mangle]
    pub unsafe extern "C-unwind" fn luauAnalyze_push_result(
        receiver: *mut RustCheckResultReceiver,
        module: RustString,
        error_start: RustLineColumn,
        error_end: RustLineColumn,
        is_error: bool,
        is_lint: bool,
        message: RustString,
    ) {
        (*receiver).results.push(AnalyzeResult {
            location: module.decode(),
            location_start: error_start.into(),
            location_end: error_end.into(),
            is_error,
            is_lint,
            message: message.decode(),
        });
    }
}

pub struct LuaAnalyzerBuilder {
    underlying: *mut FrontendWrapper,
}
impl LuaAnalyzerBuilder {
    pub fn new() -> Self {
        LuaAnalyzerBuilder { underlying: unsafe { luauAnalyze_new_frontend() } }
    }

    pub fn add_definitions(&mut self, name: &str, definitions: &str) {
        if !unsafe {
            luauAnalyze_register_definitions(
                self.underlying,
                RustString::encode(name),
                RustString::encode(definitions),
            )
        } {
            panic!("Failed to parse definitions file: {name}");
        }
    }

    pub fn set_deprecation(&mut self, name: &str, replacement: Option<&str>) {
        let replacement = replacement.unwrap_or("");
        if !unsafe {
            luauAnalyze_set_deprecation(
                self.underlying,
                RustString::encode(name),
                RustString::encode(replacement),
            )
        } {
            panic!("set_deprecation failed");
        }
    }

    pub fn build(self) -> LuaAnalyzer {
        unsafe { luauAnalyze_freeze_definitions(self.underlying) }
        let out = LuaAnalyzer { underlying: self.underlying };
        mem::forget(self);
        out
    }
}
impl Drop for LuaAnalyzerBuilder {
    fn drop(&mut self) {
        unsafe { luauAnalyze_free_frontend(self.underlying) }
    }
}

pub struct LuaAnalyzer {
    underlying: *mut FrontendWrapper,
}
impl LuaAnalyzer {
    pub fn check(&self, name: &str, contents: &str, is_module: bool) -> Vec<AnalyzeResult> {
        let mut receiver = RustCheckResultReceiver { results: vec![] };
        unsafe {
            luauAnalyze_check(
                &mut receiver,
                self.underlying,
                RustString::encode(name),
                RustString::encode(contents),
                is_module,
            );
        }
        receiver.results
    }
}
impl Drop for LuaAnalyzer {
    fn drop(&mut self) {
        unsafe { luauAnalyze_free_frontend(self.underlying) }
    }
}
