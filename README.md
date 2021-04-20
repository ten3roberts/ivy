# Ivy
Ivy is a modular game engine in Rust using the vulkan
graphics API.

## Application
Represents the main application managing events and window

### Application creation
  ```rust
let app = Application::builder()
<<<<<<< HEAD
.name("Sandbox")
.with_step(Step::Tied,)
=======
  .name("Sandbox")
  .with_step(Step::Tied,)
>>>>>>> eede25c (Add default logger)

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
  pub fn new(app: &Application) -> Self {
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
<<<<<<< HEAD
=======

>>>>>>> eede25c (Add default logger)
  }
}
```
