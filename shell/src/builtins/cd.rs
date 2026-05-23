use alloc::borrow::ToOwned;
use libtinyos::{eprintln, path::Path};

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

        let mut path = Path::new(cwd).to_owned().join(arg);
        path.canonicalize();

        // TODO : we should check if this is actually a dir. Currently this is impossible, need kernel support for that.
        drop(global_vars);
        _ = global_env
            // to_string inserts a newline at the end de to display invocation. thus we do a detour via Path
            .update_cwd(path.as_path().as_str().to_owned())
            .inspect_err(|e| _ = eprintln!("could not cd into {} due to {:?}", arg, e));
    }
}
