use mlua::{
    prelude::{LuaString, LuaUserDataRef, LuaUserDataRefMut},
    Error, Lua, Result, Table, UserData, UserDataFields,
};
use std::{
    ops::{Deref, DerefMut},
    process::{ExitStatus, Stdio},
    sync::{Arc, LazyLock, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, Command},
    runtime::{Builder, Runtime},
    task::JoinHandle,
};

static ASYNC_EXECUTOR: LazyLock<Runtime> = LazyLock::new(|| {
    Builder::new_multi_thread()
        .thread_name("Crabsoup - Process Manager Thread")
        .worker_threads(1)
        .enable_all()
        .build()
        .unwrap()
});

enum CommandSource {
    ShellCommand(LuaString),
    TableArgs(Vec<LuaString>),
}

fn parse_command_value(data: &Table) -> Result<CommandInfo> {
    let shell_command = if let Some(str) = data.raw_get::<Option<LuaString>>("shell")? {
        CommandSource::ShellCommand(str)
    } else {
        let len = data.raw_len();
        if len == 0 {
            return Err(Error::runtime("No command arguments given."));
        } else {
            let mut vec = Vec::new();
            for arg in data.clone().sequence_values::<LuaString>() {
                vec.push(arg?);
            }
            CommandSource::TableArgs(vec)
        }
    };

    let mut command = match shell_command {
        #[cfg(unix)]
        CommandSource::ShellCommand(cmd) => {
            let mut command = Command::new("sh");
            command.args(["-c", &cmd.to_str()?]);
            command
        }

        #[cfg(windows)]
        CommandSource::ShellCommand(cmd) => {
            let mut command = Command::new("cmd");
            command.args(["/C", cmd.to_str()?]);
            command
        }

        #[cfg(not(any(unix, windows)))]
        CommandSource::ShellCommand(cmd) => {
            compile_error!("No processlib implementation for this process!!")
        }

        CommandSource::TableArgs(args) => {
            assert_ne!(args.len(), 0);
            let mut command = Command::new(args[0].to_str()?.deref());
            for arg in args.into_iter().skip(1) {
                command.arg(arg.to_str()?.deref());
            }
            command
        }
    };

    let mut stdin = None;

    if let Some(dir) = data.get::<Option<LuaString>>("current_directory")? {
        command.current_dir(dir.to_str()?.deref());
    }
    if let Some(env) = data.get::<Option<Table>>("env")? {
        for t in env.pairs::<LuaString, LuaString>() {
            let (k, v) = t?;
            command.env(k.to_str()?.deref(), v.to_str()?.deref());
        }
    }
    if let Some(dir) = data.get::<Option<LuaString>>("stdin")? {
        command.stdin(Stdio::piped());
        stdin = Some(dir.to_str()?.to_string());
    } else {
        command.stdin(Stdio::null());
    }
    if let Some(true) = data.get::<Option<bool>>("capture_stdout")? {
        command.stdout(Stdio::piped());
    } else {
        command.stdout(Stdio::null());
    }
    if let Some(true) = data.get::<Option<bool>>("capture_stderr")? {
        command.stderr(Stdio::piped());
    } else {
        command.stderr(Stdio::inherit());
    }

    Ok(CommandInfo { command, stdin })
}

