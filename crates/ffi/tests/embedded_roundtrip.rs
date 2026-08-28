use gannyu_input_ffi::{
    gannyu_ffi_status_ok, gannyu_pipeline_compose, gannyu_pipeline_create, gannyu_pipeline_destroy,
    gannyu_pipeline_retrieve, gannyu_string_destroy,
};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

fn cstr(ptr: *mut c_char) -> String {
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

#[test]
fn embedded_resources_load_and_compose() {
    let mut handle: *mut gannyu_input_ffi::GannyuPipelineHandle = std::ptr::null_mut();
    let status = unsafe { gannyu_pipeline_create(std::ptr::null(), std::ptr::null(), &mut handle) };
    assert_eq!(status, gannyu_ffi_status_ok(), "pipeline_create failed");

    let input = CString::new("lancong").unwrap();
    let mut out: *mut c_char = std::ptr::null_mut();
    let status = unsafe { gannyu_pipeline_compose(handle, input.as_ptr(), &mut out) };
    assert_eq!(status, gannyu_ffi_status_ok(), "compose failed");
    let json = cstr(out);
    unsafe { gannyu_string_destroy(out) };
    assert!(!json.is_empty(), "compose returned empty JSON");

    let mut out2: *mut c_char = std::ptr::null_mut();
    let status = unsafe { gannyu_pipeline_retrieve(handle, input.as_ptr(), &mut out2) };
    assert_eq!(status, gannyu_ffi_status_ok(), "retrieve failed");
    let json2 = cstr(out2);
    unsafe { gannyu_string_destroy(out2) };
    assert!(!json2.is_empty(), "retrieve returned empty JSON");

    let handle_addr = handle as usize;
    let workers: Vec<_> = (0..4)
        .map(|_| {
            std::thread::spawn(move || {
                let input = CString::new("lancong").unwrap();
                let mut out: *mut c_char = std::ptr::null_mut();
                let status = unsafe {
                    gannyu_pipeline_retrieve(
                        handle_addr as *mut gannyu_input_ffi::GannyuPipelineHandle,
                        input.as_ptr(),
                        &mut out,
                    )
                };
                assert_eq!(status, gannyu_ffi_status_ok());
                assert!(!out.is_null());
                unsafe { gannyu_string_destroy(out) };
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }

    unsafe { gannyu_pipeline_destroy(handle) };
}
