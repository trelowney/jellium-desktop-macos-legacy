//! Certificate authorities for mpv's HTTPS connections.
//!
//! ffmpeg talks TLS through OpenSSL, which cannot read the macOS keychain and
//! has no CA file of its own on a user's Mac, so every HTTPS stream failed with
//! "certificate verify failed". mpv is pointed at a PEM file instead: the
//! roots macOS trusts (system, admin and user trust settings, so a CA the user
//! added in Keychain Access works too), written to the cache directory. Until
//! that file is ready, and if reading the keychain fails, macOS's own bundle of
//! public roots at `/etc/ssl/cert.pem` is used.

use std::ffi::CString;
use std::path::Path;

use base64::Engine;

const SYSTEM_BUNDLE: &str = "/etc/ssl/cert.pem";
const OPTION: &str = "tls-ca-file";

/// Everything runs on a side thread: reading an mpv property can block while
/// mpv's VO still waits for the main thread at startup.
pub(crate) fn install() {
    let spawned = std::thread::Builder::new()
        .name("tls-roots".into())
        .spawn(|| {
            // A tls-ca-file set in the user's mpv.conf wins.
            if !current().is_empty() {
                tracing::info!(target: "Main", "TLS roots: keeping {OPTION} from mpv.conf");
                return;
            }
            if Path::new(SYSTEM_BUNDLE).is_file() {
                set(SYSTEM_BUNDLE);
            }
            let started = std::time::Instant::now();
            let result = rustls_native_certs::load_native_certs();
            for error in &result.errors {
                tracing::warn!(target: "Main", "TLS roots: {error}");
            }
            if result.certs.is_empty() {
                tracing::warn!(target: "Main", "TLS roots: keychain gave no roots, using {SYSTEM_BUNDLE}");
                return;
            }
            let path = jfn_paths::cache_dir().join("tls-roots.pem");
            if let Err(error) = jfn_paths::write_atomic(&path, to_pem(&result.certs).as_bytes()) {
                tracing::warn!(target: "Main", "TLS roots: writing {}: {error}", path.display());
                return;
            }
            set(&path.to_string_lossy());
            tracing::info!(
                target: "Main",
                "TLS roots: {} from the keychain in {} ms",
                result.certs.len(),
                started.elapsed().as_millis()
            );
        });
    if let Err(error) = spawned {
        tracing::warn!(target: "Main", "TLS roots: {error}");
    }
}

fn to_pem<C: AsRef<[u8]>>(certs: &[C]) -> String {
    let mut pem = String::new();
    for cert in certs {
        let b64 = base64::engine::general_purpose::STANDARD.encode(cert.as_ref());
        pem.push_str("-----BEGIN CERTIFICATE-----\n");
        for line in b64.as_bytes().chunks(64) {
            // base64 output is ASCII, so every chunk is valid UTF-8.
            pem.push_str(std::str::from_utf8(line).unwrap_or_default());
            pem.push('\n');
        }
        pem.push_str("-----END CERTIFICATE-----\n");
    }
    pem
}

fn current() -> String {
    let name = CString::new(OPTION).unwrap_or_default();
    let p = unsafe { jfn_mpv::api::jfn_mpv_get_property_string(name.as_ptr()) };
    if p.is_null() {
        return String::new();
    }
    let value = unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned();
    unsafe { jfn_mpv::api::jfn_mpv_free_string(p) };
    value
}

fn set(path: &str) {
    let (Ok(name), Ok(value)) = (CString::new(OPTION), CString::new(path)) else {
        return;
    };
    unsafe { jfn_mpv::api::jfn_mpv_set_property_string_async(name.as_ptr(), value.as_ptr()) };
}

#[cfg(test)]
mod tests {
    use super::to_pem;

    #[test]
    fn pem_wraps_at_64_columns() {
        let pem = to_pem(&[vec![0u8; 60]]);
        let lines: Vec<&str> = pem.lines().collect();
        assert_eq!(lines.first(), Some(&"-----BEGIN CERTIFICATE-----"));
        assert_eq!(lines.last(), Some(&"-----END CERTIFICATE-----"));
        assert_eq!(lines[1].len(), 64);
        assert_eq!(lines[2].len(), 16);
    }
}
