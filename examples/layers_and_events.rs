use flume::Receiver;
use hecs::World;
use ivy::{App, Key, Layer, Logger, Resources, WindowInfo};
use ivy_base::{AppEvent, Events};
use ivy_graphics::layer::{WindowLayer, WindowLayerInfo};
use ivy_input::{Action, InputEvent, Modifiers};
use ivy_vulkan::SwapchainInfo;
use log::{error, info};

enum GameState {
    Playing,
    Paused,
}

impl GameState {
    fn toggle(&mut self) {
        match self {
            GameState::Playing => *self = GameState::Paused,
            GameState::Paused => *self = GameState::Playing,
        }
    }

    /// Returns `true` if the game state is [`Playing`].
    ///
    /// [`Playing`]: GameState::Playing
    fn is_playing(&self) -> bool {
        matches!(self, Self::Playing)
    }

    /// Returns `true` if the game state is [`Paused`].
    ///
    /// [`Paused`]: GameState::Paused
    fn is_paused(&self) -> bool {
        matches!(self, Self::Paused)
    }
}

struct GameLayer {
    score: [usize; 2],
    state: GameState,
    input_events: Receiver<InputEvent>,
}

impl GameLayer {
    pub fn new(_: &mut World, _: &mut Resources, events: &mut Events) -> Self {
        Self {
            score: [0; 2],
            state: GameState::Playing,
            input_events: events.subscribe(),
        }
    }
}

impl Layer for GameLayer {
    fn on_update(
        &mut self,
        _: &mut hecs::World,
        _: &mut ivy::Resources,
        e: &mut ivy_base::Events,
        _: std::time::Duration,
    ) -> anyhow::Result<()> {
        // Read events
        for event in self.input_events.try_iter() {
            match event {
                // Player A wins
                InputEvent::Key {
                    key: Key::A,
                    action: Action::Press,
                    ..
                } if self.state.is_playing() => self.score[0] += 1,
                // Player B wins
                InputEvent::Key {
                    key: Key::B,
                    action: Action::Press,
                    ..
                } if self.state.is_playing() => self.score[1] += 1,
                // Toggle pause
                InputEvent::Key {
                    key: Key::Escape,
                    action: Action::Press,
                    ..
                } => self.state.toggle(),
                // Tell the app to stop on Ctrl-q
                InputEvent::Key {
                    key: Key::Q,
                    mods: Modifiers::Control,
                    ..
                } => e.send(AppEvent::Exit),
                _ => {}
            }
        }

        if self.state.is_paused() {
            info!("Paused");
        } else {
            info!("score: {} - {}", self.score[0], self.score[1]);
        }

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    // Open a simple window for input events
    let window = WindowLayerInfo {
        window: WindowInfo {
            title: "Layer".into(),
            ..Default::default()
        },
        swapchain: SwapchainInfo::default(),
    };

    Logger::default().install();

    let result = App::builder()
        .try_push_layer(|_, r, _| WindowLayer::new(r, window))?
        .push_layer(GameLayer::new)
        .build()
        .run();

    // Pretty pritn result
    match &result {
        Ok(()) => {}
        Err(val) => error!("Error: {}", val),
    }

    result
}
