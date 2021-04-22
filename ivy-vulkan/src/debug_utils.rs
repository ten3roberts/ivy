use crate::Error;
use ash::extensions::ext::DebugUtils;
use ash::vk;
use ash::vk::DebugUtilsMessengerEXT;
use ash::Entry;
use ash::Instance;
use std::ffi::{c_void, CStr};

pub fn create(
    entry: &Entry,
    instance: &Instance,
) -> Result<(DebugUtils, DebugUtilsMessengerEXT), Error> {
    let debug_utils = DebugUtils::new(entry, instance);

    let create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
        .message_severity(
            vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
                | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE,
        )
        .message_type(
            vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
        )
        .pfn_user_callback(Some(debug_callback));

    let messenger = unsafe { debug_utils.create_debug_utils_messenger(&create_info, None)? };
    return Ok((debug_utils, messenger));
}

pub fn destroy(debug_utils: &DebugUtils, messenger: DebugUtilsMessengerEXT) {
    unsafe { debug_utils.destroy_debug_utils_messenger(messenger, None) };
}

// Debug callback
unsafe extern "system" fn debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    _message_types: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut c_void,
) -> vk::Bool32 {
    let msg = CStr::from_ptr((*p_callback_data).p_message)
        .to_str()
        .unwrap_or("Invalid UTF-8");
    match message_severity {
        vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::error!("{}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => log::warn!("{}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::INFO => log::info!("{}", msg),
        vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => log::trace!("{}", msg),
        _ => {
            panic!("Unexhaustive match")
        }
    };
    vk::FALSE
}
