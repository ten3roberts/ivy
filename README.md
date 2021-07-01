# Ivy
Ivy is a modular game and graphics framework in Rust.

## Application
Represents the main application managing events and window

### Application creation
  ```rust
let app = Application::builder()
.name("Sandbox")
.with_layer(SandboxLayer::new)
  .name("Sandbox")
  .with_step(Step::Tied,)

  app.run();
  ```

## Layers
  Layers can be added to an application and allow for low
  level engine control. Each layer can interact directly and
  control the application. Used to represent low level logic
  and dispatch different workloads.

  ```rust
  struct SandboxLayer {
frame: usize,
         accumulator: f32,
         clock: Clock,
         dt: f32,
  }

impl SandboxLayer {
  pub fn new(world: &mut World, events: &mut Events) -> Self {
    Self {
frame: 0,
         accumulator: 0.0,
         clock: Clock::new(),
         dt: 0.02,
    }
  }
}

impl Layer for SandboxLayer {
  fn on_update(&mut self, app: &Application) {
    println!("Updating");
    frame_time = self.clock.reset();

    // Run physics at a fixed deltatime of 0.02
    self.accumulator += self.dt;
    while self.accumulator >= self.dt {
      app.world.run_workload("Physics");
      self.accumulator -= dt;
    }
  }
}
```
