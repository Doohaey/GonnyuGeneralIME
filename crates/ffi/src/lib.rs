use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use gannyu_input_core::{default_region_entry, load_region_from_manifest, InputPipeline};
use parking_lot::Mutex;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::fs;
use std::io;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::ptr;

mod obfstr;
use obfstr::{obfbytes, obfstr};

mod antidebug;
mod whitebox;

include!(concat!(env!("OUT_DIR"), "/embedded_resources.rs"));
include!(concat!(env!("OUT_DIR"), "/integrity.rs"));

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn 文件头标记() -> [u8; 4] {
    obfbytes!(b"GNYE")
}

/// Verify the whole-set integrity HMAC over all embedded resource blobs.
///
/// The integrity key is derived from the master key (mirroring build.rs). If
/// any blob was swapped or modified, the recomputed tag will not match the
/// embedded `RESOURCE_INTEGRITY_TAG` and the loader refuses to proceed.
fn verify_resource_integrity() -> bool {
    let key = whitebox::derive_master_key();
    let mut integrity_key = [0u8; 32];
    for i in 0..32 {
        integrity_key[i] = key[i] ^ 0xa5;
    }
    let mut mac = match <HmacSha256 as Mac>::new_from_slice(&integrity_key) {
        Ok(m) => m,
        Err(_) => return false,
    };
    for path in embedded_resource_paths() {
        if let Some(blob) = embedded_resource(path) {
            mac.update(blob);
        }
    }
    let tag = mac.finalize().into_bytes();
    // Constant-time comparison to avoid leaking timing information.
    let expected = RESOURCE_INTEGRITY_TAG;
    let mut diff = 0u8;
    for i in 0..32 {
        diff |= tag[i] ^ expected[i];
    }
    diff == 0
}

pub struct GannyuPipelineHandle {
    pipeline: Mutex<InputPipeline>,
    /// Temp directory holding decrypted resources — cleaned up on destroy
    _temp_dir: Option<tempfile::TempDir>,
    /// Open file descriptors for unlinked decrypted resources (Linux; empty
    /// elsewhere). Kept alive so the /proc/self/fd/N symlinks remain valid for
    /// the pipeline's lifetime.
    _resource_fds: Vec<std::fs::File>,
    manifest_path: Option<String>,
    region_id: Option<String>,
}

const STATUS_OK: c_int = 0;
const STATUS_INVALID_ARGUMENT: c_int = 1;
const STATUS_LOAD_FAILURE: c_int = 2;
const STATUS_SERIALIZE_FAILURE: c_int = 3;

thread_local! {
    static LAST_ERROR: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn clear_last_error() {
    LAST_ERROR.with(|slot| *slot.borrow_mut() = None);
}

fn set_last_error(message: impl Into<String>) {
    LAST_ERROR.with(|slot| *slot.borrow_mut() = Some(message.into()));
}

fn take_last_error() -> Option<String> {
    LAST_ERROR.with(|slot| slot.borrow_mut().take())
}

fn load_failure(stage: &str, error: impl std::fmt::Display) -> c_int {
    set_last_error(format!("{stage}: {error}"));
    STATUS_LOAD_FAILURE
}

#[no_mangle]
pub unsafe extern "C" fn gannyu_last_error(out_error: *mut *mut c_char) -> c_int {
    if out_error.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    *out_error = ptr::null_mut();
    let Some(message) = take_last_error() else {
        return STATUS_OK;
    };
    match CString::new(message) {
        Ok(value) => {
            *out_error = value.into_raw();
            STATUS_OK
        }
        Err(_) => STATUS_SERIALIZE_FAILURE,
    }
}

unsafe fn cstr_to_str<'a>(value: *const c_char) -> Option<&'a str> {
    if value.is_null() {
        return None;
    }
    CStr::from_ptr(value).to_str().ok()
}

fn decrypt_to_file(
    加密器: &XChaCha20Poly1305,
    data: &[u8],
    output: &std::path::Path,
) -> io::Result<()> {
    if data.len() >= 4 && data[0..4] != 文件头标记() {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        return fs::write(output, data);
    }

    if data.len() < 28 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            obfstr!("blob too short"),
        ));
    }

    let 种子 = XNonce::from_slice(&data[4..28]);
    let 密文 = &data[28..];
    let plaintext = 加密器
        .decrypt(种子, 密文)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, obfstr!("decryption failed")))?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output, &plaintext)
}

