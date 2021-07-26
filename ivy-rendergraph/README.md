# Ivy Rendergraph

Provides an easy to use node and dependency oriented graph abstraction for
renderpasses, barriers, and subpass dependencies.

## How it works

The graph is built using nodes, where each node corresponds to a `vkRenderPass`.

Each node contains:
  - A function which is executed to fill a commandbuffer for each frame
  - A list of color attachments
  - An optional depth attachment
  - Forward dependencies to build src and dst stages and access masks for
    subsequent renderpasses.

