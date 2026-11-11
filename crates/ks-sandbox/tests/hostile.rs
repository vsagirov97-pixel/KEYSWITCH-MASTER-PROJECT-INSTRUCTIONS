//! Hostile-module suite, Wasmtime layer (spike S-3a; SR-41, SR-43, SR-44, SR-46, SR-47, SR-51,
//! SR-52; H-05..H-07, H-20, H-22, H-23, H-25..H-28). Guests are hand-written WAT, test data is
//! synthetic. OS-layer cases (H-30..H-40) belong to S-3b/S-3c.
//!
//! Every test asserts a content-free [`OutcomeCode`]; nothing prints guest output.

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use ks_sandbox::{
    GuestModule, OutcomeCode, Sandbox, SandboxConfig, SandboxError, Violation, hardened_config,
    memory_limit_pages,
};
use ks_sandbox_proto::{
    Action, ForbiddenClass, HostRequest, MAX_GUEST_FRAME_BYTES, ProtoError, ProtoText, RequestId,
    SlotPermissions,
};

const ID: RequestId = RequestId(42);
/// Allowed scheduling slack on top of the deadline (task acceptance: deadline + 0.5 s).
const SLACK: Duration = Duration::from_millis(500);

fn sandbox() -> Sandbox {
    Sandbox::new(SandboxConfig::default()).expect("engine")
}

fn request() -> HostRequest {
    HostRequest {
        v: 1,
        request_id: ID,
        permissions: SlotPermissions::default(),
        input: None,
    }
}

fn wasm(wat: &str) -> Vec<u8> {
    wat::parse_str(wat).expect("test WAT must parse")
}

fn compile(sb: &Sandbox, wat: &str) -> Result<GuestModule, SandboxError> {
    sb.compile(&wasm(wat))
}

fn run_wat(wat: &str) -> OutcomeCode {
    let mut sb = sandbox();
    let m = compile(&sb, wat).expect("module admitted");
    sb.run(&m, &request()).outcome.code()
}

/// WAT string literal with every byte hex-escaped.
fn wat_bytes(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3 + 2);
    s.push('"');
    for b in bytes {
        let _ = write!(s, "\\{b:02x}");
    }
    s.push('"');
    s
}

const DATA_AT: usize = 1024;

/// Guest that emits `frame` once (via a data segment) and returns.
fn emit_module(frame: &[u8]) -> String {
    emit_module_len(frame, frame.len())
}

/// Same, but announces `len` to `ks.emit` (may differ from the data length).
fn emit_module_len(frame: &[u8], len: usize) -> String {
    let pages = (DATA_AT + frame.len().max(len)).div_ceil(65_536).max(1);
    format!(
        r#"(module
  (import "ks" "emit" (func $emit (param i32 i32)))
  (memory (export "memory") {pages})
  (data (i32.const {DATA_AT}) {data})
  (func (export "run") (call $emit (i32.const {DATA_AT}) (i32.const {len}))))"#,
        data = wat_bytes(frame)
    )
}

