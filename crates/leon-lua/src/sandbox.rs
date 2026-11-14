//! The sandbox around every Lua state (config, actions, scripts).
//!
//! mlua's safe mode already leaves out the debug library and C modules. On top: `require` finds
//! siblings in `dir`; `io.popen`, `os.execute` and `os.exit` raise; only text chunks load (Lua 5.1
//! doesn't verify bytecode, so a crafted chunk could corrupt memory); `newproxy` is gone, since a
//! `__gc` finalizer runs with hooks off and so could outrun any deadline; and `pcall`, `xpcall` and
//! `coroutine.resume` re-raise once `flags` says the run is over (timed out or `leon.stop`), so Lua
//! code can't catch its way past either.

use std::path::Path;
use std::rc::Rc;

use mlua::{Lua, Table, Value};

use crate::runtime::Flags;

pub(crate) fn sandbox(lua: &Lua, dir: &Path, flags: &Rc<Flags>) -> mlua::Result<()> {
    let package: Table = lua.globals().get("package")?;
    // shortcut: a config dir whose path holds `?` or `;` breaks these templates; escape them if anyone hits it.
    package.set("path", format!("{0}/?.lua;{0}/?/init.lua", dir.display()))?;
    package.set("cpath", "")?;
    // The full debug library is memory-unsafe under mlua; traceback covers `xpcall(f, debug.traceback)`.
    let debug = lua.create_table()?;
    debug.set(
        "traceback",
        lua.create_function(|lua, (msg, level): (Value, Option<usize>)| match msg {
            Value::String(s) => lua.traceback(Some(&s.to_string_lossy()), level.unwrap_or(1)).map(Value::String),
            Value::Nil => lua.traceback(None, level.unwrap_or(1)).map(Value::String),
            v => Ok(v),
        })?,
    )?;
    lua.globals().set("debug", debug)?;
    let f = flags.clone();
    let aborted = lua.create_function(move |_, ()| Ok(f.abort.borrow().clone()))?;
    lua.load(
        r#"local aborted = ...
           local function gone(name)
             return function() error(name .. " is not available in leon; use leon.exec", 2) end
           end
           io.popen, os.execute, os.exit = gone("io.popen"), gone("os.execute"), gone("os.exit")
           newproxy, string.dump = nil, nil

           local function text(s)
             if type(s) == "string" and s:byte(1) == 27 then error("binary chunks are not allowed", 3) end
             return s
           end
           local loadstring0, load0, open, gsub = loadstring, load, io.open, string.gsub
           loadstring = function(s, name) return loadstring0(text(s), name) end
           load = function(f, name)
             local first = true
             return load0(function()
               local piece = f()
               if first and piece ~= nil and piece ~= "" then first = false; text(piece) end
               return piece
             end, name)
           end
           loadfile = function(path)
             local f, err = open(path or "", "rb")
             if not f then return nil, err end
             local s = f:read("*a")
             f:close()
             return loadstring(gsub(text(s), "^#[^\n]*", ""), "@" .. path)
           end
           dofile = function(path) return assert(loadfile(path))() end
           -- Lua's own file searcher loads bytecode too; this one goes through loadfile.
           package.loaders[2] = function(name)
             local file = gsub(name, "%.", "/")
             local tried = ""
             for template in package.path:gmatch("[^;]+") do
               local path = gsub(template, "%?", file)
               local f = open(path, "rb")
               if f then
                 f:close()
                 return assert(loadfile(path))
               end
               tried = tried .. "\n\tno file '" .. path .. "'"
             end
             return tried
           end

           local function check(ok, ...)
             if not ok then
               local m = aborted()
               if m then error(m, 0) end
             end
             return ok, ...
           end
           local pcall0, xpcall0, resume0 = pcall, xpcall, coroutine.resume
           pcall = function(...) return check(pcall0(...)) end
           xpcall = function(f, h) return check(xpcall0(f, h)) end
           coroutine.resume = function(...) return check(resume0(...)) end"#,
    )
    .set_name("=sandbox")
    .call(aborted)
}