struct CommandInfo {
    command: Command,
    stdin: Option<String>,
}
impl CommandInfo {
    fn spawn(mut self) -> Result<LuaProcess> {
        let mut proc = ASYNC_EXECUTOR.block_on(async { self.command.spawn() })?;
        let mut join_handles = Vec::new();

        #[derive(Default)]
        struct ErrorInfo {
            stdin_error: Option<Error>,
            stdout_error: Option<Error>,
            stderr_error: Option<Error>,
        }
        let error_info = Arc::new(Mutex::new(ErrorInfo::default()));

        if let Some(mut stdin) = proc.stdin.take() {
            if let Some(stdin_content) = self.stdin.take() {
                let error_info = error_info.clone();
                let handle = ASYNC_EXECUTOR.spawn(async move {
                    match stdin.write_all(stdin_content.as_bytes()).await {
                        Ok(()) => {}
                        Err(e) => {
                            error_info.lock().unwrap().stdin_error = Some(e.into());
                        }
                    }
                });
                join_handles.push(handle);
            }
        }

        let stdout_info = Arc::new(Mutex::new(Vec::<u8>::new()));
        let stdout_captured = proc.stdout.is_some();
        if let Some(mut stdout) = proc.stdout.take() {
            let error_info = error_info.clone();
            let stdout_info = stdout_info.clone();
            let handle = ASYNC_EXECUTOR.spawn(async move {
                let mut buffer = [0; 1024 * 16];
                loop {
                    let len = match stdout.read(&mut buffer).await {
                        Ok(v) => v,
                        Err(e) => {
                            error_info.lock().unwrap().stdout_error = Some(e.into());
                            break;
                        }
                    };
                    if len == 0 {
                        break;
                    }
                    stdout_info.lock().unwrap().extend(&buffer[..len]);
                }
            });
            join_handles.push(handle);
        }

        let stderr_info = Arc::new(Mutex::new(Vec::<u8>::new()));
        let stderr_captured = proc.stderr.is_some();
        if let Some(mut stderr) = proc.stderr.take() {
            let error_info = error_info.clone();
            let stderr_info = stderr_info.clone();
            let handle = ASYNC_EXECUTOR.spawn(async move {
                let mut buffer = [0; 1024 * 16];
                loop {
                    let len = match stderr.read(&mut buffer).await {
                        Ok(v) => v,
                        Err(e) => {
                            error_info.lock().unwrap().stderr_error = Some(e.into());
                            break;
                        }
                    };
                    if len == 0 {
                        break;
                    }
                    stderr_info.lock().unwrap().extend(&buffer[..len]);
                }
            });
            join_handles.push(handle);
        }

        Ok(LuaProcess {
            info: LuaProcessInfo::RegularProcess(LuaRegularProcessInfo {
                child: proc,
                waiters: join_handles,
            }),
            stdout_info: if stdout_captured { Some(stdout_info) } else { None },
            stderr_info: if stderr_captured { Some(stderr_info) } else { None },
        })
    }
}
impl Deref for CommandInfo {
    type Target = Command;
    fn deref(&self) -> &Self::Target {
        &self.command
    }
}
impl DerefMut for CommandInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.command
    }
}

struct LuaProcess {
    info: LuaProcessInfo,
    stdout_info: Option<Arc<Mutex<Vec<u8>>>>,
    stderr_info: Option<Arc<Mutex<Vec<u8>>>>,
}
enum LuaProcessInfo {
    RegularProcess(LuaRegularProcessInfo),
    Waited,
}
struct LuaRegularProcessInfo {
    child: Child,
    waiters: Vec<JoinHandle<()>>,
}
impl LuaProcess {
    fn check_is_completed(&mut self) -> Result<bool> {
        match &mut self.info {
            LuaProcessInfo::RegularProcess(proc) => Ok(proc.child.try_wait()?.is_some()),
            LuaProcessInfo::Waited => Err(Error::runtime("Process has already been awaited.")),
        }
    }

    fn wait(&mut self) -> Result<LuaCompletedProcess> {
        let status = match &mut self.info {
            LuaProcessInfo::RegularProcess(proc) => {
                let wait_result = ASYNC_EXECUTOR.block_on(async move {
                    for waits in proc.waiters.drain(..) {
                        waits.await.unwrap();
                    }
                    proc.child.wait().await
                });
                Ok(wait_result?)
            }
            LuaProcessInfo::Waited => Err(Error::runtime("Process has already been awaited.")),
        }?;
        self.info = LuaProcessInfo::Waited;
        Ok(LuaCompletedProcess {
            status,
            stdout_info: self.stdout_info.take(),
            stderr_info: self.stderr_info.take(),
        })
    }
}
impl UserData for LuaProcess {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "Process");
    }
}

