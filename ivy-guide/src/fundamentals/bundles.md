# Bundles

Many of the built in systems require a certain set of components to be present
in order to avoid many `Option` in query and branches.

For example, rendering requires `Position`, `Rotation`, `Scale`, `Mesh`,
`Visible`, `Color`, `Pass`. Remembering to add all these when spawning entities
is a chore, and makes it easy to forget some and not having the entity show up.

To fix this several bundles are provided, which has the added benefit of
providing sane defaults.

The following snippet shows how to create a new entity which will be moving in
the world with an initial velocity.

**Note**: The entity won't be rendered, since no rendergraph has been setup,
and the entity does not have the `ObjectBundle` bundle. It is recommended to
use a custom layer and creating the entities in `new` or a `setup` function.
However, the raw usage of `App` is used for brevity.

## Bundles
The following bundles are provided:
- `RenderObjectBundle` - Renderable objects with mesh and materials
- `RigidBodyBundle` - Rigidbody object
- `ColliderBundle` - Collider for physics
- `TransformBundle` - Position an object with Position, Rotation, and Scale. A matching `TransformQuery` and `.into_matrix()` are provided as well.
- UI components are provided by the violet library, see violet-core for available components and widgets.
