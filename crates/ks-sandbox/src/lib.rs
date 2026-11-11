//! KeySwitch macro sandbox: Wasmtime embedding with hard host-side limits (spike S-3a).
//!
//! Layer 1 of two (ADR-0006): this crate runs a Wasm guest with
//! * no WASI at all - the [`Linker`] exposes only the `ks` namespace ([`ALLOWED_IMPORTS`]);
//!   every other import (`wasi_snapshot_preview1`, `wasi:*`, `env`, ...) is refused before
//!   instantiation (SR-43);
//! * epoch interruption with a host watchdog thread: deadline 2 s by default (SR-52);
//! * a [`ResourceLimiter`]: linear memory <= 64 MiB, 1 instance, 1 memory, 1 table, table
//!   elements bounded (SR-52);
//! * a bounded Wasm stack ([`SandboxConfig::max_wasm_stack_bytes`]);
//! * Wasm proposals not needed by QuickJS/CPython builds disabled in [`Config`] (SR-41);
//! * host functions written without `unsafe`, using only Wasmtime's checked memory API with
//!   `checked_add` and frame-size checks before any copy (SR-44);
//! * exactly one result frame per run, parsed and validated by [`ks_sandbox_proto`] (SR-46/47).
//!
//! Layer 2 (separate low-privilege process: LPAC + Job Object on Windows, App Sandbox helper on
//! macOS) is S-3b/S-3c and not part of this crate yet.
//!
//! Privacy: nothing here logs. [`Outcome`] has no `Debug`; use [`Outcome::code`] for journals.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use ks_sandbox_proto::{
    HostRequest, MAX_GUEST_FRAME_BYTES, MAX_HOST_FRAME_BYTES, ProtoError, ValidatedAction,
    parse_guest_frame,
};
use wasmtime::{
    Caller, Config, Engine, Extern, ExternType, Linker, Module, OptLevel, ResourceLimiter, Store,
    Trap, WasmFeatures,
};

/// The only imports a guest module may declare: `(module, name)`.
///
/// * `ks.emit(ptr: i32, len: i32)` - hand the single result frame (guest -> host, <= 64 KiB).
/// * `ks.request_len() -> i32` - size of the host -> guest request frame.
/// * `ks.request_read(ptr: i32, cap: i32) -> i32` - copy the request frame into guest memory.
pub const ALLOWED_IMPORTS: [(&str, &str); 3] = [
    ("ks", "emit"),
    ("ks", "request_len"),
    ("ks", "request_read"),
];

/// Exported function the host calls (`() -> ()`).
pub const ENTRY_EXPORT: &str = "run";
/// Exported linear memory used by the `ks.*` host functions.
pub const MEMORY_EXPORT: &str = "memory";

const WASM_PAGE: usize = 64 * 1024;

/// Host-side limits for one run.
#[derive(Clone, Debug)]
pub struct SandboxConfig {
    /// Wall-clock deadline for instantiation (incl. `start`) plus `run` (default 2 s, SR-52).
    pub deadline: Duration,
    /// Maximum linear memory in bytes (default 64 MiB, SR-52).
    pub memory_limit_bytes: usize,
    /// Maximum number of elements in the guest's table.
    pub max_table_elements: usize,
    /// Maximum native stack the guest may use (Wasmtime `max_wasm_stack`).
    pub max_wasm_stack_bytes: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            deadline: Duration::from_secs(2),
            memory_limit_bytes: 64 * 1024 * 1024,
            max_table_elements: 20_000,
            max_wasm_stack_bytes: 512 * 1024,
        }
    }
}

/// Setup or module-admission errors. Content-free.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SandboxError {
    /// Engine could not be created from the hardened [`Config`].
    EngineConfig,
    /// Module failed validation/compilation (includes disabled Wasm proposals).
    InvalidModule,
    /// Module declares an import outside [`ALLOWED_IMPORTS`] (e.g. any WASI or `env` import).
    ImportDenied,
    /// Module lacks `run: () -> ()`.
    MissingEntry,
    /// Module was compiled by a different [`Sandbox`].
    ForeignModule,
}

impl core::fmt::Display for SandboxError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::EngineConfig => "engine configuration rejected",
            Self::InvalidModule => "invalid or disallowed wasm module",
            Self::ImportDenied => "module imports a function outside the ks namespace allowlist",
            Self::MissingEntry => "module does not export run: () -> ()",
            Self::ForeignModule => "module belongs to another engine",
        })
    }
}

impl std::error::Error for SandboxError {}

/// Guest misbehaviour detected inside a host function; the call traps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Violation {
    /// `ptr..ptr+len` outside guest memory, or arithmetic overflow.
    OutOfBounds,
    /// `ks.emit` with `len` > 64 KiB (checked before any copy).
    FrameTooLarge,
    /// `ks.emit` called more than once.
    DuplicateEmit,
    /// Guest does not export `memory`.
    NoMemory,
    /// `ks.request_read` with a buffer smaller than the request.
    BufferTooSmall,
}

