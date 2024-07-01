use anyhow::Context;
use std::env;

// Set directory to nth parent of current executable
pub fn normalize_dir(nth: usize) -> anyhow::Result<()> {
    let current_exe = env::current_exe()?
        .canonicalize()
        .context("Failed to canonicalize current exe")?;

    let dir = (0..nth + 1)
        .fold(Some(current_exe.as_path()), |acc, _| {
            acc.and_then(|val| val.parent())
        })
        .context("Failed to get parent dir of executable")?;

    env::set_current_dir(dir).context("Failed to set current directory")?;

    Ok(())
}
