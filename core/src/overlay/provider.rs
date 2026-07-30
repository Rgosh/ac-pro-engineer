use super::state::OverlayState;

pub trait OverlayProvider {
    fn init(&mut self) -> Result<(), String> {
        Ok(())
    }
    fn render(&mut self, state: &OverlayState);
    fn set_visible(&mut self, _visible: bool) {}
    fn set_unlocked(&mut self, _unlocked: bool) {}
    fn shutdown(&mut self) {}
}
