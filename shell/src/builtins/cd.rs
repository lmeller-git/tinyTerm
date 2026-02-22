use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use libtinyos::{
    eprintln,
    syscalls::{self, OpenOptions},
};

use crate::{builtins::BuiltinRunnable, env::get_env, logic::jobs::FDActionBuilder};

pub struct CD;

impl BuiltinRunnable for CD {
    fn run(arg: &str, env: &str, _builder: &FDActionBuilder) {
        if arg.is_empty() {
            return;
        }
        let global_env = get_env();
        let global_vars = global_env.vars();

        let cwd = if let Some(var) = env.split('\0').find(|element| element.starts_with("CWD"))
            && let Some((_, path)) = var.split_once('=')
        {
            path
        } else {
            global_vars.get("CWD").unwrap_or("/")
        };

        if let Some(resolved) = resolve_relative(cwd, arg) {
            drop(global_vars);
            if let Err(e) = global_env.update_cwd(resolved) {
                eprintln!("could not cd into {} due to {:?}", arg, e);
            }
        } else {
            drop(global_vars);
            if let Err(e) = global_env.update_cwd(arg.to_string()) {
                eprintln!("could not cd into {} due to {:?}", arg, e);
            }
        }
    }
}

fn resolve_relative(root: &str, append: &str) -> Option<String> {
    if append.starts_with('/') {
        return None;
    }

    if !(append.starts_with("./") || append.starts_with("../"))
        && let Some((append_root, _)) = append.split_once('/')
    {
        // check wether the path exits in the dir at root
        let env = get_env();
        let cwd = env.cwd().unwrap_or(unsafe {
            syscalls::open(root.as_ptr(), root.len(), OpenOptions::READ)
                .inspect_err(|e| {
                    eprintln!("cannot open current dir {}: {:?}", root, e);
                })
                .ok()?
        });

        let mut buffer = Vec::new();
        let mut cursor = 0;

        while let Ok(n) =
            unsafe { syscalls::read(cwd, buffer[cursor..].as_mut_ptr(), buffer.len() - cursor, 0) }
            && n > 0
        {
            if n as usize == buffer.len() - cursor {
                buffer.resize(buffer.len() + 32, 0);
            }
            cursor += n as usize;
        }

        let mut contents = buffer[..cursor].split(|b| *b == b'\t');

        if env.cwd().is_none() {
            _ = unsafe { syscalls::close(cwd) };
        }
        if contents.all(|child| child != append_root.as_bytes()) {
            return None;
        }
    }

    let mut root = root.to_string();
    if root.ends_with('/') {
        root.pop();
    }

    let segments = append
        .split('/')
        .filter(|&segment| !segment.is_empty() && segment != ".");
    for segment in segments {
        if segment == ".." {
            if let Some((r, _)) = root.rsplit_once('/') {
                root.truncate(r.len());
            }
        } else {
            root.push('/');
            root.push_str(segment);
        }
    }

    Some(root)
}
