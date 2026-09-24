//! Native default render endpoint names (WASAPI / Core Audio / Pulse).

pub fn os_default_render_endpoint_names() -> Vec<String> {
    native_os_defaults()
}

#[cfg(windows)]
fn native_os_defaults() -> Vec<String> {
    windows_default_name().into_iter().collect()
}

#[cfg(target_os = "macos")]
fn native_os_defaults() -> Vec<String> {
    macos_default_name().into_iter().collect()
}

#[cfg(target_os = "linux")]
fn native_os_defaults() -> Vec<String> {
    linux_default_names()
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn native_os_defaults() -> Vec<String> {
    Vec::new()
}

#[cfg(windows)]
fn windows_default_name() -> Option<String> {
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
    };
    use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, STGM_READ,
    };

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let store = device.OpenPropertyStore(STGM_READ).ok()?;
        let value = store.GetValue(&PKEY_Device_FriendlyName).ok()?;
        let pwstr = PropVariantToStringAlloc(&value).ok()?;
        pwstr.to_string().ok()
    }
}

/// Same Core Audio default output CPAL uses, resolved independently so a
/// missing CPAL default still matches the enumerated device name.
#[cfg(target_os = "macos")]
fn macos_default_name() -> Option<String> {
    use core_foundation_sys::string::{
        CFStringGetCString, CFStringGetCStringPtr, CFStringRef, kCFStringEncodingUTF8,
    };
    use coreaudio_sys::{
        AudioDeviceID, AudioObjectGetPropertyData, AudioObjectPropertyAddress,
        kAudioDevicePropertyDeviceNameCFString, kAudioDevicePropertyScopeOutput,
        kAudioHardwareNoError, kAudioHardwarePropertyDefaultOutputDevice,
        kAudioObjectPropertyElementMaster, kAudioObjectPropertyScopeGlobal,
        kAudioObjectSystemObject,
    };
    use std::ffi::CStr;
    use std::os::raw::c_char;
    use std::ptr::null;

    unsafe {
        let property_address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };
        let audio_device_id: AudioDeviceID = 0;
        let data_size = std::mem::size_of::<AudioDeviceID>();
        let status = AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &property_address as *const _,
            0,
            null(),
            &data_size as *const _ as *mut _,
            &audio_device_id as *const _ as *mut _,
        );
        if status != kAudioHardwareNoError as i32 || audio_device_id == 0 {
            return None;
        }

        let name_address = AudioObjectPropertyAddress {
            mSelector: kAudioDevicePropertyDeviceNameCFString,
            mScope: kAudioDevicePropertyScopeOutput,
            mElement: kAudioObjectPropertyElementMaster,
        };
        let device_name: CFStringRef = null();
        let name_size = std::mem::size_of::<CFStringRef>();
        let status = AudioObjectGetPropertyData(
            audio_device_id,
            &name_address as *const _,
            0,
            null(),
            &name_size as *const _ as *mut _,
            &device_name as *const _ as *mut _,
        );
        if status != kAudioHardwareNoError as i32 || device_name.is_null() {
            return None;
        }

        let c_string: *const c_char = CFStringGetCStringPtr(device_name, kCFStringEncodingUTF8);
        if !c_string.is_null() {
            return Some(CStr::from_ptr(c_string).to_string_lossy().into_owned());
        }
        let mut buf = [0i8; 255];
        if CFStringGetCString(
            device_name,
            buf.as_mut_ptr(),
            buf.len() as _,
            kCFStringEncodingUTF8,
        ) == 0
        {
            return None;
        }
        CStr::from_ptr(buf.as_ptr())
            .to_str()
            .ok()
            .map(str::to_owned)
    }
}

/// Pulse/PipeWire default sink plus ALSA bridge PCM names CPAL enumerates.
#[cfg(target_os = "linux")]
fn linux_default_names() -> Vec<String> {
    let mut names = Vec::new();
    if let Some(sink) = pactl_default_sink() {
        names.push(sink);
    }
    for alias in ["pulse", "pipewire"] {
        if !names.iter().any(|n| n == alias) {
            names.push(alias.to_string());
        }
    }
    names
}

#[cfg(target_os = "linux")]
fn pactl_default_sink() -> Option<String> {
    let output = std::process::Command::new("pactl")
        .arg("get-default-sink")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() { None } else { Some(name) }
}