fn embedded_cipher() -> io::Result<XChaCha20Poly1305> {
    let key = whitebox::derive_master_key();
    XChaCha20Poly1305::new_from_slice(&key)
        .map_err(|_| io::Error::other(obfstr!("invalid XChaCha20-Poly1305 key")))
}

fn deploy_embedded_path(
    加密器: &XChaCha20Poly1305,
    temp: &tempfile::TempDir,
    path: &str,
    fds: &mut Vec<std::fs::File>,
) -> io::Result<()> {
    let Some(blob) = embedded_resource(path) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{}: {path}", obfstr!("missing embedded resource")),
        ));
    };
    let output = temp.path().join(path);
    decrypt_to_file(加密器, blob, &output)?;
    // On Linux, unlink the decrypted file immediately and replace it with a
    // symlink to /proc/self/fd/N. The data lives only in the open fd (never
    // reachable by path), so a same-user process cannot read the plaintext by
    // walking the temp dir. The fd is kept alive by the caller.
    #[cfg(target_os = "linux")]
    {
        let file = std::fs::OpenOptions::new().read(true).open(&output)?;
        let fd = std::os::unix::io::AsRawFd::as_raw_fd(&file);
        std::fs::remove_file(&output)?;
        std::os::unix::fs::symlink(format!("/proc/self/fd/{fd}"), &output)?;
        fds.push(file);
    }
    Ok(())
}

fn path_needed_for_region(path: &str, region_id: &str) -> bool {
    path == obfstr!("manifest.toml")
        || path == obfstr!("fuzzy_scheme.tsv")
        || path.starts_with(obfstr!("frequency/"))
        || path.starts_with(obfstr!("schemas/"))
        || path.starts_with(&format!("{}/{}/", obfstr!("regions"), region_id))
}

/// Holds a deployed (decrypted) resource tree: the temp dir plus the open fds
/// backing unlinked files (Linux; empty elsewhere). Dropping this cleans up both.
///
/// After the pipeline loads, the decrypted files are removed from disk (see
/// `gannyu_pipeline_create`); the `TempDir` handle and open fds are kept only
/// so the backing storage is reclaimed on drop.
struct DeployedResources {
    temp: tempfile::TempDir,
    _fds: Vec<std::fs::File>,
}

fn deploy_embedded_manifest() -> io::Result<DeployedResources> {
    let temp = private_temp_dir()?;
    let 加密器 = embedded_cipher()?;
    let mut fds = Vec::new();
    deploy_embedded_path(&加密器, &temp, obfstr!("manifest.toml"), &mut fds)?;
    Ok(DeployedResources { temp, _fds: fds })
}

/// Create a temp dir whose decrypted resources are readable only by the owning
/// user. `tempfile::TempDir::new()` uses the process umask (typically 0o755,
/// world-readable) on Unix, which would expose the decrypted dictionary data.
fn private_temp_dir() -> io::Result<tempfile::TempDir> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tempfile::Builder::new()
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()
    }
    #[cfg(not(unix))]
    {
        tempfile::TempDir::new()
    }
}

