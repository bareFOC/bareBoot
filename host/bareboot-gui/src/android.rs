// Android JNI bridge for receiving native USB file descriptors from UsbManager / UsbDeviceConnection.

use std::sync::atomic::{AtomicI32, Ordering};

pub static ANDROID_USB_FD: AtomicI32 = AtomicI32::new(-1);
pub static ANDROID_EP_IN: AtomicI32 = AtomicI32::new(0x81);
pub static ANDROID_EP_OUT: AtomicI32 = AtomicI32::new(0x02);

#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_org_bareboot_gui_UsbBridge_setNativeFd(
    _env: jni::JNIEnv,
    _class: jni::objects::JClass,
    fd: jni::sys::jint,
    ep_in: jni::sys::jint,
    ep_out: jni::sys::jint,
) {
    ANDROID_USB_FD.store(fd, Ordering::SeqCst);
    ANDROID_EP_IN.store(ep_in, Ordering::SeqCst);
    ANDROID_EP_OUT.store(ep_out, Ordering::SeqCst);
}

pub fn get_attached_usb_fd() -> Option<(i32, u8, u8)> {
    let fd = ANDROID_USB_FD.load(Ordering::SeqCst);
    if fd >= 0 {
        let ep_in = ANDROID_EP_IN.load(Ordering::SeqCst) as u8;
        let ep_out = ANDROID_EP_OUT.load(Ordering::SeqCst) as u8;
        Some((fd, ep_in, ep_out))
    } else {
        None
    }
}
