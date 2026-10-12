//! Which video codecs this Mac can decode in hardware, asked of VideoToolbox.

/// `kCMVideoCodecType_*` four-character codes for the codecs that are worth
/// asking about, keyed by their ffmpeg/mpv decoder name.
fn codec_type(codec: &str) -> Option<u32> {
    let fourcc = match codec {
        "h264" => b"avc1",
        "hevc" => b"hvc1",
        "vp9" => b"vp09",
        "av1" => b"av01",
        _ => return None,
    };
    Some(u32::from_be_bytes(*fourcc))
}

#[link(name = "VideoToolbox", kind = "framework")]
unsafe extern "C" {
    fn VTIsHardwareDecodeSupported(codec_type: u32) -> u8;
    // VP9 and AV1 decoders are "supplemental": they only show up in
    // VTIsHardwareDecodeSupported after being registered (macOS 11+).
    fn VTRegisterSupplementalVideoDecoderIfAvailable(codec_type: u32);
}

/// True when VideoToolbox has a hardware decoder for `codec`.
pub(crate) fn hardware_decodable(codec: &str) -> bool {
    let Some(ty) = codec_type(codec) else {
        return false;
    };
    unsafe {
        if matches!(codec, "vp9" | "av1") {
            VTRegisterSupplementalVideoDecoderIfAvailable(ty);
        }
        VTIsHardwareDecodeSupported(ty) != 0
    }
}