fn deploy_embedded_resources(region_id: Option<&str>) -> io::Result<DeployedResources> {
    let deployed = deploy_embedded_manifest()?;
    let manifest = deployed.temp.path().join(obfstr!("manifest.toml"));
    let selected_region = match region_id.filter(|value| !value.is_empty()) {
        Some(value) => value.to_owned(),
        None => default_region_entry(&manifest)
            .map(|entry| entry.id)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?,
    };
    let 加密器 = embedded_cipher()?;
    let mut paths: Vec<&&str> = embedded_resource_paths()
        .iter()
        .filter(|path| {
            **path != obfstr!("manifest.toml") && path_needed_for_region(path, &selected_region)
        })
        .collect();
    paths.sort_by_key(|path| path.contains(obfstr!("dictionary_runtime_cache")));
    // Keep the manifest fd alive (it backs the unlinked manifest.toml symlink).
    let mut fds = deployed._fds;
    for path in paths {
        deploy_embedded_path(&加密器, &deployed.temp, path, &mut fds)?;
    }
    Ok(DeployedResources {
        temp: deployed.temp,
        _fds: fds,
    })
}

#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_create(
    manifest_path: *const c_char,
    region_id: *const c_char,
    out_handle: *mut *mut GannyuPipelineHandle,
) -> c_int {
    clear_last_error();
    pipeline_create(manifest_path, region_id, out_handle)
}

unsafe fn pipeline_create(
    manifest_path: *const c_char,
    region_id: *const c_char,
    out_handle: *mut *mut GannyuPipelineHandle,
) -> c_int {
    if out_handle.is_null() {
        set_last_error("invalid output handle");
        return STATUS_INVALID_ARGUMENT;
    }
    *out_handle = ptr::null_mut();

    // Anti-debug / anti-Frida: refuse to deploy resources when a debugger or
    // tracer is attached. This blocks the easiest dynamic-analysis shortcuts.
    if antidebug::debugger_present() {
        set_last_error("debugger or tracer detected");
        return STATUS_LOAD_FAILURE;
    }

    // Whole-set integrity check: refuse to load if any embedded blob was
    // swapped or modified.
    if !embedded_resource_paths().is_empty() && !verify_resource_integrity() {
        set_last_error("embedded resource integrity check failed");
        return STATUS_LOAD_FAILURE;
    }

    let has_embedded = !embedded_resource_paths().is_empty();
    let requested_manifest = cstr_to_str(manifest_path).map(str::to_owned);
    let caller_manifest = requested_manifest.as_ref().map(PathBuf::from);
    let requested_region = cstr_to_str(region_id).filter(|value| !value.is_empty());
    let requested_region_id = requested_region.map(str::to_owned);

    // Determine manifest path and optional temp dir:
    // 1. Caller-provided valid manifest → use filesystem (dev mode)
    // 2. Embedded resources available → deploy to temp, use embedded manifest
    // 3. Neither → error
    let (deployed, manifest) = match (&caller_manifest, has_embedded) {
        (Some(p), _) if p.exists() => (None, p.clone()),
        (_, true) => {
            let deployed = match deploy_embedded_resources(requested_region) {
                Ok(t) => t,
                Err(error) => return load_failure("embedded resource deployment failed", error),
            };
            let m = deployed.temp.path().join(obfstr!("manifest.toml"));
            (Some(deployed), m)
        }
        (Some(p), false) => (None, p.clone()),
        (None, false) => {
            set_last_error("no embedded resources or manifest path available");
            return STATUS_INVALID_ARGUMENT;
        },
    };

    let resource = match requested_region {
        Some(value) => match load_region_from_manifest(&manifest, value) {
            Ok(resource) => resource,
            Err(error) => return load_failure("region manifest load failed", error),
        },
        None => match default_region_entry(&manifest) {
            Ok(entry) => match load_region_from_manifest(&manifest, &entry.id) {
                Ok(resource) => resource,
                Err(error) => return load_failure("default region manifest load failed", error),
            },
            Err(error) => return load_failure("default region selection failed", error),
        },
    };
    let pipeline = match InputPipeline::load(&resource) {
        Ok(value) => value,
        Err(error) => return load_failure("dictionary pipeline load failed", error),
    };
    // The pipeline has now loaded all dictionary/slang/hints data into memory.
    if let Some(d) = &deployed {
        let _ = fs::remove_dir_all(d.temp.path());
    }
    let (temp_dir, resource_fds) = match deployed {
        Some(d) => (Some(d.temp), d._fds),
        None => (None, Vec::new()),
    };
    let handle = Box::new(GannyuPipelineHandle {
        pipeline: Mutex::new(pipeline),
        _temp_dir: temp_dir,
        _resource_fds: resource_fds,
        manifest_path: requested_manifest,
        region_id: requested_region_id,
    });
    *out_handle = Box::into_raw(handle);
    STATUS_OK
}

