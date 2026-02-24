use mlua::prelude::LuaString;
use mlua::{BorrowedStr, Error, Lua, Result};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use typed_path::{
    NativePath, Utf8NativeEncoding, Utf8NativePath, Utf8UnixEncoding, Utf8UnixPath, Utf8UnixPathBuf,
};

type StandardPath = Utf8UnixPath;
type StandardPathBuf = Utf8UnixPathBuf;
type StandardEncoding = Utf8UnixEncoding;

pub fn lstr_to_system_path(path: LuaString) -> Result<PathBuf> {
    let path = path.to_str()?;
    let path = StandardPath::new(path.deref());
    let native = path.with_encoding::<Utf8NativeEncoding>();
    if native.is_absolute() {
        Err(Error::runtime("Absolute paths are not allowed in crabsoup."))
    } else {
        Ok((&native).into())
    }
}

pub fn lstr_to_path<'a, 'b: 'a>(path: &'b BorrowedStr<'a>) -> Result<&'a StandardPath> {
    Ok(StandardPath::new(path.deref()))
}

pub fn system_path_to_path(path: &Path) -> Result<StandardPathBuf> {
    let path =
        Utf8NativePath::from_bytes_path(NativePath::new(path.as_os_str().as_encoded_bytes()))
            .map_err(Error::runtime)?;
    Ok(path.with_encoding::<StandardEncoding>())
}

pub fn system_path_to_lstr(lua: &Lua, path: &Path) -> Result<LuaString> {
    let standard = system_path_to_path(path)?;
    lua.create_string(standard.as_str())
}

pub fn basename(path: &StandardPath) -> Result<&str> {
    if let Some(name) = path.file_name() {
        Ok(name)
    } else {
        Err(Error::runtime("Path must not end in a relative path component"))
    }
}

pub fn dirname(path: &StandardPath) -> Result<&str> {
    if let Some(parent) = path.parent() {
        Ok(parent.as_str())
    } else {
        Err(Error::runtime("Path must have a parent directory"))
    }
}
