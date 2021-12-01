# ivy-ui

Provides a fully fledged UI system for the Ivy framework.

Each UI element is composed of several components. See
[`crate::constraints`].

### Positioning

The different UI widgets are positioned using constraints.

[`constraints::AbsoluteOffset`] specifies an offset in pixels from the parent.

[`constraints::RelativeOffset`] specifies an offset proportional to the size of
the parent. `(1.0, 1.0)` specifies the top right corner, and `(-1.0, -1.0)`
specifies the bottom left corner. This coordinate system is also known as
normalized device coordinates. These constraints can be combined and will be
applied one after another by attaching them both to an entity.

[`constraints::RelativeSize`] size is relative to the parent size. A value larger
than 1.0 signifies that the child is larger than the parent.

[`constraints::AbsoluteSize`] size is given in absolute pixels. If combined with
[`crate::constraints::RelativeSize`] the result is additive. It is possible to supply a
negative absolute size if a relative size is used as it will subtract from
the parent size. This can be used to specify 50% of parent size, but 10
pixels smaller. This is useful for margins.

[`constraints::Aspect`] force the width to be dependent on the height.

[`constraints::Origin2D`] by default, widgets are positioned by their center. The
origin specifies an offset relative to the own size. For example, (1.0, 1.0)
moves the widget to be positioned in respect to the top right.

