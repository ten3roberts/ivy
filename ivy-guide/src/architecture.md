# Architecture

## Layered Architecture
Ivy is primarily structured into *layers*.

Each layer defines its own state and governs its own execution and event handling.

A layer may for example be an UI layer, input processing layer, or a game logic layer.

A layer is indented to be a self-contained unit of functionality, and does not inherently need to interact with the ECS.

It may, for example, react to an event such as `OnInit`, `Tick`, or `Input` events, or govern it's own async execution
for running things in the background outside the engines main event loop.

## Update and Fixed Update layers



