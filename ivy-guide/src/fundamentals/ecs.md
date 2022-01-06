# Entity Component System

The logic of the library is centered around the Entity Component System
design pattern.

In short, the ECS pattern describes entities as a collection of components. The
principle is tightly coupled with data driven development.

Ivy makes use of [hecs](https://github.com/ralith/hecs) for the ecs, with the
extension libraries [hecs-hierarchy](https://github.com/ten3roberts/hecs-hierarchy) for declaring relationships between entities, and [hecs-schedule](https://github.com/ten3roberts/hecs-schedule) for system execution abstractions, borrowing, and automatic multithreading.

Behaviors are controlled by the components attached to an entity.

[Systems](https://docs.rs/hecs-schedule/0.3.21/hecs_schedule/#system-and-schedule) declared in schedules then query and operate on the entities and can
modify state.
