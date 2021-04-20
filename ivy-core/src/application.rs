pub struct ApplicationBuilder {
    application: Application,
}

impl ApplicationBuilder {
    pub fn build(self) -> Application {
        self.application
    }
}

pub struct Application {
    name: String,
}

impl Application {
    pub fn builder() -> ApplicationBuilder {
        ApplicationBuilder {
            application: Application {
                name: "ivy".to_owned(),
            },
        }
    }

    pub fn run(&mut self) {
        println!("Hello, World!");
    }

    /// Return a reference to the application's name.
    pub fn name(&self) -> &String {
        &self.name
    }
}
