//! Native observations only. No IPC flag can establish platform qualification.
use workflow_engine::runs::Admission;
pub fn admission() -> Admission {
    let reason = if cfg!(target_os = "linux") {
        match std::fs::read_to_string("/proc/swaps") {
            Ok(swaps) if swaps.lines().skip(1).any(|s|!s.trim().is_empty()) => "Native paging observation: active swap. Protected host/daemon paging and dump policy is not established.",
            Ok(_) => "Native paging observation: no active host swap. Docker daemon/VM paging, dumps, native secure-store roundtrip and provider-only containment remain unqualified.",
            Err(_) => "Native paging observation unavailable. Secure staging cannot be established.",
        }
    } else {
        "This native platform has no established protected paging/dump and secure-store qualification. Execution withheld."
    };
    Admission{image_id:String::new(),blocker:Some(format!("secureStagingUnavailable: {reason} Trusted-private workload authorization and an independently app-acquired managed session are also required. No login, model call or fallback was attempted."))}
}
