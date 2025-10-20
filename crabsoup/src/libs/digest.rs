use blake2::{Blake2b512, Blake2s256};
use digest::Digest;
use md5::Md5;
use mlua::{
    prelude::{LuaFunction, LuaString},
    Lua, Result, Table,
};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::fmt::Write;

fn digest_helper<D: Digest>(lua: &Lua) -> Result<LuaFunction> {
    Ok(lua.create_function(|_, input: LuaString| {
        let mut str = String::new();
        for byte in D::digest(input.as_bytes()).as_slice() {
            write!(str, "{:02x}", *byte).unwrap();
        }
        Ok(str)
    })?)
}

pub fn create_digest_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;

    table.raw_set("md5", digest_helper::<Md5>(lua)?)?;
    table.raw_set("sha1", digest_helper::<Sha1>(lua)?)?;
    table.raw_set("sha256", digest_helper::<Sha256>(lua)?)?;
    table.raw_set("sha512", digest_helper::<Sha512>(lua)?)?;
    table.raw_set("blake2s", digest_helper::<Blake2s256>(lua)?)?;
    table.raw_set("blake2b", digest_helper::<Blake2b512>(lua)?)?;

    Ok(table)
}
