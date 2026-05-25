use core::sync::atomic::{AtomicBool, AtomicU32};

use alloc::{str, string::String};
use conquer_once::spin::OnceCell;
use hashbrown::HashMap;
use libtinyos::{
    serial_println,
    syscalls::{self, FStat, FileDescriptor, NodeType, OpenOptions, SysCallRes},
};
use spin::{Mutex, RwLock, RwLockReadGuard, rwlock::RwLockWriteGuard};

use crate::logic::jobs::ExecutionContext;

static ENV: OnceCell<StaticEnv> = OnceCell::uninit();

pub fn get_env<'a>() -> &'a StaticEnv {
    ENV.get_or_init(|| EnvBuilder::new().default_init().build())
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct EnvVarStack {
    temp: HashMap<String, String>,
}

impl EnvVarStack {
    pub fn new() -> Self {
        Self {
            temp: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn env(&self) -> String {
        self.temp
            .iter()
            .flat_map(|(k, v)| [k, "=", v, "\0"])
            .collect()
    }

    pub fn add(&mut self, k: String, v: String) -> Option<String> {
        self.temp.insert(k, v)
    }

    pub fn get(&self, v: &str) -> Option<&str> {
        self.temp.get(v).map(|v| v.as_str())
    }

    /// builds an env using entries from both stacks, with entries in stack two having priority
    pub fn joined_as_env(one: &EnvVarStack, two: &EnvVarStack) -> String {
        one.temp
            .iter()
            .filter(|(k, _)| two.get(k).is_none())
            .flat_map(|(k, v)| [k, "=", v, "\0"])
            .chain(two.temp.iter().flat_map(|(k, v)| [k, "=", v, "\0"]))
            .collect()
    }
}

struct EnvBuilder {
    inner: StaticEnv,
}

impl EnvBuilder {
    pub fn new() -> Self {
        Self {
            inner: StaticEnv::new(),
        }
    }

    pub fn build(self) -> StaticEnv {
        self.inner
    }

    pub fn default_init(self) -> Self {
        let mut vars = self.inner.vars.write();
        vars.temp.insert("PATH".into(), ".:/ram/bin".into());
        drop(vars);
        _ = self.inner.update_cwd("/".into());
        self
    }

    #[allow(dead_code)]
    pub fn add_file_conf(self, _f: String) -> Self {
        todo!()
    }
}

pub struct StaticEnv {
    vars: RwLock<EnvVarStack>,
    open_wd: AtomicU32,
    shell_is_fg: AtomicBool,
    active_exec_ctx: Mutex<Option<ExecutionContext>>,
}

impl StaticEnv {
    fn new() -> Self {
        Self {
            vars: RwLock::default(),
            open_wd: 0.into(),
            shell_is_fg: true.into(),
            active_exec_ctx: Mutex::default(),
        }
    }

    #[allow(dead_code)]
    pub fn cwd(&self) -> Option<FileDescriptor> {
        let fd = self.open_wd.load(core::sync::atomic::Ordering::Acquire);
        (fd != 0).then_some(fd)
    }

    pub fn update_cwd(&self, p: String) -> SysCallRes<()> {
        let mut writer = self.vars.write();
        let cwd_fd = unsafe { syscalls::open(p.as_ptr(), p.len(), OpenOptions::READ) }?;

        let mut stat_buf = FStat::default();
        unsafe { syscalls::fstat(cwd_fd, &mut stat_buf as *mut FStat) }?;

        if stat_buf.node_type != NodeType::DIR {
            return unsafe { syscalls::close(cwd_fd) };
        }

        let old = self
            .open_wd
            .swap(cwd_fd, core::sync::atomic::Ordering::AcqRel);
        if old != 0 && old != cwd_fd {
            unsafe { syscalls::close(old) }?;
        }

        writer.temp.insert("CWD".into(), p);
        Ok(())
    }

    pub fn vars(&self) -> RwLockReadGuard<'_, EnvVarStack> {
        self.vars.read()
    }

    #[allow(dead_code)]
    pub fn vars_mut(&self) -> RwLockWriteGuard<'_, EnvVarStack> {
        self.vars.write()
    }

    #[allow(dead_code)]
    pub fn env(&self) -> String {
        self.vars.read().env()
    }

    pub fn is_fg(&self) -> bool {
        self.shell_is_fg.load(core::sync::atomic::Ordering::Acquire)
    }

    pub fn set_bg(&self) {
        self.shell_is_fg
            .store(false, core::sync::atomic::Ordering::Release);
    }

    pub fn set_fg(&self) {
        self.shell_is_fg
            .store(true, core::sync::atomic::Ordering::Release);
    }

    #[allow(dead_code)]
    pub fn exec_ctx(&self) -> &Mutex<Option<ExecutionContext>> {
        &self.active_exec_ctx
    }

    pub fn clear_ctx(&self) {
        if let Some(ctx) = self.active_exec_ctx.lock().take() {
            serial_println!("there was a stale ctx. killing it...");
            _ = ctx.kill_all();
        }
    }

    pub fn set_ctx(&self, ctx: ExecutionContext) {
        if let Some(ctx) = self.active_exec_ctx.lock().replace(ctx) {
            serial_println!("there was a stale ctx. killing it...");
            _ = ctx.kill_all();
        }
    }

    pub fn get_ctx(&self) -> Option<ExecutionContext> {
        self.active_exec_ctx.lock().clone()
    }
}
