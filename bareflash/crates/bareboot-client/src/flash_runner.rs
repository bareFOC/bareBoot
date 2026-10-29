use crate::session::FlashSession;
use crate::transport::HostBus;
use crate::error::HostError;

pub fn run_flash<B, F, G>(
    bus: B,
    bytes: &[u8],
    tries: usize,
    verify: bool,
    mut on_progress: F,
    mut on_log: G,
) -> Result<(), HostError>
where
    B: HostBus,
    F: FnMut(u8),
    G: FnMut(String),
{
    let mut session = FlashSession::new(bus);
    let info = session.connect(tries).map_err(|e| HostError(format!("connect: {e}")))?;
    on_log(format!(
        "connected: mcu={} block_size={} app_base=0x{:x}",
        info.mcu_type, info.block_size, info.app_base
    ));

    let total = bytes.len();
    let mut offset = 0usize;
    while offset < total {
        let remaining = total - offset;
        let chunk_len = if remaining >= info.block_size { info.block_size } else { remaining };
        let mut block = vec![0xFFu8; info.block_size];
        block[..chunk_len].copy_from_slice(&bytes[offset..offset + chunk_len]);
        let addr = info.app_base + offset as u32;
        session.write_block(addr, &block).map_err(|e| HostError(format!("write block 0x{addr:x}: {e}")))?;
        if verify {
            session.verify_block(addr, &block).map_err(|e| HostError(format!("verify block 0x{addr:x}: {e}")))?;
        }
        offset += chunk_len;
        let percent = (offset * 100 / total.max(1)).min(100) as u8;
        on_progress(percent);
        on_log(format!("progress: {percent}% ({offset}/{total})"));
    }

    let pages = session.finish().map_err(|e| HostError(format!("eof: {e}")))?;
    on_log(format!("eof: {pages} pages written"));
    session.complete().map_err(|e| HostError(format!("complete: {e}")))?;
    on_log("done".to_string());
    Ok(())
}