#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_compose(
    handle: *mut GannyuPipelineHandle,
    input: *const c_char,
    out_json: *mut *mut c_char,
) -> c_int {
    if handle.is_null() || out_json.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    *out_json = ptr::null_mut();
    let Some(text) = cstr_to_str(input) else {
        return STATUS_INVALID_ARGUMENT;
    };
    let candidates = (*handle).pipeline.lock().compose(text);
    let serialized = match serde_json::to_string(&candidates) {
        Ok(value) => value,
        Err(_) => return STATUS_SERIALIZE_FAILURE,
    };
    match CString::new(serialized) {
        Ok(value) => {
            *out_json = value.into_raw();
            STATUS_OK
        }
        Err(_) => STATUS_SERIALIZE_FAILURE,
    }
}

/// Build the layered selection list (Gan reading, fuzzy, Mandarin pinyin,
/// Mandarin word) for an input string and return it as JSON.
#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_retrieve(
    handle: *mut GannyuPipelineHandle,
    input: *const c_char,
    out_json: *mut *mut c_char,
) -> c_int {
    if handle.is_null() || out_json.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    *out_json = ptr::null_mut();
    let Some(text) = cstr_to_str(input) else {
        return STATUS_INVALID_ARGUMENT;
    };
    let candidates = (*handle).pipeline.lock().retrieve(text);
    let serialized = match serde_json::to_string(&candidates) {
        Ok(value) => value,
        Err(_) => return STATUS_SERIALIZE_FAILURE,
    };
    match CString::new(serialized) {
        Ok(value) => {
            *out_json = value.into_raw();
            STATUS_OK
        }
        Err(_) => STATUS_SERIALIZE_FAILURE,
    }
}

/// Segment a continuous Latin string into word boundaries and return
/// candidates per segment as JSON `[[candidate,...],[candidate,...],...]`.
#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_segment_sentence(
    handle: *mut GannyuPipelineHandle,
    input: *const c_char,
    out_json: *mut *mut c_char,
) -> c_int {
    if handle.is_null() || out_json.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    *out_json = ptr::null_mut();
    let Some(text) = cstr_to_str(input) else {
        return STATUS_INVALID_ARGUMENT;
    };
    let segments = (*handle).pipeline.lock().segment_sentence(text);
    let serialized = match serde_json::to_string(&segments) {
        Ok(value) => value,
        Err(_) => return STATUS_SERIALIZE_FAILURE,
    };
    match CString::new(serialized) {
        Ok(value) => {
            *out_json = value.into_raw();
            STATUS_OK
        }
        Err(_) => STATUS_SERIALIZE_FAILURE,
    }
}

/// Return segmentation boundary positions (byte offsets) for a continuous
/// Latin string as a JSON array of integers, e.g. `[7]` for "lancongwa"
/// (boundary between "lancong" and "wa").
#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_segment_boundaries(
    handle: *mut GannyuPipelineHandle,
    input: *const c_char,
    out_json: *mut *mut c_char,
) -> c_int {
    if handle.is_null() || out_json.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    *out_json = ptr::null_mut();
    let Some(text) = cstr_to_str(input) else {
        return STATUS_INVALID_ARGUMENT;
    };
    let boundaries = (*handle).pipeline.lock().segment_boundaries(text);
    let serialized = match serde_json::to_string(&boundaries) {
        Ok(value) => value,
        Err(_) => return STATUS_SERIALIZE_FAILURE,
    };
    match CString::new(serialized) {
        Ok(value) => {
            *out_json = value.into_raw();
            STATUS_OK
        }
        Err(_) => STATUS_SERIALIZE_FAILURE,
    }
}

