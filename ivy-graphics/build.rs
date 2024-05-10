use anyhow::{Context, Result};
use shaderc::ShaderKind;
use std::{
    env,
    error::Error,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    slice,
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

    println!(
        "cargo:rerun-if-changed={}",
        path.canonicalize().unwrap().display()
    );
}

// struct CompilationProcess {
//     path: PathBuf,
//     child: Child,
// }

fn compile_dir<A, B, F, C>(src: A, dst: B, rename_func: F, compile_func: C) -> Result<()>
where
    A: AsRef<Path>,
    B: AsRef<Path>,
    F: Fn(&mut OsString),
    C: Fn(&Path, &Path) -> Result<()>,
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
                if dst_metadata.modified()? > metadata.modified()? {
                    return Ok(None);
                }
            }

            eprintln!("{:?} => {:?}", path, dst_path);

            compile_func(path, &dst_path)
                .with_context(|| format!("Failed to compile {:?}", path))?;

            Ok(Some(()))
        })
        .flat_map(|val| val.transpose())
        .collect()
}

fn glslc(src: &Path, dst: &Path) -> Result<()> {
    let compiler = shaderc::Compiler::new().unwrap();
    let mut options = shaderc::CompileOptions::new().unwrap();
    options.set_optimization_level(shaderc::OptimizationLevel::Performance);
    let source = fs::read_to_string(src)?;

    let ext = src.extension().unwrap_or_default();
    let kind = match ext.to_string_lossy().as_ref() {
        "vert" => ShaderKind::Vertex,
        "frag" => ShaderKind::Fragment,
        "geom" => ShaderKind::Geometry,
        "comp" => ShaderKind::Compute,
        _ => ShaderKind::InferFromSource,
    };

    options.add_macro_definition("EP", Some("main"));
    let binary_result = compiler.compile_into_spirv(
        &source,
        kind,
        &src.to_string_lossy(),
        "main",
        Some(&options),
    )?;

    assert_eq!(Some(&0x07230203), binary_result.as_binary().first());

    // Write to dst
    let bin = binary_result.as_binary();

    let data = bin.as_ptr() as *const u8;
    let bin = unsafe { slice::from_raw_parts(data, bin.len() * 4) };
    fs::write(dst, bin)?;
    Ok(())
}

fn main() -> Result<()> {
    let out_dir = env::var("OUT_DIR")?;
    let mut dst = PathBuf::new();
    dst.push(out_dir);
    dst.push("shaders");

    compile_dir(
        "./shaders/",
        &dst,
        |path| path.push(".spv"),
        |src, dst| glslc(src, dst),
    )?;

    Ok(())
}
