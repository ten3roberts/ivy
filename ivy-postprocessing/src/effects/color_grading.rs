use bytemuck::{Pod, Zeroable};

/// Color grading configuration for cinematic color correction
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ColorGradingConfig {
    // Professional Lift/Gamma/Gain color correction
    pub lift: glam::Vec3,  // Shadow color adjustment (-1.0 to 1.0 per channel)
    pub exposure: f32,     // Overall brightness (-4.0 to +4.0 EV)
    pub gamma: glam::Vec3, // Midtone adjustment (0.1 to 10.0 per channel)
    pub contrast: f32,     // Contrast multiplier (0.0 to 2.0)
    pub gain: glam::Vec3,  // Highlight boost (0.0 to 16.0 per channel)
    //
    // Basic tonal adjustments
    pub saturation: f32, // Color saturation (0.0 to 2.0)

    // White balance
    pub temperature: f32, // Blue ↔ Yellow shift (-1.0 to 1.0)
    pub tint: f32,        // Green ↔ Magenta shift (-1.0 to 1.0)
    pub _padding: glam::Vec2,
}

impl Default for ColorGradingConfig {
    fn default() -> Self {
        Self {
            exposure: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            lift: glam::Vec3::ZERO,
            gamma: glam::Vec3::ONE,
            gain: glam::Vec3::ONE,
            temperature: 0.0,
            tint: 0.0,
            _padding: Default::default(),
        }
    }
}

impl ColorGradingConfig {
    /// Neutral color grading (no adjustments)
    pub fn neutral() -> Self {
        Self::default()
    }

    /// Cinematic color grading deeper colors, higher contrast)
    pub fn cinematic() -> Self {
        Self {
            exposure: -0.3,                         // Darker overall for depth
            contrast: 1.2,                          // Higher contrast for drama
            saturation: 0.9,                        // Slightly desaturated for realism
            lift: glam::Vec3::new(0.1, 0.05, 0.15), // Blue-tinted shadows
            gamma: glam::Vec3::new(1.0, 0.95, 1.1), // Warmer midtones
            gain: glam::Vec3::new(1.2, 1.1, 1.0),   // Brighter highlights
            temperature: -0.1,                      // Slightly cool
            tint: 0.0,
            _padding: Default::default(),
        }
    }

    /// Cave atmosphere (grayish, darker, lower contrast)
    pub fn cave() -> Self {
        Self {
            exposure: -0.5,                        // Much darker
            contrast: 0.8,                         // Lower contrast (flatter)
            saturation: 0.6,                       // Desaturated (grayish)
            lift: glam::Vec3::new(0.2, 0.2, 0.2),  // Neutral gray shadows
            gamma: glam::Vec3::new(0.9, 0.9, 0.9), // Darker midtones
            gain: glam::Vec3::new(0.8, 0.8, 0.8),  // Muted highlights
            temperature: -0.2,                     // Cooler
            tint: 0.0,
            _padding: Default::default(),
        }
    }

    /// Bright sunny day
    pub fn bright_sunny() -> Self {
        Self {
            exposure: 0.3,
            contrast: 1.1,
            saturation: 1.2,
            temperature: 0.1, // Warmer
            _padding: Default::default(),
            ..Default::default()
        }
    }
}

/// Dynamic color grading controller for smooth transitions
#[derive(Clone)]
pub enum EasingFunction {
    Linear,
    SmoothStep,
    Exponential,
}

pub struct ColorGradingController {
    current: ColorGradingConfig,
    target: ColorGradingConfig,
    transition_duration: f32,
    elapsed_time: f32,
    easing_function: EasingFunction,
}

impl ColorGradingController {
    pub fn new(initial_config: ColorGradingConfig) -> Self {
        Self {
            current: initial_config.clone(),
            target: initial_config,
            transition_duration: 0.0,
            elapsed_time: 0.0,
            easing_function: EasingFunction::SmoothStep,
        }
    }

    pub fn transition_to(&mut self, target: ColorGradingConfig, duration: f32) {
        self.target = target;
        self.transition_duration = duration;
        self.elapsed_time = 0.0;
    }

    pub fn update(&mut self, delta_time: f32) -> &ColorGradingConfig {
        if self.elapsed_time < self.transition_duration {
            self.elapsed_time += delta_time;
            let t = (self.elapsed_time / self.transition_duration).min(1.0);
            let eased_t = self.apply_easing(t);

            self.current = self.interpolate_configs(&self.current, &self.target, eased_t);
        }
        &self.current
    }

    fn interpolate_configs(
        &self,
        a: &ColorGradingConfig,
        b: &ColorGradingConfig,
        t: f32,
    ) -> ColorGradingConfig {
        ColorGradingConfig {
            exposure: a.exposure + (b.exposure - a.exposure) * t,
            contrast: a.contrast + (b.contrast - a.contrast) * t,
            saturation: a.saturation + (b.saturation - a.saturation) * t,
            lift: a.lift.lerp(b.lift, t),
            gamma: a.gamma.lerp(b.gamma, t),
            gain: a.gain.lerp(b.gain, t),
            temperature: a.temperature + (b.temperature - a.temperature) * t,
            tint: a.tint + (b.tint - a.tint) * t,
            _padding: Default::default(),
        }
    }

    fn apply_easing(&self, t: f32) -> f32 {
        match self.easing_function {
            EasingFunction::Linear => t,
            EasingFunction::SmoothStep => t * t * (3.0 - 2.0 * t),
            EasingFunction::Exponential => 1.0 - (-t * 5.0).exp(),
        }
    }
}