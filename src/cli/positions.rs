use crate::state::StateStore;
use crate::telemetry::TelemetryNotifier;
use parking_lot::Mutex;
use std::sync::Arc;

pub fn run(state_store: Arc<Mutex<StateStore>>) {
    let store = state_store.lock();
    let positions = store.get_active_positions();
    TelemetryNotifier::render_positions_table(&positions);
}
