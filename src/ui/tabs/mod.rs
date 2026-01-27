pub mod analysis;
pub mod dashboard;
pub mod engineer;
pub mod ffb;
pub mod guide;
pub mod settings;
pub mod setup;
pub mod strategy;
pub mod telemetry;

#[derive(PartialEq, Clone, Copy)]
pub enum Tab {
    Dashboard,
    Engineer,
    Telemetery,
    Analysis,
    Setup,
    Strategy,
    Ffb,
    Settings,
}

impl Tab {
    pub fn next(&self) -> Self {
        match self {
            Tab::Dashboard => Tab::Engineer,
            Tab::Engineer => Tab::Telemetery,
            Tab::Telemetery => Tab::Analysis,
            Tab::Analysis => Tab::Setup,
            Tab::Setup => Tab::Strategy,
            Tab::Strategy => Tab::Ffb,
            Tab::Ffb => Tab::Settings,
            Tab::Settings => Tab::Dashboard,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Tab::Dashboard => Tab::Settings,
            Tab::Engineer => Tab::Dashboard,
            Tab::Telemetery => Tab::Engineer,
            Tab::Analysis => Tab::Telemetery,
            Tab::Setup => Tab::Analysis,
            Tab::Strategy => Tab::Setup,
            Tab::Ffb => Tab::Strategy,
            Tab::Settings => Tab::Ffb,
        }
    }
}