struct LuaCompletedProcess {
    status: ExitStatus,
    stdout_info: Option<Arc<Mutex<Vec<u8>>>>,
    stderr_info: Option<Arc<Mutex<Vec<u8>>>>,
}
impl LuaCompletedProcess {
    fn status(&self) -> Result<i32> {
        match self.status.code() {
            Some(status) => Ok(status),
            None => Err(Error::runtime("Process terminated by signal.")),
        }
    }
    fn check_status(&self) -> Result<()> {
        match self.status.code() {
            Some(0) => Ok(()),
            Some(status) => Err(Error::runtime(format!("Process has wrong status code: {status}"))),
            None => Err(Error::runtime("Process terminated by signal.")),
        }
    }

    fn get_stdout(&self, lua: &Lua) -> Result<LuaString> {
        match &self.stdout_info {
            None => Err(Error::runtime("stdout is not captured")),
            Some(vec) => Ok(lua.create_string(vec.lock().unwrap().as_slice())?),
        }
    }

    fn get_stderr(&self, lua: &Lua) -> Result<LuaString> {
        match &self.stderr_info {
            None => Err(Error::runtime("stderr is not captured")),
            Some(vec) => Ok(lua.create_string(vec.lock().unwrap().as_slice())?),
        }
    }
}
impl UserData for LuaCompletedProcess {
    fn add_fields<'lua, F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_meta_field("__type", "CompletedProcess");
    }
}

pub fn create_process_table(lua: &Lua) -> Result<Table> {
    let table = lua.create_table()?;

    // Simple API
    table.raw_set(
        "run",
        lua.create_function(|_, info: Table| {
            let mut child = parse_command_value(&info)?.spawn()?;
            let completed = child.wait()?;
            completed.check_status()?;
            Ok(())
        })?,
    )?;
    table.raw_set(
        "try_run",
        lua.create_function(|_, info: Table| {
            let mut child = parse_command_value(&info)?.spawn()?;
            let completed = child.wait()?;
            completed.status()
        })?,
    )?;
    table.raw_set(
        "run_output",
        lua.create_function(|lua, info: Table| {
            let mut command = parse_command_value(&info)?;
            command.stdout(Stdio::piped());
            let mut child = command.spawn()?;
            let completed = child.wait()?;
            completed.check_status()?;
            completed.get_stdout(lua)
        })?,
    )?;

    // Spawn API
    table.raw_set(
        "spawn",
        lua.create_function(|_, info: Table| {
            let child = parse_command_value(&info)?.spawn()?;
            Ok(child)
        })?,
    )?;
    table.raw_set(
        "is_completed",
        lua.create_function(|_, mut process: LuaUserDataRefMut<LuaProcess>| {
            process.check_is_completed()
        })?,
    )?;
    table.raw_set(
        "wait_on",
        lua.create_function(|_, mut process: LuaUserDataRefMut<LuaProcess>| process.wait())?,
    )?;

    // Completed process API
    table.raw_set(
        "check_status",
        lua.create_function(|_, process: LuaUserDataRef<LuaCompletedProcess>| {
            process.check_status()
        })?,
    )?;
    table.raw_set(
        "status",
        lua.create_function(|_, process: LuaUserDataRef<LuaCompletedProcess>| process.status())?,
    )?;
    table.raw_set(
        "get_stdout",
        lua.create_function(|lua, process: LuaUserDataRef<LuaCompletedProcess>| {
            process.get_stdout(lua)
        })?,
    )?;
    table.raw_set(
        "get_stderr",
        lua.create_function(|lua, process: LuaUserDataRef<LuaCompletedProcess>| {
            process.get_stderr(lua)
        })?,
    )?;

    Ok(table)
}