fn guest_frame(action_json: &str) -> Vec<u8> {
    format!(r#"{{"v":1,"request_id":42,"action":{action_json}}}"#).into_bytes()
}

// ------------------------------------------------------------------------------------------
// Valid path
// ------------------------------------------------------------------------------------------

#[test]
fn valid_insert_is_parsed() {
    let mut sb = sandbox();
    let m = compile(
        &sb,
        &emit_module(&guest_frame(r#"{"type":"insert","text":"Привет"}"#)),
    )
    .unwrap();
    let report = sb.run(&m, &request());
    assert_eq!(report.outcome.code(), OutcomeCode::Ok);
    let ks_sandbox::Outcome::Ok(v) = report.outcome else {
        unreachable!()
    };
    match v.into_action() {
        Action::Insert { text } => assert_eq!(text.as_str(), "Привет"),
        _ => panic!("wrong action kind"),
    }
}

#[test]
fn guest_can_read_request_then_emit() {
    // Copies the request into memory at 0, traps if request_len disagrees with request_read.
    let frame = guest_frame(r#"{"type":"none"}"#);
    let wat = format!(
        r#"(module
  (import "ks" "emit" (func $emit (param i32 i32)))
  (import "ks" "request_len" (func $len (result i32)))
  (import "ks" "request_read" (func $read (param i32 i32) (result i32)))
  (memory (export "memory") 2)
  (data (i32.const 70000) {data})
  (func (export "run")
    (if (i32.ne (call $len) (call $read (i32.const 0) (i32.const 65536))) (then unreachable))
    (call $emit (i32.const 70000) (i32.const {n}))))"#,
        data = wat_bytes(&frame),
        n = frame.len()
    );
    let mut sb = sandbox();
    let m = compile(&sb, &wat).unwrap();
    let req = HostRequest {
        input: Some(ProtoText::new("context".repeat(100))),
        ..request()
    };
    assert_eq!(sb.run(&m, &req).outcome.code(), OutcomeCode::Ok);
}

#[test]
fn request_read_with_small_or_oob_buffer_is_violation() {
    let small = r#"(module
  (import "ks" "request_read" (func $read (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "run") (drop (call $read (i32.const 0) (i32.const 1)))))"#;
    assert_eq!(
        run_wat(small),
        OutcomeCode::Violation(Violation::BufferTooSmall)
    );
    let oob = r#"(module
  (import "ks" "request_read" (func $read (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "run") (drop (call $read (i32.const 65530) (i32.const 65536)))))"#;
    assert_eq!(run_wat(oob), OutcomeCode::Violation(Violation::OutOfBounds));
}

// ------------------------------------------------------------------------------------------
// H-20 / SR-52: deadline
// ------------------------------------------------------------------------------------------

fn assert_timeout_in_time(wat: &str) {
    let mut sb = sandbox();
    let deadline = sb.config().deadline;
    assert_eq!(deadline, Duration::from_secs(2));
    let m = compile(&sb, wat).unwrap();
    let t = Instant::now();
    let report = sb.run(&m, &request());
    let wall = t.elapsed();
    assert_eq!(report.outcome.code(), OutcomeCode::Timeout);
    assert!(wall >= deadline, "terminated before the deadline: {wall:?}");
    assert!(wall <= deadline + SLACK, "terminated too late: {wall:?}");
}

#[test]
fn h20_infinite_loop_terminated_by_deadline() {
    assert_timeout_in_time(
        r#"(module (memory (export "memory") 1) (func (export "run") (loop $l (br $l))))"#,
    );
}

#[test]
fn h20_infinite_loop_in_start_function_terminated() {
    // `start` runs during instantiation: the watchdog must already be armed.
    assert_timeout_in_time(
        r#"(module (memory (export "memory") 1)
  (func $s (loop $l (br $l))) (start $s)
  (func (export "run")))"#,
    );
}

#[test]
fn h20_emit_then_loop_discards_frame() {
    let frame = guest_frame(r#"{"type":"insert","text":"x"}"#);
    let wat = format!(
        r#"(module
  (import "ks" "emit" (func $emit (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 1024) {data})
  (func (export "run") (call $emit (i32.const 1024) (i32.const {n})) (loop $l (br $l))))"#,
        data = wat_bytes(&frame),
        n = frame.len()
    );
    assert_timeout_in_time(&wat);
}

#[test]
fn short_deadline_is_honoured() {
    let mut sb = Sandbox::new(SandboxConfig {
        deadline: Duration::from_millis(200),
        ..SandboxConfig::default()
    })
    .unwrap();
    let m = compile(
        &sb,
        r#"(module (memory (export "memory") 1) (func (export "run") (loop $l (br $l))))"#,
    )
    .unwrap();
    let t = Instant::now();
    assert_eq!(sb.run(&m, &request()).outcome.code(), OutcomeCode::Timeout);
    assert!(t.elapsed() <= Duration::from_millis(200) + SLACK);
}

// ------------------------------------------------------------------------------------------
// H-22: deep recursion
// ------------------------------------------------------------------------------------------

#[test]
fn h22_deep_recursion_traps_and_host_survives() {
    let mut sb = sandbox();
    let rec = compile(
        &sb,
        r#"(module (memory (export "memory") 1)
  (func $f (param i64) (result i64) (i64.add (call $f (i64.add (local.get 0) (i64.const 1))) (i64.const 1)))
  (func (export "run") (drop (call $f (i64.const 0)))))"#,
    )
    .unwrap();
    assert_eq!(
        sb.run(&rec, &request()).outcome.code(),
        OutcomeCode::StackOverflow
    );
    // The same host and engine keep working afterwards.
    let ok = compile(&sb, &emit_module(&guest_frame(r#"{"type":"none"}"#))).unwrap();
    assert_eq!(sb.run(&ok, &request()).outcome.code(), OutcomeCode::Ok);
    assert_eq!(
        sb.run(&rec, &request()).outcome.code(),
        OutcomeCode::StackOverflow
    );
}

// ------------------------------------------------------------------------------------------
// H-23 / SR-52: memory and table limits
// ------------------------------------------------------------------------------------------

#[test]
fn h23_memory_grow_loop_denied() {
    assert_eq!(memory_limit_pages(&SandboxConfig::default()), 1024);
    // Grows one page at a time forever; must stop at 64 MiB with a trap, not -1 / OOM.
    let wat = r#"(module (memory (export "memory") 1)
  (func (export "run") (loop $l (drop (memory.grow (i32.const 1))) (br $l))))"#;
    assert_eq!(run_wat(wat), OutcomeCode::ResourceLimit);
}

#[test]
fn h23_single_huge_grow_denied() {
    let wat = r#"(module (memory (export "memory") 1)
  (func (export "run") (drop (memory.grow (i32.const 65535)))))"#;
    assert_eq!(run_wat(wat), OutcomeCode::ResourceLimit);
}

#[test]
fn h23_growth_up_to_limit_allowed() {
    // 1 + 1023 pages = exactly 64 MiB: allowed, then emits.
    let frame = guest_frame(r#"{"type":"none"}"#);
    let wat = format!(
        r#"(module
  (import "ks" "emit" (func $emit (param i32 i32)))
  (memory (export "memory") 1)
  (data (i32.const 1024) {data})
  (func (export "run")
    (if (i32.eq (memory.grow (i32.const 1023)) (i32.const -1)) (then unreachable))
    (call $emit (i32.const 1024) (i32.const {n}))))"#,
        data = wat_bytes(&frame),
        n = frame.len()
    );
    assert_eq!(run_wat(&wat), OutcomeCode::Ok);
}

#[test]
fn h23_huge_initial_memory_denied_at_instantiation() {
    let mut sb = sandbox();
    let m = compile(
        &sb,
        r#"(module (memory (export "memory") 2000) (func (export "run")))"#,
    )
    .unwrap();
    assert_eq!(
        sb.run(&m, &request()).outcome.code(),
        OutcomeCode::ResourceLimit
    );
}

#[test]
fn h23_table_bomb_denied() {
    let wat = r#"(module (memory (export "memory") 1) (table 1 funcref)
  (func (export "run") (drop (table.grow (ref.null func) (i32.const 1000000)))))"#;
    assert_eq!(run_wat(wat), OutcomeCode::ResourceLimit);
}

// ------------------------------------------------------------------------------------------
// H-05 / H-06 / H-07, SR-43: no WASI, only the `ks` namespace
// ------------------------------------------------------------------------------------------

#[test]
fn wasi_env_and_unknown_imports_rejected() {
    let sb = sandbox();
    let imports = [
        // H-07 / H-06 / file access via WASI preview 1.
        r#"(import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))"#,
        r#"(import "wasi_snapshot_preview1" "environ_get" (func (param i32 i32) (result i32)))"#,
        r#"(import "wasi_snapshot_preview1" "path_open" (func (param i32 i32 i32 i32 i32 i64 i64 i32 i32) (result i32)))"#,
        r#"(import "wasi_snapshot_preview1" "proc_exit" (func (param i32)))"#,
        // H-05: sockets.
        r#"(import "wasi_snapshot_preview1" "sock_accept" (func (param i32 i32 i32) (result i32)))"#,
        r#"(import "wasi:sockets/tcp@0.2.0" "connect" (func))"#,
        r#"(import "wasi:http/outgoing-handler@0.2.0" "handle" (func))"#,
        // Emscripten-style `env`.
        r#"(import "env" "system" (func (param i32) (result i32)))"#,
        r#"(import "env" "memory" (memory 1))"#,
        // Unknown names inside `ks`, and allowlisted names with a non-function kind.
        r#"(import "ks" "exec" (func))"#,
        r#"(import "ks" "emit" (global i32))"#,
        r#"(import "ks" "emit" (memory 1))"#,
        r#"(import "KS" "emit" (func (param i32 i32)))"#,
    ];
    for imp in imports {
        // A memory import is the module's only memory (multi-memory is disabled, SR-41).
        let own_memory = if imp.contains("(memory") {
            ""
        } else {
            r#"(memory (export "memory") 1)"#
        };
        let wat = format!(r#"(module {imp} {own_memory} (func (export "run")))"#);
        assert_eq!(
            compile(&sb, &wat).err(),
            Some(SandboxError::ImportDenied),
            "{imp}"
        );
    }
}

#[test]
fn allowlisted_import_with_wrong_signature_fails_instantiation() {
    let mut sb = sandbox();
    let m = compile(
        &sb,
        r#"(module (import "ks" "emit" (func (param i64))) (memory (export "memory") 1) (func (export "run")))"#,
    )
    .unwrap();
    assert_eq!(
        sb.run(&m, &request()).outcome.code(),
        OutcomeCode::InstantiationFailed
    );
}

#[test]
fn module_admission_errors() {
    let sb = sandbox();
    assert_eq!(
        sb.compile(b"\0asm garbage").err(),
        Some(SandboxError::InvalidModule)
    );
    assert_eq!(sb.compile(&[]).err(), Some(SandboxError::InvalidModule));
    assert_eq!(
        compile(&sb, r#"(module (memory (export "memory") 1))"#).err(),
        Some(SandboxError::MissingEntry)
    );
    assert_eq!(
        compile(&sb, r#"(module (func (export "run") (param i32)))"#).err(),
        Some(SandboxError::MissingEntry)
    );
    assert_eq!(
        compile(&sb, r#"(module (memory (export "run") 1))"#).err(),
        Some(SandboxError::MissingEntry)
    );
}

#[test]
fn module_from_another_engine_is_refused() {
    let a = sandbox();
    let mut b = sandbox();
    let m = compile(&a, &emit_module(&guest_frame(r#"{"type":"none"}"#))).unwrap();
    assert_eq!(
        b.run(&m, &request()).outcome.code(),
        OutcomeCode::InstantiationFailed
    );
}

// ------------------------------------------------------------------------------------------
// SR-41: disabled Wasm proposals
// ------------------------------------------------------------------------------------------

#[test]
fn sr41_disabled_proposals_rejected() {
    let sb = sandbox();
    let cases = [
        (
            "threads",
            r#"(module (memory (export "memory") 1 1 shared) (func (export "run")))"#,
        ),
        (
            "memory64",
            r#"(module (memory (export "memory") i64 1) (func (export "run")))"#,
        ),
        (
            "multi-memory",
            r#"(module (memory (export "memory") 1) (memory 1) (func (export "run")))"#,
        ),
        (
            "tail-call",
            r#"(module (memory (export "memory") 1) (func $f (return_call $f)) (func (export "run")))"#,
        ),
        (
            "exceptions",
            r#"(module (memory (export "memory") 1) (tag $t) (func (export "run") (throw $t)))"#,
        ),
        (
            "function-references",
            r#"(module (memory (export "memory") 1) (type $t (func)) (func (export "run") (call_ref $t (ref.null $t))))"#,
        ),
        (
            "gc",
            r#"(module (memory (export "memory") 1) (type $s (struct (field i32))) (func (export "run") (drop (struct.new $s (i32.const 1)))))"#,
        ),
        (
            "relaxed-simd",
            r#"(module (memory (export "memory") 1) (func (export "run") (drop (i32x4.relaxed_trunc_f32x4_s (v128.const i32x4 0 0 0 0)))))"#,
        ),
    ];
    for (name, wat) in cases {
        assert_eq!(
            compile(&sb, wat).err(),
            Some(SandboxError::InvalidModule),
            "{name}"
        );
    }
    // Baseline MVP + bulk memory + SIMD (needed by interpreter builds) still compiles.
    assert!(
        compile(
            &sb,
            r#"(module (memory (export "memory") 1)
  (func (export "run") (memory.fill (i32.const 0) (i32.const 0) (i32.const 16))
    (drop (i32x4.add (v128.const i32x4 1 2 3 4) (v128.const i32x4 1 1 1 1)))))"#
        )
        .is_ok()
    );
    // Config builds deterministically from limits (smoke for the exposed builder).
    let _ = hardened_config(&SandboxConfig::default());
}

// ------------------------------------------------------------------------------------------
// H-25..H-27, SR-44, SR-46, SR-47: the result frame
// ------------------------------------------------------------------------------------------

#[test]
fn h25_oversized_emit_rejected_before_copy() {
    // Announces 64 KiB + 1 (data present in memory): refused by the size check.
    let big = vec![b' '; MAX_GUEST_FRAME_BYTES + 1];
    assert_eq!(
        run_wat(&emit_module(&big)),
        OutcomeCode::Violation(Violation::FrameTooLarge)
    );
    // Announces 2 GiB with nothing behind it: refused on size, never allocated.
    let wat = r#"(module (import "ks" "emit" (func $emit (param i32 i32))) (memory (export "memory") 1)
  (func (export "run") (call $emit (i32.const 0) (i32.const 0x7fffffff))))"#;
    assert_eq!(
        run_wat(wat),
        OutcomeCode::Violation(Violation::FrameTooLarge)
    );
    // Negative length = 4 GiB - 1 as u32.
    let neg = r#"(module (import "ks" "emit" (func $emit (param i32 i32))) (memory (export "memory") 1)
  (func (export "run") (call $emit (i32.const 0) (i32.const -1))))"#;
    assert_eq!(
        run_wat(neg),
        OutcomeCode::Violation(Violation::FrameTooLarge)
    );
}

#[test]
fn h25_text_over_5000_chars_rejected() {
    let text = "a".repeat(5_001);
    let frame = guest_frame(&format!(r#"{{"type":"insert","text":"{text}"}}"#));
    assert_eq!(
        run_wat(&emit_module(&frame)),
        OutcomeCode::FrameRejected(ProtoError::TextTooLong)
    );
    let ok = guest_frame(&format!(
        r#"{{"type":"insert","text":"{}"}}"#,
        "a".repeat(5_000)
    ));
    assert_eq!(run_wat(&emit_module(&ok)), OutcomeCode::Ok);
}

#[test]
fn sr44_out_of_bounds_pointers_are_violations() {
    for (ptr, len) in [
        ("65530", "100"),
        ("-1", "10"),
        ("-2147483648", "65536"),
        ("65536", "1"),
    ] {
        let wat = format!(
            r#"(module (import "ks" "emit" (func $emit (param i32 i32))) (memory (export "memory") 1)
  (func (export "run") (call $emit (i32.const {ptr}) (i32.const {len}))))"#
        );
        assert_eq!(
            run_wat(&wat),
            OutcomeCode::Violation(Violation::OutOfBounds),
            "{ptr}+{len}"
        );
    }
    // Exactly at the end of memory is in bounds (and then malformed, since it is all zeros).
    let edge = r#"(module (import "ks" "emit" (func $emit (param i32 i32))) (memory (export "memory") 1)
  (func (export "run") (call $emit (i32.const 65526) (i32.const 10))))"#;
    assert_eq!(
        run_wat(edge),
        OutcomeCode::FrameRejected(ProtoError::Malformed)
    );
}

#[test]
fn emit_without_exported_memory_is_violation() {
    let wat = r#"(module (import "ks" "emit" (func $emit (param i32 i32))) (memory 1)
  (func (export "run") (call $emit (i32.const 0) (i32.const 1))))"#;
    assert_eq!(run_wat(wat), OutcomeCode::Violation(Violation::NoMemory));
}

#[test]
fn sr46_duplicate_emit_is_violation() {
    let frame = guest_frame(r#"{"type":"none"}"#);
    let wat = format!(
        r#"(module (import "ks" "emit" (func $emit (param i32 i32))) (memory (export "memory") 1)
  (data (i32.const 1024) {data})
  (func (export "run") (call $emit (i32.const 1024) (i32.const {n})) (call $emit (i32.const 1024) (i32.const {n}))))"#,
        data = wat_bytes(&frame),
        n = frame.len()
    );
    assert_eq!(
        run_wat(&wat),
        OutcomeCode::Violation(Violation::DuplicateEmit)
    );
}

#[test]
fn sr46_emit_then_trap_discards_frame() {
    let frame = guest_frame(r#"{"type":"insert","text":"x"}"#);
    let wat = format!(
        r#"(module (import "ks" "emit" (func $emit (param i32 i32))) (memory (export "memory") 1)
  (data (i32.const 1024) {data})
  (func (export "run") (call $emit (i32.const 1024) (i32.const {n})) unreachable))"#,
        data = wat_bytes(&frame),
        n = frame.len()
    );
    assert_eq!(run_wat(&wat), OutcomeCode::Trap);
}

#[test]
fn no_emit_is_no_result() {
    assert_eq!(
        run_wat(r#"(module (memory (export "memory") 1) (func (export "run")))"#),
        OutcomeCode::NoResult
    );
}

#[test]
fn malformed_frames_rejected() {
    let cases: [(&[u8], ProtoError); 7] = [
        (b"not json", ProtoError::Malformed),
        (
            br#"{"v":1,"request_id":42,"action":{"type":"exec","cmd":"calc"}}"#,
            ProtoError::Malformed,
        ),
        (
            br#"{"v":1,"request_id":42,"action":{"type":"none"},"extra":true}"#,
            ProtoError::Malformed,
        ),
        (
            br#"{"v":1,"request_id":41,"action":{"type":"none"}}"#,
            ProtoError::RequestIdMismatch,
        ),
        (
            br#"{"v":9,"request_id":42,"action":{"type":"none"}}"#,
            ProtoError::UnsupportedVersion,
        ),
        (b"[[[[[[[[[[1]]]]]]]]]]", ProtoError::TooDeep),
        (&[0xFF, 0xFE, 0x00], ProtoError::Malformed),
    ];
    for (frame, err) in cases {
        assert_eq!(
            run_wat(&emit_module(frame)),
            OutcomeCode::FrameRejected(err)
        );
    }
    // Zero-length emit.
    assert_eq!(
        run_wat(&emit_module_len(b"", 0)),
        OutcomeCode::FrameRejected(ProtoError::EmptyFrame)
    );
}

#[test]
fn h26_non_whitelisted_keys_rejected() {
    for keys in [
        r#"["Ctrl"]"#,
        r#"["Escape"]"#,
        r#"["Left","Win"]"#,
        r#"[{"Enter":null}]"#,
    ] {
        let f = guest_frame(&format!(r#"{{"type":"keys","keys":{keys}}}"#));
        assert_eq!(
            run_wat(&emit_module(&f)),
            OutcomeCode::FrameRejected(ProtoError::Malformed),
            "{keys}"
        );
    }
    // Enter without the slot permission.
    let enter = guest_frame(r#"{"type":"keys","keys":["Enter"]}"#);
    assert_eq!(
        run_wat(&emit_module(&enter)),
        OutcomeCode::FrameRejected(ProtoError::KeyNotPermitted)
    );
    let mut sb = sandbox();
    let m = compile(&sb, &emit_module(&enter)).unwrap();
    let req = HostRequest {
        permissions: SlotPermissions { enter_tab: true },
        ..request()
    };
    assert_eq!(sb.run(&m, &req).outcome.code(), OutcomeCode::Ok);
    let seventeen = guest_frame(&format!(
        r#"{{"type":"keys","keys":[{}]}}"#,
        vec![r#""Left""#; 17].join(",")
    ));
    assert_eq!(
        run_wat(&emit_module(&seventeen)),
        OutcomeCode::FrameRejected(ProtoError::TooManyKeys)
    );
}

#[test]
fn h27_control_bidi_zero_width_rejected() {
    let cases = [
        (r"a\u202eb", ProtoError::ForbiddenChar(ForbiddenClass::Bidi)),
        (r"a\u2066b", ProtoError::ForbiddenChar(ForbiddenClass::Bidi)),
        (
            r"a\u200bb",
            ProtoError::ForbiddenChar(ForbiddenClass::Invisible),
        ),
        (
            r"a\ufeffb",
            ProtoError::ForbiddenChar(ForbiddenClass::Invisible),
        ),
        (
            r"a\u0000b",
            ProtoError::ForbiddenChar(ForbiddenClass::Control),
        ),
        (
            r"a\r\nb",
            ProtoError::ForbiddenChar(ForbiddenClass::Control),
        ),
        (
            r"a\u001bb",
            ProtoError::ForbiddenChar(ForbiddenClass::Control),
        ),
        (r"a\nb", ProtoError::LineBreakNotPermitted),
    ];
    for (text, err) in cases {
        let f = guest_frame(&format!(r#"{{"type":"insert","text":"{text}"}}"#));
        assert_eq!(
            run_wat(&emit_module(&f)),
            OutcomeCode::FrameRejected(err),
            "{text}"
        );
    }
}

// ------------------------------------------------------------------------------------------
// H-28 / SR-51: no state between runs
// ------------------------------------------------------------------------------------------

#[test]
fn h28_no_state_shared_between_runs() {
    // Increments a mutable global and a memory cell; emits only if both are 1 (first run in a
    // fresh instance), otherwise traps. Two runs must both be Ok.
    let frame = guest_frame(r#"{"type":"none"}"#);
    let wat = format!(
        r#"(module (import "ks" "emit" (func $emit (param i32 i32))) (memory (export "memory") 1)
  (global $g (mut i32) (i32.const 0))
  (data (i32.const 1024) {data})
  (func (export "run")
    (global.set $g (i32.add (global.get $g) (i32.const 1)))
    (i32.store (i32.const 0) (i32.add (i32.load (i32.const 0)) (i32.const 1)))
    (if (i32.ne (global.get $g) (i32.const 1)) (then unreachable))
    (if (i32.ne (i32.load (i32.const 0)) (i32.const 1)) (then unreachable))
    (call $emit (i32.const 1024) (i32.const {n}))))"#,
        data = wat_bytes(&frame),
        n = frame.len()
    );
    let mut sb = sandbox();
    let m = compile(&sb, &wat).unwrap();
    for _ in 0..3 {
        assert_eq!(sb.run(&m, &request()).outcome.code(), OutcomeCode::Ok);
    }
}

#[test]
fn oversized_host_request_is_refused_without_running_guest() {
    let mut sb = sandbox();
    let m = compile(
        &sb,
        r#"(module (memory (export "memory") 1) (func (export "run") unreachable))"#,
    )
    .unwrap();
    let req = HostRequest {
        input: Some(ProtoText::new("x".repeat(300 * 1024))),
        ..request()
    };
    assert_eq!(
        sb.run(&m, &req).outcome.code(),
        OutcomeCode::BadRequest(ProtoError::FrameTooLarge)
    );
}

/// Prints content-free timing numbers for the spike report (run with `--nocapture`).
/// Only durations and outcome codes are printed, never guest output.
#[test]
#[allow(clippy::print_stderr)]
fn timing_numbers_for_report() {
    let cases = [
        (
            "h20_loop",
            r#"(module (memory (export "memory") 1) (func (export "run") (loop $l (br $l))))"#.to_owned(),
        ),
        (
            "h23_grow_loop",
            r#"(module (memory (export "memory") 1)
  (func (export "run") (loop $l (drop (memory.grow (i32.const 1))) (br $l))))"#
                .to_owned(),
        ),
        (
            "h22_recursion",
            r#"(module (memory (export "memory") 1)
  (func $f (param i64) (result i64) (i64.add (call $f (i64.add (local.get 0) (i64.const 1))) (i64.const 1)))
  (func (export "run") (drop (call $f (i64.const 0)))))"#
                .to_owned(),
        ),
        ("valid_insert", emit_module(&guest_frame(r#"{"type":"insert","text":"hello"}"#))),
    ];
    for (name, wat) in cases {
        let mut sb = sandbox();
        let bytes = wasm(&wat);
        let t = Instant::now();
        let m = sb.compile(&bytes).unwrap();
        let compile = t.elapsed();
        let r = sb.run(&m, &request());
        eprintln!(
            "KS-TIMING {name}: outcome={:?} compile={compile:?} instantiate={:?} run_total={:?}",
            r.outcome.code(),
            r.instantiate,
            r.elapsed
        );
    }
}
