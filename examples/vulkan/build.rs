use anyhow::{Context, Result};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
};

#[derive(Debug)]
struct CompilationFailure(PathBuf);

impl Error for CompilationFailure {}

impl std::fmt::Display for CompilationFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Failed to compile resource: {}", self.0.display())
    }
}

fn rerun_if_changed<P: AsRef<Path>>(path: P) {
    let path = path.as_ref();

    println!("cargo:rerun-if-changed={}", path.display());
}

fn compile_resource<P>(path: P) -> Result<Option<(PathBuf, Child)>>
where
    P: AsRef<Path> + ToOwned<Owned = PathBuf>,
{
    let path = path.as_ref();

    println!("Compiling {}", path.display());

    let extension = path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();

    let process = match extension {
        ".glsl" | "vert" | "frag" => Some(compile_glsl(path)?),
        _ => None,
    };

    Ok(process.map(|p| {
        rerun_if_changed(path);
        (path.to_owned(), p)
    }))
}

fn compile_glsl<P: AsRef<Path>>(path: P) -> Result<Child> {
    let path = path.as_ref();
    let path = path.to_string_lossy();

    let ofile = format!("{}.spv", path,);

    Command::new("glslc")
        .args(&[&path, "-o", &ofile])
        .spawn()
        .context("Failed to run glslc")
}

fn main() -> Result<()> {
    let resdir = "./res";
    let resdir = fs::canonicalize(resdir)
        .with_context(|| format!("Failed to canonicalize path {:?}", resdir))?;

    let processes = fs::read_dir(&resdir)
        .with_context(|| format!("Failed to read path {}", resdir.display()))?
        .filter_map(|dir| {
            dir.ok()
                .map(|dir| {
                    if dir.metadata().unwrap().is_dir() {
                        let path = dir.path();

                        // configure_rerun(&path);
                        fs::read_dir(&path).ok()
                    } else {
                        None
                    }
                })
                .flatten()
        })
        .flatten()
        .map(|entry| {
            let entry = entry?;
            compile_resource(entry.path())
        })
        .collect::<Result<Vec<_>>>()?;

    processes.into_iter().filter_map(|val| val).try_for_each(
        |(resource, mut process)| -> Result<()> {
            if !process.wait()?.success() {
                Err(CompilationFailure(resource).into())
            } else {
                Ok(())
            }
        },
    )?;

    Ok(())
}