impl core::fmt::Display for Violation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::OutOfBounds => "guest pointer out of bounds",
            Self::FrameTooLarge => "guest frame too large",
            Self::DuplicateEmit => "guest emitted more than one frame",
            Self::NoMemory => "guest exports no memory",
            Self::BufferTooSmall => "guest buffer too small for request",
        })
    }
}

impl std::error::Error for Violation {}

#[derive(Debug)]
struct ResourceLimitHit;

impl core::fmt::Display for ResourceLimitHit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("sandbox resource limit exceeded")
    }
}

impl std::error::Error for ResourceLimitHit {}

/// Result of one run. No `Debug`/`Display` (the `Ok` variant carries script output).
pub enum Outcome {
    /// Script finished and returned a valid action.
    Ok(ValidatedAction),
    /// Deadline reached (epoch interrupt).
    Timeout,
    /// Memory or table growth beyond the limit (trapped), or instance/memory/table count.
    ResourceLimit,
    /// Guest stack exhausted.
    StackOverflow,
    /// Any other Wasm trap (unreachable, OOB access, division by zero, ...).
    Trap,
    /// A host function caught the guest misbehaving.
    Violation(Violation),
    /// The single result frame failed protocol validation.
    FrameRejected(ProtoError),
    /// Script finished without calling `ks.emit`.
    NoResult,
    /// Instantiation failed for another reason (e.g. import type mismatch).
    InstantiationFailed,
    /// The host request could not be encoded within limits.
    BadRequest(ProtoError),
    /// Host could not set up the run (e.g. watchdog thread); guest code was not started.
    HostFailure,
}

/// Content-free outcome code for logs/journals (SR-58).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutcomeCode {
    Ok,
    Timeout,
    ResourceLimit,
    StackOverflow,
    Trap,
    Violation(Violation),
    FrameRejected(ProtoError),
    NoResult,
    InstantiationFailed,
    BadRequest(ProtoError),
    HostFailure,
}

impl OutcomeCode {
    /// Process exit code used by the `ks-sandbox` binary (0 = Ok, 10.. = failures).
    pub fn exit_code(self) -> i32 {
        match self {
            Self::Ok => 0,
            Self::Timeout => 10,
            Self::ResourceLimit => 11,
            Self::StackOverflow => 12,
            Self::Trap => 13,
            Self::Violation(_) => 14,
            Self::FrameRejected(_) => 15,
            Self::NoResult => 16,
            Self::InstantiationFailed => 17,
            Self::BadRequest(_) => 18,
            Self::HostFailure => 19,
        }
    }
}

impl Outcome {
    /// Content-free code.
    pub fn code(&self) -> OutcomeCode {
        match self {
            Self::Ok(_) => OutcomeCode::Ok,
            Self::Timeout => OutcomeCode::Timeout,
            Self::ResourceLimit => OutcomeCode::ResourceLimit,
            Self::StackOverflow => OutcomeCode::StackOverflow,
            Self::Trap => OutcomeCode::Trap,
            Self::Violation(v) => OutcomeCode::Violation(*v),
            Self::FrameRejected(e) => OutcomeCode::FrameRejected(*e),
            Self::NoResult => OutcomeCode::NoResult,
            Self::InstantiationFailed => OutcomeCode::InstantiationFailed,
            Self::BadRequest(e) => OutcomeCode::BadRequest(*e),
            Self::HostFailure => OutcomeCode::HostFailure,
        }
    }
}

/// Outcome plus timing of one run.
pub struct RunReport {
    /// What happened.
    pub outcome: Outcome,
    /// Time spent instantiating the module (incl. `start`).
    pub instantiate: Duration,
    /// Total wall time of the run (instantiate + `run` + validation).
    pub elapsed: Duration,
}

/// A module that passed admission (allowlisted imports, `run` export) for one [`Sandbox`].
pub struct GuestModule {
    module: Module,
}

struct Limiter {
    memory_limit: usize,
    table_limit: usize,
    hit: bool,
}

impl ResourceLimiter for Limiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.memory_limit {
            self.hit = true;
            // Trap instead of returning -1: deterministic termination of memory bombs (H-23).
            return Err(wasmtime::Error::new(ResourceLimitHit));
        }
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.table_limit {
            self.hit = true;
            return Err(wasmtime::Error::new(ResourceLimitHit));
        }
        Ok(true)
    }

    fn instances(&self) -> usize {
        1
    }
    fn tables(&self) -> usize {
        1
    }
    fn memories(&self) -> usize {
        1
    }
}

