pub mod frame;
pub mod provider;
pub mod shared_writer;
pub mod state;

#[cfg(target_os = "windows")]
pub mod native_window;
pub mod openxr;

use self::provider::OverlayProvider;
use self::state::OverlayState;
use crate::session_info::SessionInfo;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayMode {
    External,
    NativeDesktop,
    StandaloneTest,
    VR,
}

pub struct OverlayManager {
    pub state: OverlayState,
    pub mode: OverlayMode,
    provider: Option<Box<dyn OverlayProvider + Send>>,
    pub is_active: bool,
    pub is_unlocked: bool,
}

impl OverlayManager {
    pub fn new(mode: OverlayMode) -> Self {
        let mut provider: Option<Box<dyn OverlayProvider + Send>> = match mode {
            OverlayMode::VR => Some(Box::new(openxr::OpenXrProvider::new())),
            #[cfg(target_os = "windows")]
            OverlayMode::NativeDesktop => {
                Some(Box::new(native_window::NativeWindowProvider::new(false)))
            }
            #[cfg(target_os = "windows")]
            OverlayMode::StandaloneTest => {
                Some(Box::new(native_window::NativeWindowProvider::new(true)))
            }
            #[cfg(not(target_os = "windows"))]
            OverlayMode::NativeDesktop | OverlayMode::StandaloneTest => None,
            OverlayMode::External => None,
        };

        if let Some(p) = &mut provider {
            let _ = p.init();
        }

        Self {
            state: OverlayState::default(),
            mode,
            provider,
            is_active: false,
            is_unlocked: false,
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
        if let Some(p) = &mut self.provider {
            p.set_visible(active);
        }
    }

    pub fn toggle(&mut self) {
        self.set_active(!self.is_active);
    }

    pub fn toggle_unlocked(&mut self) {
        self.is_unlocked = !self.is_unlocked;
        if let Some(p) = &mut self.provider {
            p.set_unlocked(self.is_unlocked);
        }
    }

    pub fn update(&mut self, session: &SessionInfo) {
        self.state.update_from_session(session);
    }

    pub fn render_manual_state(&mut self) {
        if self.is_active
            && let Some(provider) = &mut self.provider
        {
            provider.render(&self.state);
        }
    }

    pub fn get_state(&self) -> &OverlayState {
        &self.state
    }
}
