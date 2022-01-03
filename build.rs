use anyhow::{Context, Result};
use std::{
    error::Error,
    ffi::OsString,
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

fn compile_glsl(src: &Path, dst: &Path) -> Result<Child> {
    Command::new("glslc")
        .arg(src)
        .arg("-o")
        .arg(dst)
        .spawn()
        .context("Failed to run glslc")
}

struct CompilationProcess {
    path: PathBuf,
    child: Child,
}

fn compile_dir<A, B, F, C>(
    src: A,
    dst: B,
    rename_func: F,
    compile_func: C,
) -> Result<Vec<CompilationProcess>>
where
    A: AsRef<Path>,
    B: AsRef<Path>,
    F: Fn(&mut OsString),
    C: Fn(&Path, &Path) -> Result<Child>,
{
    let src = src.as_ref();
    let dst = dst.as_ref();

    walkdir::WalkDir::new(src)
        .follow_links(true)
        .into_iter()
        .flat_map(Result::ok)
        .map(|entry| -> Result<Option<_>> {
            let path = entry.path();

            rerun_if_changed(path);

            let metadata = entry.metadata()?;

            if metadata.is_dir() {
                return Ok(None);
            }

            let mut fname = entry.file_name().to_os_string();
            rename_func(&mut fname);

            let base = path
                .strip_prefix(src)?
                .parent()
                .context("No parent for path")?;

            let mut dst_path = PathBuf::new();
            dst_path.push(dst);
            dst_path.push(base);

            fs::create_dir_all(&dst_path)?;

            dst_path.push(fname);

            // Compare timestamps
            let dst_metadata = dst_path.metadata().ok();

            if let Some(dst_metadata) = dst_metadata {
                if dst_metadata.modified()? >= metadata.modified()? {
                    return Ok(None);
                }
            }

            eprintln!("{:?} => {:?}", path, dst_path);

            let child = compile_func(path, &dst_path)?;

            Ok(Some(CompilationProcess {
                child,
                path: path.to_owned(),
            }))
        })
        .flat_map(|val| val.transpose())
        .collect()
}

fn main() -> Result<()> {
    let children = compile_dir(
        "./shaders/",
        "./res/shaders",
        |path| path.push(".spv"),
        |src, dst| compile_glsl(src, dst),
    )?;

    children.into_iter().try_for_each(|mut val| -> Result<()> {
        if !val.child.wait()?.success() {
            Err(anyhow::anyhow!("Failed to compile: {:?}", val.path))
        } else {
            Ok(())
        }
    })?;

    Ok(())
}