/// Format preedit text with the shared Fcitx5-compatible display rule. The
/// returned string is presentation only; `input` remains the retrieval input.
#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_format_preedit(
    handle: *mut GannyuPipelineHandle,
    input: *const c_char,
    consumed_bytes: usize,
    out_text: *mut *mut c_char,
) -> c_int {
    if handle.is_null() || out_text.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    *out_text = ptr::null_mut();
    let Some(text) = cstr_to_str(input) else {
        return STATUS_INVALID_ARGUMENT;
    };
    let display = (*handle)
        .pipeline
        .lock()
        .format_preedit_display(text, consumed_bytes);
    match CString::new(display) {
        Ok(value) => {
            *out_text = value.into_raw();
            STATUS_OK
        }
        Err(_) => STATUS_SERIALIZE_FAILURE,
    }
}

/// Add a user-contributed word to the persistent user dictionary.
/// `headword` is the Chinese text (e.g. "南昌话"), `pinyin` is the
/// dialect reading (e.g. "lan4 cong1 wa5"), `mandarin_pinyin` is the
/// Mandarin reading (e.g. "nan2 chang1 hua4").
/// Returns JSON `{"ok":true}` or `{"ok":false}` if `out_json` is non-null.
#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_entry_count(handle: *mut GannyuPipelineHandle) -> c_int {
    if handle.is_null() {
        return -1;
    }
    (*handle).pipeline.lock().entry_count() as c_int
}

#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_user_dict_add(
    handle: *mut GannyuPipelineHandle,
    headword: *const c_char,
    pinyin: *const c_char,
    mandarin_pinyin: *const c_char,
    out_json: *mut *mut c_char,
) -> c_int {
    if handle.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    let Some(hw) = cstr_to_str(headword) else {
        return STATUS_INVALID_ARGUMENT;
    };
    let Some(py) = cstr_to_str(pinyin) else {
        return STATUS_INVALID_ARGUMENT;
    };
    let mp = cstr_to_str(mandarin_pinyin).unwrap_or("");
    let ok = (*handle).pipeline.lock().add_user_word(hw, py, mp);
    if !out_json.is_null() {
        let result = format!(r#"{{"ok":{}}}"#, ok);
        match CString::new(result) {
            Ok(value) => {
                *out_json = value.into_raw();
            }
            Err(_) => return STATUS_SERIALIZE_FAILURE,
        }
    }
    STATUS_OK
}

#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_user_dict_boost(
    handle: *mut GannyuPipelineHandle,
    headword: *const c_char,
    out_json: *mut *mut c_char,
) -> c_int {
    if handle.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    let Some(hw) = cstr_to_str(headword) else {
        return STATUS_INVALID_ARGUMENT;
    };
    let ok = (*handle).pipeline.lock().boost_frequency(hw);
    if !out_json.is_null() {
        let result = format!(r#"{{"ok":{}}}"#, ok);
        match CString::new(result) {
            Ok(value) => *out_json = value.into_raw(),
            Err(_) => return STATUS_SERIALIZE_FAILURE,
        }
    }
    STATUS_OK
}

#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_user_data_clear(
    handle: *mut GannyuPipelineHandle,
    scope: c_int,
) -> c_int {
    if handle.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    let (clear_words, clear_frequencies) = match scope {
        1 => (true, false),
        2 => (false, true),
        3 => (true, true),
        _ => return STATUS_INVALID_ARGUMENT,
    };
    let current = &mut *handle;
    if !current
        .pipeline
        .lock()
        .clear_user_data(clear_words, clear_frequencies)
    {
        return STATUS_LOAD_FAILURE;
    }
    let manifest = current
        .manifest_path
        .as_ref()
        .map(|value| CString::new(value.as_str()).unwrap());
    let region = current
        .region_id
        .as_ref()
        .map(|value| CString::new(value.as_str()).unwrap());
    let mut replacement = ptr::null_mut();
    let status = gannyu_pipeline_create(
        manifest
            .as_ref()
            .map_or(ptr::null(), |value| value.as_ptr()),
        region.as_ref().map_or(ptr::null(), |value| value.as_ptr()),
        &mut replacement,
    );
    if status != STATUS_OK || replacement.is_null() {
        return if status == STATUS_OK {
            STATUS_LOAD_FAILURE
        } else {
            status
        };
    }
    let replacement = *Box::from_raw(replacement);
    *current.pipeline.lock() = replacement.pipeline.into_inner();
    current._temp_dir = replacement._temp_dir;
    current._resource_fds = replacement._resource_fds;
    current.manifest_path = replacement.manifest_path;
    current.region_id = replacement.region_id;
    STATUS_OK
}

