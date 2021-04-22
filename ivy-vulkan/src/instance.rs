use super::Error;
use ash::{version::EntryV1_0, version::InstanceV1_0, Instance};
use ash::{vk, Entry};
use glfw::Glfw;
use std::ffi::{CStr, CString};

pub const VALIDATION_LAYERS: &'static [&str] = &["VK_LAYER_KHRONOS_validation"];
#[cfg(debug_assertions)]
pub const ENABLE_VALIDATION_LAYERS: bool = true;
#[cfg(not(debug_assertions))]
pub const ENABLE_VALIDATION_LAYERS: bool = false;

pub const INSTANCE_EXTENSIONS: &'static [&str] = &["VK_EXT_debug_utils"];

// Returns the currently enabled instance layers
pub fn get_layers() -> &'static [&'static str] {
    if ENABLE_VALIDATION_LAYERS {
        VALIDATION_LAYERS
    } else {
        &[]
    }
}

/// Creates a vulkan instance with the appropriate extensions and layers
pub fn create(
    entry: &Entry,
    glfw: &Glfw,
    name: &str,
    engine_name: &str,
) -> Result<Instance, Error> {
    let name = CString::new(name).unwrap();
    let engine_name = CString::new(engine_name).unwrap();

    let app_info = vk::ApplicationInfo::builder()
        .application_name(&name)
        .engine_name(&engine_name);

    let extensions: Vec<CString> = glfw
        .get_required_instance_extensions()
        .ok_or(Error::VulkanUnsupported)?
        .into_iter()
        .chain(INSTANCE_EXTENSIONS.iter().map(|s| s.to_string()))
        .map(CString::new)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Ensure extensions are present
    let missing = get_missing_extensions(entry, &extensions)?;

    if !missing.is_empty() {
        return Err(Error::MissingExtensions(missing));
    }

    let extension_names_raw = extensions
        .iter()
        .map(|ext| ext.as_ptr() as *const i8)
        .collect::<Vec<_>>();

    let instance_layers = get_layers();

    let layers = instance_layers
        .iter()
        .map(|s| CString::new(*s))
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Ensure all requested layers are present
    let missing = get_missing_layers(entry, &layers)?;

    if !layers.is_empty() && !missing.is_empty() {
        return Err(Error::MissingLayers(missing));
    }

    let layer_names_raw = layers
        .iter()
        .map(|layer| layer.as_ptr() as *const i8)
        .collect::<Vec<_>>();

    let create_info = vk::InstanceCreateInfo::builder()
        .application_info(&app_info)
        .enabled_extension_names(&extension_names_raw)
        .enabled_layer_names(&layer_names_raw);

    let instance = unsafe { entry.create_instance(&create_info, None)? };
    Ok(instance)
}

pub fn destroy(instance: &Instance) {
    unsafe { instance.destroy_instance(None) };
}

/// Returns a vector of missing extensions
fn get_missing_extensions(
    entry: &Entry,
    extensions: &[CString],
) -> Result<Vec<CString>, vk::Result> {
    let available = entry.enumerate_instance_extension_properties()?;

    Ok(extensions
        .iter()
        .filter(|ext| {
            available
                .iter()
                .find(|avail| unsafe {
                    CStr::from_ptr(avail.extension_name.as_ptr()) == ext.as_c_str()
                })
                .is_none()
        })
        .cloned()
        .collect())
}

/// Returns a vector of missing layers
fn get_missing_layers(entry: &Entry, layers: &[CString]) -> Result<Vec<CString>, vk::Result> {
    let available = entry.enumerate_instance_layer_properties()?;

    Ok(layers
        .iter()
        .filter(|ext| {
            available
                .iter()
                .find(|avail| unsafe {
                    CStr::from_ptr(avail.layer_name.as_ptr()) == ext.as_c_str()
                })
                .is_none()
        })
        .cloned()
        .collect())
}
