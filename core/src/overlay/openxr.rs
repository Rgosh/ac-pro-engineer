use super::provider::OverlayProvider;
use super::state::OverlayState;

pub struct OpenXrProvider {}

impl OpenXrProvider {
    pub fn new() -> Self {
        Self {}
    }
}

impl OverlayProvider for OpenXrProvider {
    fn init(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn render(&mut self, _state: &OverlayState) {}
    fn set_visible(&mut self, _visible: bool) {}
}
