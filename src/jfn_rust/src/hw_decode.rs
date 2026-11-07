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
fn hardware_decodable(codec: &str) -> bool {
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

/// How long startup waits for VideoToolbox. On the first launch after a boot or
/// an update it can block for a long time, which froze the app at startup.
const QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// The `codecs` this Mac decodes in hardware, asked on a helper thread so a
/// slow VideoToolbox can't hold up startup. When it doesn't answer within
/// [`QUERY_TIMEOUT`], none of them count as hardware-decodable: the server
/// converts them for this session, which plays everywhere.
pub(crate) fn hardware_decodable_codecs(codecs: &[&'static str]) -> Vec<&'static str> {
    let started = std::time::Instant::now();
    let (tx, rx) = std::sync::mpsc::channel();
    let codecs = codecs.to_vec();
    let spawned = std::thread::Builder::new()
        .name("vt-query".into())
        .spawn(move || {
            let found: Vec<&'static str> =
                codecs.into_iter().filter(|c| hardware_decodable(c)).collect();
            let _ = tx.send(found);
        });
    if let Err(error) = spawned {
        tracing::warn!(target: "Main", "VideoToolbox query: {error}");
        return Vec::new();
    }
    match rx.recv_timeout(QUERY_TIMEOUT) {
        Ok(found) => {
            tracing::info!(
                target: "Main",
                "VideoToolbox answered in {} ms",
                started.elapsed().as_millis()
            );
            found
        }
        Err(_) => {
            tracing::warn!(
                target: "Main",
                "VideoToolbox did not answer in {} s; the server converts those codecs this session",
                QUERY_TIMEOUT.as_secs()
            );
            Vec::new()
        }
    }
}
