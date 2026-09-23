//! UI Engine integrating StreamDeckController, BrokerClient, GuardedEngine, and Pages.
//!
//! Handles event loop dispatch, key-level dirty icon rendering, page transitions,
//! telemetry updates, and state replay across reconnections.

use std::sync::Arc;
use std::time::Instant;

use crate::broker_client::BrokerClient;
use crate::controller::StreamDeckController;
use crate::guarded::{GuardedAction, GuardedEngine};
use crate::pages::{
    IngestPage, Page, PageAction, PageContext, PageId, PerformancePage, SystemPage, KEY_COUNT,
};

pub struct UIEngine {
    controller: Arc<StreamDeckController>,
    broker: Arc<BrokerClient>,
    pub guarded: GuardedEngine,
    context: PageContext,

    performance_page: PerformancePage,
    ingest_page: IngestPage,
    system_page: SystemPage,

    last_rendered_page: Option<PageId>,
}

impl UIEngine {
    pub fn new(controller: Arc<StreamDeckController>, broker: Arc<BrokerClient>) -> Self {
        let guarded = GuardedEngine::default();
        let context = PageContext::default();

        let performance_page = PerformancePage::new();
        let ingest_page = IngestPage::new();
        let system_page = SystemPage::new();

        Self {
            controller,
            broker,
            guarded,
            context,
            performance_page,
            ingest_page,
            system_page,
            last_rendered_page: None,
        }
    }

    pub fn set_page(&mut self, new_page: PageId) {
        let old_page = self.context.active_page;
        if new_page == PageId::Performance && old_page != PageId::Performance {
            self.context.ingest.transcode_paused_for_perf = true;
            self.broker.send_command("ingest.pause_transcode", serde_json::json!({ "reason": "performance_mode_active" }));
        } else if old_page == PageId::Performance && new_page != PageId::Performance {
            self.context.ingest.transcode_paused_for_perf = false;
            self.broker.send_command("ingest.resume_transcode", serde_json::json!({}));
        }
        self.context.active_page = new_page;
        self.render_full_page();
    }

    pub fn active_page_id(&self) -> PageId {
        self.context.active_page
    }

    pub fn context(&self) -> &PageContext {
        &self.context
    }

    pub fn context_mut(&mut self) -> &mut PageContext {
        &mut self.context
    }

    fn get_active_page_ref(&self) -> &dyn Page {
        match self.context.active_page {
            PageId::Performance => &self.performance_page,
            PageId::Ingest => &self.ingest_page,
            PageId::System => &self.system_page,
        }
    }

    /// Render and upload all 32 keys for the current active page.
    pub fn render_full_page(&mut self) {
        self.context.now = Instant::now();
        self.context.state = self.broker.state();

        let page_ref = self.get_active_page_ref();
        for key in 0..KEY_COUNT as u8 {
            let canvas = page_ref.render_key(key, &self.context, &self.guarded);
            let jpeg = canvas.encode_jpeg();
            self.controller.set_icon(key, jpeg);
        }

        self.last_rendered_page = Some(self.context.active_page);
    }

    /// Render only a single specific key (fast partial update).
    pub fn render_single_key(&mut self, key: u8) {
        self.context.now = Instant::now();
        self.context.state = self.broker.state();

        let page_ref = self.get_active_page_ref();
        let canvas = page_ref.render_key(key, &self.context, &self.guarded);
        let jpeg = canvas.encode_jpeg();
        self.controller.set_icon(key, jpeg);
    }

    /// Process a button press event.
    pub fn on_key_press(&mut self, key: u8) {
        self.on_key_press_at(key, Instant::now());
    }

    /// Process a button press event at specific timestamp (for deterministic testing).
    pub fn on_key_press_at(&mut self, key: u8, now: Instant) {
        self.context.now = now;
        self.context.state = self.broker.state();

        let action = match self.context.active_page {
            PageId::Performance => self.performance_page.handle_key_press(key, &mut self.context, &self.broker, &mut self.guarded),
            PageId::Ingest => self.ingest_page.handle_key_press(key, &mut self.context, &self.broker, &mut self.guarded),
            PageId::System => self.system_page.handle_key_press(key, &mut self.context, &self.broker, &mut self.guarded),
        };

        self.handle_page_action(action);
        self.render_full_page();
    }

    /// Process a button release event.
    pub fn on_key_release(&mut self, key: u8) {
        self.on_key_release_at(key, Instant::now());
    }