#[no_mangle]
pub unsafe extern "C" fn gannyu_region_list(
    manifest_path: *const c_char,
    out_json: *mut *mut c_char,
) -> c_int {
    if out_json.is_null() {
        return STATUS_INVALID_ARGUMENT;
    }
    *out_json = ptr::null_mut();

    let mut _embedded_temp = None;
    let mut manifest = match cstr_to_str(manifest_path) {
        Some(m) => PathBuf::from(m),
        None => PathBuf::new(),
    };

    if !manifest.exists() {
        if embedded_resource_paths().is_empty() {
            return STATUS_LOAD_FAILURE;
        }
        let deployed = match deploy_embedded_manifest() {
            Ok(d) => d,
            Err(_) => return STATUS_LOAD_FAILURE,
        };
        manifest = deployed.temp.path().join(obfstr!("manifest.toml"));
        _embedded_temp = Some(deployed);
    }

    let entries = match gannyu_input_core::list_region_entries(&manifest) {
        Ok(value) => value,
        Err(_) => return STATUS_LOAD_FAILURE,
    };
    let serialized = match serde_json::to_string(&entries) {
        Ok(value) => value,
        Err(_) => return STATUS_SERIALIZE_FAILURE,
    };
    match CString::new(serialized) {
        Ok(value) => {
            *out_json = value.into_raw();
            STATUS_OK
        }
        Err(_) => STATUS_SERIALIZE_FAILURE,
    }
}

#[no_mangle]
pub unsafe extern "C" fn gannyu_pipeline_destroy(handle: *mut GannyuPipelineHandle) {
    if handle.is_null() {
        return;
    }
    drop(Box::from_raw(handle));
}

#[no_mangle]
pub unsafe extern "C" fn gannyu_string_destroy(value: *mut c_char) {
    if value.is_null() {
        return;
    }
    drop(CString::from_raw(value));
}

#[no_mangle]
pub extern "C" fn gannyu_ffi_status_ok() -> c_int {
    STATUS_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_check_passes_on_unmodified_resources() {
        if !embedded_resource_paths().is_empty() {
            assert!(
                verify_resource_integrity(),
                "unmodified embedded resources must pass integrity check"
            );
        }
    }

    #[test]
    fn integrity_check_detects_tampering() {
        if embedded_resource_paths().is_empty() {
            return;
        }
        let first = embedded_resource_paths()[0];
        let blob = embedded_resource(first).unwrap();
        let mut tampered = blob.to_vec();
        tampered[0] ^= 0xff;

        let key = whitebox::derive_master_key();
        let mut integrity_key = [0u8; 32];
        for i in 0..32 {
            integrity_key[i] = key[i] ^ 0xa5;
        }
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&integrity_key).unwrap();
        for path in embedded_resource_paths() {
            if *path == first {
                mac.update(&tampered);
            } else if let Some(b) = embedded_resource(path) {
                mac.update(b);
            }
        }
        let tag = mac.finalize().into_bytes();
        assert_ne!(
            tag.as_slice(),
            &RESOURCE_INTEGRITY_TAG[..],
            "tampered blob must produce a different HMAC"
        );
    }
    #[test]
    fn last_error_is_consumed_after_reading() {
        clear_last_error();
        set_last_error("resource deployment failed: denied");
        assert_eq!(take_last_error().as_deref(), Some("resource deployment failed: denied"));
        assert_eq!(take_last_error(), None);
    }
}
