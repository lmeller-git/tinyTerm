use crate::logic::jobs::FDActionBuilder;

pub mod cd;

pub fn dispatch(
    cmd: &str,
    arg: &str,
    env_vars: &str,
    action_builder: &FDActionBuilder,
) -> Result<(), ()> {
    match cmd {
        "cd" => cd::CD::run(arg, env_vars, action_builder),
        _ => return Err(()),
    }
    Ok(())
}

pub trait BuiltinRunnable {
    fn run(arg: &str, env_vars: &str, action_builder: &FDActionBuilder);
}
