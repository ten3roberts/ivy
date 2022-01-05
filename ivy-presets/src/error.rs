use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Graphics error")]
    Graphics(#[from] ivy_graphics::Error),
    #[error("Resources error")]
    Resources(#[from] ivy_resources::Error),
    #[error("Vulkan error")]
    Vulkan(#[from] ivy_vulkan::Error),
    #[error("UI error")]
    Ui(#[from] ivy_ui::Error),

    #[error("Rendergraph error")]
    RenderGraph(#[from] ivy_rendergraph::Error),

    #[error("Failed to get component from entity")]
    ComponentError(#[from] hecs::ComponentError),

    #[error("Failed to find main camera in world")]
    MissingMainCamera,
    #[error("Failed to find canvas in world")]
    MissingCanvas,
    #[error("No pipeline of type {0:?} exists in pipeline store")]
    MissingPipeline(&'static str),
    #[error("No pipeline with name {0:?} and type {1:?} exists in pipeline store")]
    MissingPipelineName(String, &'static str),
}

pub type Result<T> = std::result::Result<T, Error>;
