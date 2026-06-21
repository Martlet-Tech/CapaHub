use crate::event::Event;
use std::sync::Arc;

pub trait Plugin: Send {
    fn on_load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn on_enable(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn on_disable(&mut self) {}

    fn on_unload(&mut self);

    fn on_event(&mut self, event: Arc<dyn Event>);
}