    /// Process a button release event at specific timestamp (for deterministic testing).
    pub fn on_key_release_at(&mut self, key: u8, now: Instant) {
        self.context.now = now;
        self.context.state = self.broker.state();

        let action = match self.context.active_page {
            PageId::Performance => self.performance_page.handle_key_release(key, &mut self.context, &self.broker, &mut self.guarded),
            PageId::Ingest => self.ingest_page.handle_key_release(key, &mut self.context, &self.broker, &mut self.guarded),
            PageId::System => self.system_page.handle_key_release(key, &mut self.context, &self.broker, &mut self.guarded),
        };

        self.handle_page_action(action);
        self.render_full_page();
    }

    fn handle_page_action(&mut self, action: PageAction) {
        match action {
            PageAction::SwitchPage(new_page) => {
                self.set_page(new_page);
            }
            PageAction::TriggerGuarded(guarded_action) => {
                eprintln!("[UI ENGINE] Guarded action confirmed and executed: {:?}", guarded_action);
                match guarded_action {
                    GuardedAction::Shutdown => {
                        self.broker.send_command("system.shutdown", serde_json::json!({}));
                    }
                    GuardedAction::Reboot => {
                        self.broker.send_command("system.reboot", serde_json::json!({}));
                    }
                    GuardedAction::RestartRenderer => {
                        self.broker.restart_renderer();
                        self.broker.replay_state();
                    }
                    GuardedAction::ActivateShow => {
                        self.broker.send_command("show.activate", serde_json::json!({}));
                    }
                    GuardedAction::CancelIngest => {
                        self.broker.send_command("ingest.cancel", serde_json::json!({}));
                    }
                    GuardedAction::DeleteShow => {
                        self.broker.send_command("show.delete_staging", serde_json::json!({}));
                    }
                    GuardedAction::EjectUsb => {
                        self.context.ingest.usb_status = crate::pages::UsbDriveStatus::Ejected;
                        self.context.ingest.usb_inserted = false;
                        self.broker.send_command("usb.eject", serde_json::json!({}));
                    }
                }
            }
            PageAction::None => {}
        }
    }

    /// Periodic tick to handle hold progress updates, telemetry refresh, and state synchronization.
    pub fn tick(&mut self) {
        self.tick_at(Instant::now());
    }

    /// Explicit time tick for deterministic unit testing.
    pub fn tick_at(&mut self, now: Instant) {
        self.context.now = now;
        let prev_state = self.context.state.clone();
        self.context.state = self.broker.state();

        // 1. Poll hold timer
        let (hold_progress, triggered_action) = self.guarded.poll_hold(now);
        if let Some(action) = triggered_action {
            self.handle_page_action(PageAction::TriggerGuarded(action));
            self.render_full_page();
            return;
        }

        if hold_progress.is_some() {
            // Re-render to update hold ring progress animation
            self.render_full_page();
            return;
        }

        // 2. State change detection -> auto re-render if broker state mutated
        if self.context.state != prev_state {
            self.render_full_page();
        }
    }

    /// Read real system telemetry if available
    pub fn refresh_telemetry(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if let Ok(h) = std::fs::read_to_string("/etc/hostname") {
                self.context.telemetry.hostname = h.trim().to_string();
            }

            if let Ok(t_str) = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp") {
                if let Ok(t_val) = t_str.trim().parse::<f32>() {
                    self.context.telemetry.temperature_c = t_val / 1000.0;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::MockTransport;

    #[test]
    fn test_ui_engine_full_lifecycle() {
        let mock_transport = Arc::new(MockTransport::new());
        let controller = Arc::new(StreamDeckController::new(Default::default(), mock_transport.clone()));
        let (broker, mock_broker) = BrokerClient::with_mock();
        let broker_arc = Arc::new(broker);

        let mut ui = UIEngine::new(controller.clone(), broker_arc.clone());

        // Boots directly onto Performance (no more separate Ready landing page)
        assert_eq!(ui.active_page_id(), PageId::Performance);

        // Trigger Clip 5 on Performance page
        ui.on_key_press(5);
        ui.on_key_release(5);

        let lines = mock_broker.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"clip.trigger\"") && l.contains("\"key\":5")));

        // Switch to System page by holding key 30 (STOP/SYSTEM) 3+ seconds
        let hold_start = Instant::now();
        ui.on_key_press_at(30, hold_start);
        ui.on_key_release_at(30, hold_start + std::time::Duration::from_millis(3100));
        assert_eq!(ui.active_page_id(), PageId::System);

        // Arm shutdown on System page (keys 27-29)
        ui.on_key_press(27);
        assert!(ui.guarded.is_active());

        // Confirm shutdown with key 0
        ui.on_key_press(0);
        let out = mock_broker.drain_outgoing();
        assert!(out.iter().any(|l| l.contains("\"system.shutdown\"")));
    }
}