struct HostState {
    limiter: Limiter,
    request: Vec<u8>,
    emitted: Option<Vec<u8>>,
}

/// Wasmtime engine with the hardened configuration. One run at a time (`&mut self`), because the
/// epoch counter is engine-wide (SR-53: one operation at a time).
pub struct Sandbox {
    engine: Engine,
    linker: Linker<HostState>,
    config: SandboxConfig,
}

/// Builds the hardened Wasmtime [`Config`] (exposed for the SR-41 unit test).
pub fn hardened_config(cfg: &SandboxConfig) -> Config {
    let mut c = Config::new();
    c.epoch_interruption(true);
    c.max_wasm_stack(cfg.max_wasm_stack_bytes);
    c.cranelift_opt_level(OptLevel::Speed);
    // SR-41: proposals the guest interpreters do not need are rejected by the validator.
    c.wasm_features(
        WasmFeatures::THREADS
            | WasmFeatures::SHARED_EVERYTHING_THREADS
            | WasmFeatures::TAIL_CALL
            | WasmFeatures::MULTI_MEMORY
            | WasmFeatures::EXCEPTIONS
            | WasmFeatures::LEGACY_EXCEPTIONS
            | WasmFeatures::MEMORY64
            | WasmFeatures::COMPONENT_MODEL
            | WasmFeatures::FUNCTION_REFERENCES
            | WasmFeatures::GC
            | WasmFeatures::MEMORY_CONTROL
            | WasmFeatures::CUSTOM_PAGE_SIZES
            | WasmFeatures::STACK_SWITCHING
            | WasmFeatures::WIDE_ARITHMETIC
            | WasmFeatures::RELAXED_SIMD
            | WasmFeatures::CUSTOM_DESCRIPTORS,
        false,
    );
    c
}

impl Sandbox {
    /// Creates the engine and the `ks`-only linker.
    pub fn new(config: SandboxConfig) -> Result<Self, SandboxError> {
        let engine =
            Engine::new(&hardened_config(&config)).map_err(|_| SandboxError::EngineConfig)?;
        let mut linker: Linker<HostState> = Linker::new(&engine);
        linker
            .func_wrap("ks", "emit", host_emit)
            .and_then(|l| l.func_wrap("ks", "request_len", host_request_len))
            .and_then(|l| l.func_wrap("ks", "request_read", host_request_read))
            .map_err(|_| SandboxError::EngineConfig)?;
        Ok(Self {
            engine,
            linker,
            config,
        })
    }

    /// Effective limits.
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    /// Validates, compiles and admits a guest module.
    pub fn compile(&self, wasm: &[u8]) -> Result<GuestModule, SandboxError> {
        let module = Module::new(&self.engine, wasm).map_err(|_| SandboxError::InvalidModule)?;
        for import in module.imports() {
            let allowed = ALLOWED_IMPORTS.contains(&(import.module(), import.name()))
                && matches!(import.ty(), ExternType::Func(_));
            if !allowed {
                return Err(SandboxError::ImportDenied);
            }
        }
        match module.get_export(ENTRY_EXPORT) {
            Some(ExternType::Func(f)) if f.params().len() == 0 && f.results().len() == 0 => {}
            _ => return Err(SandboxError::MissingEntry),
        }
        Ok(GuestModule { module })
    }

    /// Runs `module` once in a fresh [`Store`] (no state shared between runs, SR-51).
    pub fn run(&mut self, module: &GuestModule, request: &HostRequest) -> RunReport {
        let started = Instant::now();
        let report = |outcome, instantiate| RunReport {
            outcome,
            instantiate,
            elapsed: started.elapsed(),
        };
        if !Engine::same(module.module.engine(), &self.engine) {
            return report(Outcome::InstantiationFailed, Duration::ZERO);
        }
        let frame = match request.to_frame() {
            Ok(f) => f,
            Err(e) => return report(Outcome::BadRequest(e), Duration::ZERO),
        };
        debug_assert!(frame.len() <= MAX_HOST_FRAME_BYTES);

        let state = HostState {
            limiter: Limiter {
                memory_limit: self.config.memory_limit_bytes,
                table_limit: self.config.max_table_elements,
                hit: false,
            },
            request: frame,
            emitted: None,
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limiter);
        store.set_epoch_deadline(1);
        store.epoch_deadline_trap();

        // Watchdog covers instantiation too: a `start` function is guest code.
        // Fail closed: no watchdog, no guest code.
        let Some(watchdog) = Watchdog::start(self.engine.clone(), self.config.deadline) else {
            return report(Outcome::HostFailure, Duration::ZERO);
        };
        let inst_started = Instant::now();
        let instance = self.linker.instantiate(&mut store, &module.module);
        let instantiate = inst_started.elapsed();
        let result = instance.and_then(|i| {
            let run = i.get_typed_func::<(), ()>(&mut store, ENTRY_EXPORT)?;
            run.call(&mut store, ())
        });
        drop(watchdog);

        let outcome = match result {
            Ok(()) => match store.data_mut().emitted.take() {
                Some(bytes) => {
                    match parse_guest_frame(&bytes, request.request_id, request.permissions) {
                        Ok(action) => Outcome::Ok(action),
                        Err(e) => Outcome::FrameRejected(e),
                    }
                }
                None => Outcome::NoResult,
            },
            // A frame emitted before a trap/timeout is discarded (SR-46).
            Err(e) => classify(&e, store.data().limiter.hit),
        };
        report(outcome, instantiate)
    }
}

