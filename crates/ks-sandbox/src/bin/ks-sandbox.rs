//! `ks-sandbox` process skeleton (S-3a). OS isolation (LPAC/Job, App Sandbox) is S-3b/S-3c.
//!
//! stdin:  [u32 LE len][wasm module][u32 LE len][HostRequest JSON]
//! stdout: on success only, [u32 LE len][GuestMessage JSON] (already validated by the host).
//! exit:   `OutcomeCode::exit_code()`; 20 = module rejected, 21 = stdin framing error.
//!
//! Writes nothing else to stdout/stderr: no logging of script output or user text.
//! In the product the guest modules are embedded (`include_bytes!` + SHA-256, SR-57); taking
//! the module over stdin is a spike convenience for the cross-OS process test.

use std::io::{Read, Write};
use std::process::ExitCode;

use ks_sandbox::{Outcome, Sandbox, SandboxConfig};
use ks_sandbox_proto::{
    GuestMessage, HostRequest, MAX_HOST_FRAME_BYTES, PROTOCOL_VERSION, framing,
};

/// Upper bound for a guest module received over stdin.
const MAX_MODULE_BYTES: usize = 64 * 1024 * 1024;

fn read_frame(input: &mut impl Read, max: usize) -> Option<Vec<u8>> {
    let mut header = [0u8; framing::HEADER_LEN];
    input.read_exact(&mut header).ok()?;
    let len = framing::decode_len(header, max).ok()?;
    let mut body = vec![0u8; len];
    input.read_exact(&mut body).ok()?;
    Some(body)
}

fn exit(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(u8::MAX))
}

fn main() -> ExitCode {
    let mut stdin = std::io::stdin().lock();
    let Some(wasm) = read_frame(&mut stdin, MAX_MODULE_BYTES) else {
        return exit(21);
    };
    let Some(request) = read_frame(&mut stdin, MAX_HOST_FRAME_BYTES) else {
        return exit(21);
    };
    let Ok(request) = HostRequest::from_frame(&request) else {
        return exit(21);
    };
    let Ok(mut sandbox) = Sandbox::new(SandboxConfig::default()) else {
        return exit(19);
    };
    let Ok(module) = sandbox.compile(&wasm) else {
        return exit(20);
    };
    let report = sandbox.run(&module, &request);
    let code = report.outcome.code().exit_code();
    if let Outcome::Ok(action) = report.outcome {
        let msg = GuestMessage {
            v: PROTOCOL_VERSION,
            request_id: request.request_id,
            action: action.into_action(),
        };
        let Ok(out) = msg.to_frame().and_then(|b| framing::encode(&b, b.len())) else {
            return exit(19);
        };
        let mut stdout = std::io::stdout().lock();
        if stdout
            .write_all(&out)
            .and_then(|()| stdout.flush())
            .is_err()
        {
            return exit(19);
        }
    }
    exit(code)
}
