#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphicsEvent {
    /// Signifies that the swapchain was recreated. This requires images that
    /// reference the old swapchain to be recreated.
    SwapchainRecreation,
}
