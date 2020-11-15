use crate::error::Result;
use crate::memory::GuestPhysAddr;
use crate::virtdev::{DeviceRegion, EmulatedDevice, Event};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::RwLock;

#[derive(Default)]
pub struct IoApic;

impl IoApic {
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(IoApic {}))
    }
}

impl EmulatedDevice for IoApic {
    fn services(&self) -> Vec<DeviceRegion> {
        vec![
            DeviceRegion::MemIo(
                GuestPhysAddr::new(0xfec00000)..=GuestPhysAddr::new(0xfec010f0),
            ),
            //FIXME: this is actually the 1st HPET
            DeviceRegion::MemIo(
                GuestPhysAddr::new(0xfed00000)..=GuestPhysAddr::new(0xfed010f0),
            ),
        ]
    }

    fn on_event(&mut self, event: Event) -> Result<()> {
        info!("Ioapic event: {:?}", event.kind);
        Ok(())
    }
}
