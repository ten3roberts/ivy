# Welcome to Ivy Engine

Ivy Engine is a modular, ECS-driven game engine written in Rust, designed for building high-performance 3D applications and games. Ivy provides a layered architecture, advanced rendering with WebGPU, physics simulation, asset management, and an integrated editor.

## Overview

Ivy Engine is built around an Entity Component System (ECS) using Flax, enabling efficient data-oriented programming. It features a modular crate structure, allowing developers to pick and choose components for their projects. The engine supports WebGPU-based rendering, physics with Rapier3D, async asset loading, and more.

**Systems communicate not by direct action, but by sharing data**

Ivy allows Rust applications for games, simulations, and interactive 3D apps with a decoupled, modular architecture that avoids spaghetti code.

## Key Features

### Core Systems
- **ECS Architecture**: Entity Component System via Flax for efficient game logic.
- **Layered Design**: Modular layers for organizing logic (rendering, physics, input, etc.).
- **Async Asset Management**: Non-blocking asset loading with caching and hot reloading.
- **Event System**: Top-down propagated system events (e.g., input, window resize) from the application driver to layers using an observer pattern.
- **Signals**: Entity-to-entity gameplay events for cross-entity communication.

### Rendering & Graphics
- **WebGPU Backend**: Modern GPU API support via WGPU.
- **PBR Rendering**: Physically Based Rendering with materials, lights, and shadows.
- **Render Graph**: Abstractions for fine-tuned render pipelines.
- **Post-Processing**: Effects like bloom, HDR tonemapping, and custom shaders.
- **Gizmos**: Debug visualization for development.

### Physics & Simulation
- **Integrated Physics**: Full 3D physics with Rapier3D.
- **Collision Detection**: Rigid bodies, colliders, joints, and effectors.

### Input & Interaction
- **Flexible Input System**: Composable vector generation from keyboard, mouse, and controllers.
- **Action Binding**: Map inputs to component updates or callbacks.

### User Interface
- **UI System**: Built on Violet GUI library with configurable widgets and positioning.
- **Editor Integration**: In-engine editor with property editing and gizmos.

### Asset Pipeline
- **GLTF Support**: Load 3D models, animations, and scenes.
- **Image Loading**: PNG, JPEG, HDR, and more.
- **Custom Assets**: Extensible asset types with serialization.

This guide will help you understand how to use Ivy Engine to build your applications, from basic concepts to advanced features.
