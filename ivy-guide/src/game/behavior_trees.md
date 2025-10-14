# Behavior Trees for Characetr Controllers

This guide covers how to use behavior trees for structured game logic in Ivy

---

## Overview

Behavior Trees (BTs) are a hierarchical way to organize decision logic for characters and AI.  
They replace tangled `if`/`else` code and complex state machines with modular, reusable nodes.

---

## Core API

The core API defines a `BehaviorTreeNode` trait and `NodeStatus` enum.

```rust
{{#include ../../../ivy-game/examples/character_controller.rs:setup}}
````

---

## Node Types

| Type                   | Description                                       |
| ---------------------- | ------------------------------------------------- |
| **Sequence**           | Runs children in order until one fails or runs    |
| **Selector**           | Runs children in order until one succeeds or runs |
| **Parallel**           | Runs all children; success threshold configurable |
| **Inverter**           | Flips success and failure                         |
| **Succeeder**          | Always returns success                            |
| **Action / Condition** | Leaf nodes for logic or checks                    |

---

## Controller Context

```rust
{{#include ../../../ivy-game/examples/character_controller.rs:controller_context}}
```

The behavior tree operates on this context every frame.

---

## Leaf Behaviors

```rust
{{#include ../../../ivy-game/examples/character_controller.rs:actions}}
```

Each function returns a `NodeStatus` indicating whether it’s done, failed, or still running.

---

## Building the Tree

```rust
{{#include ../../../ivy-game/examples/character_controller.rs:tree_build}}
```

### Visual Layout

```
Selector
 ├── Jump
 └── Sequence
      ├── GroundCheck
      └── Selector
           ├── Fall
           └── Sequence
                ├── Run
                └── Move
```

---

## Game Loop Example

```rust
{{#include ../../../ivy-game/examples/character_controller.rs:loop}}
```

---

## Example Output

```
Frame 0
Moving → pos Vec3(0.08, 0.0, 0.0)
→ Success

Frame 2
Jump!
→ Success

Frame 3
Falling... pos Vec3(0.0, 4.92, 0.0)
→ Running

Frame 6
Running!
Moving → pos Vec3(0.42, 0.0, 0.0)
→ Success