fn classify(e: &wasmtime::Error, limit_hit: bool) -> Outcome {
    if limit_hit || e.is::<ResourceLimitHit>() {
        return Outcome::ResourceLimit;
    }
    if let Some(v) = e.downcast_ref::<Violation>() {
        return Outcome::Violation(*v);
    }
    match e.downcast_ref::<Trap>() {
        Some(Trap::Interrupt) => Outcome::Timeout,
        Some(Trap::StackOverflow) => Outcome::StackOverflow,
        Some(_) => Outcome::Trap,
        None => Outcome::InstantiationFailed,
    }
}

/// Increments the engine epoch once when the deadline passes; dropping it before then cancels.
struct Watchdog {
    cancel: Option<mpsc::Sender<()>>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Watchdog {
    /// `None` if the timer thread cannot be spawned; the caller must then not run guest code.
    fn start(engine: Engine, deadline: Duration) -> Option<Self> {
        let (tx, rx) = mpsc::channel::<()>();
        let handle = thread::Builder::new()
            .name("ks-sandbox-watchdog".into())
            .spawn(move || {
                if let Err(mpsc::RecvTimeoutError::Timeout) = rx.recv_timeout(deadline) {
                    engine.increment_epoch();
                }
            })
            .ok()?;
        Some(Self {
            cancel: Some(tx),
            handle: Some(handle),
        })
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        drop(self.cancel.take());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn guest_range(
    caller: &mut Caller<'_, HostState>,
    ptr: i32,
    len: usize,
) -> wasmtime::Result<(wasmtime::Memory, usize)> {
    let memory = caller
        .get_export(MEMORY_EXPORT)
        .and_then(Extern::into_memory)
        .ok_or(Violation::NoMemory)?;
    let start = usize::try_from(ptr.cast_unsigned()).map_err(|_| Violation::OutOfBounds)?;
    let end = start.checked_add(len).ok_or(Violation::OutOfBounds)?;
    if end > memory.data_size(&*caller) {
        return Err(Violation::OutOfBounds.into());
    }
    Ok((memory, start))
}

fn host_emit(mut caller: Caller<'_, HostState>, ptr: i32, len: i32) -> wasmtime::Result<()> {
    if caller.data().emitted.is_some() {
        return Err(Violation::DuplicateEmit.into());
    }
    let len = usize::try_from(len.cast_unsigned()).map_err(|_| Violation::FrameTooLarge)?;
    // Size check before touching memory or allocating (SR-46).
    if len > MAX_GUEST_FRAME_BYTES {
        return Err(Violation::FrameTooLarge.into());
    }
    let (memory, start) = guest_range(&mut caller, ptr, len)?;
    let mut buf = vec![0u8; len];
    memory
        .read(&caller, start, &mut buf)
        .map_err(|_| Violation::OutOfBounds)?;
    caller.data_mut().emitted = Some(buf);
    Ok(())
}

fn host_request_len(caller: Caller<'_, HostState>) -> i32 {
    // <= 256 KiB, always fits.
    i32::try_from(caller.data().request.len()).unwrap_or(i32::MAX)
}

fn host_request_read(
    mut caller: Caller<'_, HostState>,
    ptr: i32,
    cap: i32,
) -> wasmtime::Result<i32> {
    let len = caller.data().request.len();
    let cap = usize::try_from(cap.cast_unsigned()).map_err(|_| Violation::OutOfBounds)?;
    if cap < len {
        return Err(Violation::BufferTooSmall.into());
    }
    let (memory, start) = guest_range(&mut caller, ptr, len)?;
    let (data, state) = memory.data_and_store_mut(&mut caller);
    let dst = data
        .get_mut(start..start + len)
        .ok_or(Violation::OutOfBounds)?;
    dst.copy_from_slice(&state.request);
    Ok(i32::try_from(len).unwrap_or(i32::MAX))
}

/// Linear-memory page count corresponding to the configured byte limit (for tests/docs).
pub fn memory_limit_pages(cfg: &SandboxConfig) -> usize {
    cfg.memory_limit_bytes / WASM_PAGE
}
