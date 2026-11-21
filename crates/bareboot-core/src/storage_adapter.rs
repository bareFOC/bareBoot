//! Adapter bridging `embedded-storage::nor_flash::NorFlash` into `bareboot-core::FlashTarget`.

use crate::{CoreError, FlashTarget};
use embedded_storage::nor_flash::NorFlash;

pub struct NorFlashAdapter<F> {
    flash: F,
    app_base: u32,
    flash_base: u32,
    uid: [u8; 12],
}

impl<F: NorFlash> NorFlashAdapter<F> {
    pub const fn new(flash: F, app_base: u32, flash_base: u32, uid: [u8; 12]) -> Self {
        Self {
            flash,
            app_base,
            flash_base,
            uid,
        }
    }

    pub fn inner(&self) -> &F {
        &self.flash
    }

    pub fn inner_mut(&mut self) -> &mut F {
        &mut self.flash
    }

    pub fn into_inner(self) -> F {
        self.flash
    }

    fn to_offset(&self, addr: u32) -> Result<u32, CoreError> {
        addr.checked_sub(self.flash_base)
            .ok_or(CoreError::InvalidAddress)
    }
}

impl<F: NorFlash> FlashTarget for NorFlashAdapter<F> {
    fn page_size(&self) -> usize {
        F::ERASE_SIZE
    }

    fn app_base(&self) -> u32 {
        self.app_base
    }

    fn erase_page(&mut self, addr: u32) -> Result<(), CoreError> {
        let from = self.to_offset(addr)?;
        let to = from
            .checked_add(F::ERASE_SIZE as u32)
            .ok_or(CoreError::InvalidAddress)?;
        self.flash.erase(from, to).map_err(|_| CoreError::Flash)
    }

    fn write_page_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        let offset = self.to_offset(addr)?;
        self.flash.write(offset, data).map_err(|_| CoreError::Flash)
    }

    fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        let offset = self.to_offset(addr)?;
        self.flash.read(offset, out).map_err(|_| CoreError::Flash)
    }

    fn read_uid(&mut self, out: &mut [u8; 12]) {
        *out = self.uid;
    }

    unsafe fn boot_jump(_app_entry: u32) -> ! {
        panic!("boot_jump must be implemented by hardware target");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_storage::nor_flash::{ErrorType, ReadNorFlash};

    struct DummyNorFlash {
        data: [u8; 8192],
    }

    impl ErrorType for DummyNorFlash {
        type Error = core::convert::Infallible;
    }

    impl ReadNorFlash for DummyNorFlash {
        const READ_SIZE: usize = 1;

        fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            let start = offset as usize;
            bytes.copy_from_slice(&self.data[start..start + bytes.len()]);
            Ok(())
        }

        fn capacity(&self) -> usize {
            8192
        }
    }

    impl NorFlash for DummyNorFlash {
        const WRITE_SIZE: usize = 1;
        const ERASE_SIZE: usize = 64;

        fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            self.data[from as usize..to as usize].fill(0xFF);
            Ok(())
        }

        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            let start = offset as usize;
            self.data[start..start + bytes.len()].copy_from_slice(bytes);
            Ok(())
        }
    }

    #[test]
    fn test_nor_flash_adapter() {
        let dummy = DummyNorFlash { data: [0; 8192] };
        let mut adapter = NorFlashAdapter::new(dummy, 0x0800_1000, 0x0800_0000, [0x55; 12]);

        assert_eq!(adapter.page_size(), 64);
        assert_eq!(adapter.app_base(), 0x0800_1000);

        let mut uid = [0u8; 12];
        adapter.read_uid(&mut uid);
        assert_eq!(uid, [0x55; 12]);

        adapter.erase_page(0x0800_1000).unwrap();
        let mut buf = [0u8; 4];
        adapter.read_bytes(0x0800_1000, &mut buf).unwrap();
        assert_eq!(buf, [0xFF, 0xFF, 0xFF, 0xFF]);

        adapter.write_page_bytes(0x0800_1000, &[1, 2, 3, 4]).unwrap();
        adapter.read_bytes(0x0800_1000, &mut buf).unwrap();
        assert_eq!(buf, [1, 2, 3, 4]);
    }
}
