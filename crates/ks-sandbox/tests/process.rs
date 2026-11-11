//! Separate-process smoke test for the `ks-sandbox` binary (S-3a): stdin framing, exit codes,
//! and that a hostile guest kills neither the child's protocol nor the parent. Runs on every CI
//! OS. OS isolation of the child (LPAC/Job, App Sandbox) is S-3b/S-3c and is NOT tested here.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ks_sandbox_proto::{HostRequest, ProtoText, RequestId, SlotPermissions, framing};

const BIN: &str = env!("CARGO_BIN_EXE_ks-sandbox");

fn req_frame(id: u64) -> Vec<u8> {
    HostRequest {
        v: 1,
        request_id: RequestId(id),
        permissions: SlotPermissions::default(),
        input: Some(ProtoText::new("ctx".into())),
    }
    .to_frame()
    .unwrap()
}

fn run_child(stdin: &[u8]) -> (i32, Vec<u8>, Vec<u8>, Duration) {
    let t = Instant::now();
    let mut child = Command::new(BIN)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .spawn()
        .expect("spawn ks-sandbox");
    // The child may exit before reading everything (e.g. bad length prefix); ignore EPIPE.
    let _ = child.stdin.take().unwrap().write_all(stdin);
    let out = child.wait_with_output().expect("wait");
    (
        out.status.code().unwrap_or(-1),
        out.stdout,
        out.stderr,
        t.elapsed(),
    )
}

fn input(wasm: &[u8], req: &[u8]) -> Vec<u8> {
    let mut v = framing::encode(wasm, 64 * 1024 * 1024).unwrap();
    v.extend(framing::encode(req, 256 * 1024).unwrap());
    v
}

fn emit_wat(frame: &str) -> Vec<u8> {
    let escaped: String = frame.bytes().map(|b| format!("\\{b:02x}")).collect();
    wat::parse_str(format!(
        r#"(module (import "ks" "emit" (func $e (param i32 i32))) (memory (export "memory") 1)
  (data (i32.const 64) "{escaped}") (func (export "run") (call $e (i32.const 64) (i32.const {}))))"#,
        frame.len()
    ))
    .unwrap()
}

#[test]
fn child_returns_validated_frame_and_writes_nothing_else() {
    let wasm = emit_wat(r#"{"v":1,"request_id":5,"action":{"type":"insert","text":"ok"}}"#);
    let (code, stdout, stderr, _) = run_child(&input(&wasm, &req_frame(5)));
    assert_eq!(code, 0);
    assert!(stderr.is_empty());
    let body = framing::decode(&stdout, 64 * 1024).expect("one framed result");
    assert!(
        ks_sandbox_proto::parse_guest_frame(body, RequestId(5), SlotPermissions::default()).is_ok()
    );
}

#[test]
fn child_rejected_frame_produces_no_output() {
    // Valid JSON, wrong request id: exit 15 (FrameRejected), empty stdout and stderr.
    let wasm =
        emit_wat(r#"{"v":1,"request_id":6,"action":{"type":"insert","text":"SECRET_MARKER"}}"#);
    let (code, stdout, stderr, _) = run_child(&input(&wasm, &req_frame(5)));
    assert_eq!(code, 15);
    assert!(stdout.is_empty() && stderr.is_empty());
}

#[test]
fn child_infinite_loop_times_out() {
    let wasm = wat::parse_str(
        r#"(module (memory (export "memory") 1) (func (export "run") (loop $l (br $l))))"#,
    )
    .unwrap();
    let (code, stdout, _, wall) = run_child(&input(&wasm, &req_frame(1)));
    assert_eq!(code, 10);
    assert!(stdout.is_empty());
    // Process start + compile + 2 s deadline; generous bound for loaded CI runners.
    assert!(wall < Duration::from_secs(10), "{wall:?}");
}

#[test]
fn child_rejects_wasi_module_and_bad_framing() {
    let wasi = wat::parse_str(
        r#"(module (import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))
  (memory (export "memory") 1) (func (export "run")))"#,
    )
    .unwrap();
    assert_eq!(run_child(&input(&wasi, &req_frame(1))).0, 20);
    // Length prefix beyond the module cap, then truncated body, then empty stdin.
    assert_eq!(run_child(&u32::MAX.to_le_bytes()).0, 21);
    assert_eq!(run_child(&[10, 0, 0, 0, 1, 2]).0, 21);
    assert_eq!(run_child(&[]).0, 21);
    // Oversized host request prefix (256 KiB + 1).
    let mut v = framing::encode(&wasi, 1 << 26).unwrap();
    v.extend(((256 * 1024 + 1) as u32).to_le_bytes());
    assert_eq!(run_child(&v).0, 21);
}
