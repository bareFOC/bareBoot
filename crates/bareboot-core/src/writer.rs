use crate::{BootConfig, CoreError, FlashTarget};

pub struct PagedWriter<F, const PAGE: usize> {
    flash: F,
    config: BootConfig,
    cursor: u32,
    page: [u8; PAGE],
    page_len: usize,
    page_base: u32,
    pages_written: u32,
    completed: bool,
}

impl<F: FlashTarget, const PAGE: usize> PagedWriter<F, PAGE> {
    pub fn new(flash: F, config: BootConfig) -> Self {
        let app_base = config.app_base;
        Self {
            flash,
            config,
            cursor: app_base,
            page: [0u8; PAGE],
            page_len: 0,
            page_base: app_base,
            pages_written: 0,
            completed: false,
        }
    }

    pub fn config(&self) -> &BootConfig {
        &self.config
    }

    pub fn pages_written(&self) -> u32 {
        self.pages_written
    }

    pub fn is_completed(&self) -> bool {
        self.completed
    }

    pub fn set_completed(&mut self) {
        self.completed = true;
    }

    pub fn write_block(&mut self, addr: u32, data: &[u8]) -> Result<(), CoreError> {
        if addr < self.config.app_base {
            return Err(CoreError::InvalidAddress);
        }
        if addr != self.cursor {
            return Err(CoreError::InvalidAddress);
        }
        if PAGE == 0 || self.flash.page_size() != PAGE {
            return Err(CoreError::Flash);
        }

        let mut offset = 0;
        while offset < data.len() {
            if self.page_len == 0 {
                self.flash.erase_page(self.page_base)?;
            }
            let remaining = PAGE - self.page_len;
            let chunk = (data.len() - offset).min(remaining);
            self.page[self.page_len..self.page_len + chunk]
                .copy_from_slice(&data[offset..offset + chunk]);
            self.page_len += chunk;
            offset += chunk;

            if self.page_len == PAGE {
                self.flush_page()?;
            }
        }

        self.cursor += data.len() as u32;
        Ok(())
    }

    pub fn finish(&mut self) -> Result<u32, CoreError> {
        if self.page_len > 0 {
            // Pad the partial page with erased flash state.
            for byte in &mut self.page[self.page_len..PAGE] {
                *byte = 0xff;
            }
            self.flush_page()?;
        }
        Ok(self.pages_written)
    }

    pub fn read_bytes(&mut self, addr: u32, out: &mut [u8]) -> Result<(), CoreError> {
        self.flash.read_bytes(addr, out)
    }

    fn flush_page(&mut self) -> Result<(), CoreError> {
        self.flash.write_page_bytes(self.page_base, &self.page)?;
        let mut check = [0u8; 4];
        self.flash.read_bytes(self.page_base, &mut check)?;
        if check != self.page[..4] { return Err(CoreError::Flash); }
        self.pages_written += 1;
        self.page_base += PAGE as u32;
        self.page_len = 0;
        Ok(())
    }

    pub fn into_flash(self) -> F {
        self.flash
    }
}
