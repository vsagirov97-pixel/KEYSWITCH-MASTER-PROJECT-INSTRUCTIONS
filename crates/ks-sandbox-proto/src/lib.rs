//! KeySwitch macro sandbox wire protocol (spike S-3a).
//!
//! Two directions, both JSON, both with hard limits enforced *before* and *after* parsing:
//!
//! * host -> guest: [`HostRequest`], frame <= [`MAX_HOST_FRAME_BYTES`] (256 KiB);
//! * guest -> host: [`GuestMessage`], frame <= [`MAX_GUEST_FRAME_BYTES`] (64 KiB), exactly one per run.
//!
//! A guest frame is accepted only through [`parse_guest_frame`], which checks size, nesting depth
//! (<= [`MAX_JSON_DEPTH`]), `deny_unknown_fields`, protocol version, request id and then
//! [`validate_action`]: text <= [`MAX_OUTPUT_CHARS`] chars without control / bidi / zero-width /
//! invisible characters, `keys` <= [`MAX_KEYS`] entries of the closed [`AllowedKey`] enum, and
//! `Enter`/`Tab` (and `\n`/`\t` in text) only when the slot has [`SlotPermissions::enter_tab`]
//! (SR-46, SR-47, SR-49; owner decision on D-07).
//!
//! The engine must call [`validate_action`] again on its side before applying anything (SR-47:
//! validation in `ks-engine` is independent of the sandbox).
//!
//! Privacy: script text never reaches `Debug`/`Display`. [`ProtoText`] implements neither, and
//! [`ProtoError`] carries only codes (no serde messages, which could echo guest-supplied strings).

use core::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Protocol version carried in every message (`"v"`).
pub const PROTOCOL_VERSION: u32 = 1;
/// Maximum guest -> host frame size in bytes (SR-46).
pub const MAX_GUEST_FRAME_BYTES: usize = 64 * 1024;
/// Maximum host -> guest frame size in bytes.
pub const MAX_HOST_FRAME_BYTES: usize = 256 * 1024;
/// Maximum length of `insert`/`replace` text in Unicode scalar values (SR-47).
pub const MAX_OUTPUT_CHARS: usize = 5_000;
/// Maximum number of keys in one `keys` action (SR-47).
pub const MAX_KEYS: usize = 16;
/// Maximum JSON nesting depth of any frame (SR-46). The top-level object is depth 1.
pub const MAX_JSON_DEPTH: usize = 8;

/// Error codes. Deliberately content-free: nothing from the frame is ever copied in here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtoError {
    /// Frame is empty.
    EmptyFrame,
    /// Frame exceeds the direction's size limit.
    FrameTooLarge,
    /// Length prefix announces more than the direction's limit.
    LengthPrefixTooLarge,
    /// Nesting depth exceeds [`MAX_JSON_DEPTH`].
    TooDeep,
    /// Not valid JSON for the schema (syntax, unknown field, duplicate field, wrong type, ...).
    Malformed,
    /// Unsupported `"v"`.
    UnsupportedVersion,
    /// `request_id` differs from the one the host sent (stale or forged answer).
    RequestIdMismatch,
    /// Text longer than [`MAX_OUTPUT_CHARS`].
    TextTooLong,
    /// Text contains a forbidden character of the given class.
    ForbiddenChar(ForbiddenClass),
    /// `\n` or `\t` in text without the slot's Enter/Tab permission.
    LineBreakNotPermitted,
    /// `keys` action with zero keys.
    NoKeys,
    /// More than [`MAX_KEYS`] keys.
    TooManyKeys,
    /// `Enter`/`Tab` without the slot's Enter/Tab permission.
    KeyNotPermitted,
}

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::EmptyFrame => "empty frame",
            Self::FrameTooLarge => "frame too large",
            Self::LengthPrefixTooLarge => "length prefix too large",
            Self::TooDeep => "nesting too deep",
            Self::Malformed => "malformed message",
            Self::UnsupportedVersion => "unsupported protocol version",
            Self::RequestIdMismatch => "request id mismatch",
            Self::TextTooLong => "text too long",
            Self::ForbiddenChar(ForbiddenClass::Control) => "forbidden control character",
            Self::ForbiddenChar(ForbiddenClass::Bidi) => "forbidden bidi control character",
            Self::ForbiddenChar(ForbiddenClass::Invisible) => "forbidden invisible character",
            Self::LineBreakNotPermitted => "line break or tab not permitted for this slot",
            Self::NoKeys => "empty key list",
            Self::TooManyKeys => "too many keys",
            Self::KeyNotPermitted => "key not permitted for this slot",
        };
        f.write_str(s)
    }
}

impl std::error::Error for ProtoError {}

/// Class of a character that is never allowed in macro output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForbiddenClass {
    /// C0 (except `\n`/`\t`, which are permission-gated), DEL, C1, U+2028/U+2029, noncharacters.
    Control,
    /// Bidi embeddings/overrides/isolates and directional marks (Trojan-Source class).
    Bidi,
    /// Zero-width and other invisible format characters, including Unicode tag characters.
    Invisible,
}

/// Classifies a character for macro output. `None` means allowed; `\n` and `\t` return `None`
/// here and are gated separately by [`SlotPermissions::enter_tab`].
pub fn forbidden_class(c: char) -> Option<ForbiddenClass> {
    let u = u32::from(c);
    match u {
        0x0A | 0x09 => None,
        0x00..=0x1F | 0x7F..=0x9F | 0x2028 | 0x2029 | 0xFFFE | 0xFFFF => {
            Some(ForbiddenClass::Control)
        }
        0x061C | 0x200E | 0x200F | 0x202A..=0x202E | 0x2066..=0x2069 => Some(ForbiddenClass::Bidi),
        0x00AD | 0x180E | 0x200B..=0x200D | 0x2060..=0x2064 | 0xFEFF | 0xFFF9..=0xFFFB => {
            Some(ForbiddenClass::Invisible)
        }
        0xE0000..=0xE007F => Some(ForbiddenClass::Invisible),
        _ => None,
    }
}

/// Correlates one request with its single result message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(pub u64);

/// Per-slot permissions relevant to output validation. Default: everything off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotPermissions {
    /// `Enter`/`Tab` keys and `\n`/`\t` in text (SR-49; off by default).
    #[serde(default)]
    pub enter_tab: bool,
}

/// Opaque text carried by the protocol (script output or slot context).
///
/// No `Debug`, no `Display`: it must not end up in logs or panic messages (CLAUDE.md rule 1).
#[derive(Clone, PartialEq, Eq)]
pub struct ProtoText(String);

impl ProtoText {
    /// Wraps a string.
    pub fn new(s: String) -> Self {
        Self(s)
    }
    /// Borrows the text (for policy checks and applying; never for logging).
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Length in Unicode scalar values.
    pub fn char_len(&self) -> usize {
        self.0.chars().count()
    }
    /// Consumes the wrapper.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl Serialize for ProtoText {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ProtoText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self)
    }
}

/// Closed set of keys a macro may press. No modifiers, no Escape (Escape stops action replay,
/// SR-49), no function keys, nothing that could form a shortcut such as Ctrl+V or Win+R.
///
/// Wire form is only the bare JSON string (`"Left"`); serde's alternative enum encodings such as
/// `{"Left":null}` are rejected by the hand-written `Deserialize` below.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub enum AllowedKey {
    /// Permission-gated (`SlotPermissions::enter_tab`).
    Enter,
    /// Permission-gated (`SlotPermissions::enter_tab`).
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
}

impl AllowedKey {
    /// Every variant, for exhaustive tests and UI lists.
    pub const ALL: [AllowedKey; 12] = [
        Self::Enter,
        Self::Tab,
        Self::Backspace,
        Self::Delete,
        Self::Left,
        Self::Right,
        Self::Up,
        Self::Down,
        Self::Home,
        Self::End,
        Self::PageUp,
        Self::PageDown,
    ];

    /// `true` for keys that need [`SlotPermissions::enter_tab`].
    pub fn needs_enter_tab(self) -> bool {
        matches!(self, Self::Enter | Self::Tab)
    }

    /// Wire name (identical to the variant name).
    pub fn name(self) -> &'static str {
        match self {
            Self::Enter => "Enter",
            Self::Tab => "Tab",
            Self::Backspace => "Backspace",
            Self::Delete => "Delete",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Up => "Up",
            Self::Down => "Down",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "PageUp",
            Self::PageDown => "PageDown",
        }
    }
}

impl<'de> Deserialize<'de> for AllowedKey {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = AllowedKey;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an allowed key name")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<AllowedKey, E> {
                AllowedKey::ALL
                    .into_iter()
                    .find(|k| k.name() == v)
                    // Generic message: never echo the guest-supplied string.
                    .ok_or_else(|| E::custom("key not allowed"))
            }
        }
        d.deserialize_str(V)
    }
}

/// What a script asks the engine to do. No `Debug` (contains script output).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Insert text at the caret.
    Insert {
        /// Text to insert.
        text: ProtoText,
    },
    /// Replace the current selection/token.
    Replace {
        /// Replacement text.
        text: ProtoText,
    },
    /// Press keys from the closed whitelist.
    Keys {
        /// Keys in order.
        keys: Vec<AllowedKey>,
    },
    /// Do nothing. A struct variant (not a unit one) so that `deny_unknown_fields` also
    /// rejects `{"type":"none","x":..}`.
    None {},
}

impl Action {
    /// Content-free kind, safe for logs and journals (SR-58).
    pub fn kind(&self) -> ActionKind {
        match self {
            Self::Insert { .. } => ActionKind::Insert,
            Self::Replace { .. } => ActionKind::Replace,
            Self::Keys { .. } => ActionKind::Keys,
            Self::None {} => ActionKind::None,
        }
    }
}

/// Content-free action kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Insert,
    Replace,
    Keys,
    None,
}

/// Single guest -> host result message.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestMessage {
    /// Protocol version, must equal [`PROTOCOL_VERSION`].
    pub v: u32,
    /// Must equal the id of the request being answered.
    pub request_id: RequestId,
    /// Requested action.
    pub action: Action,
}

impl GuestMessage {
    /// Serializes a message (used by guests and tests). Fails if it would exceed the frame limit.
    pub fn to_frame(&self) -> Result<Vec<u8>, ProtoError> {
        let bytes = serde_json::to_vec(self).map_err(|_| ProtoError::Malformed)?;
        if bytes.len() > MAX_GUEST_FRAME_BYTES {
            return Err(ProtoError::FrameTooLarge);
        }
        Ok(bytes)
    }
}

/// Single host -> guest request.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostRequest {
    /// Protocol version, must equal [`PROTOCOL_VERSION`].
    pub v: u32,
    /// Id the guest must echo.
    pub request_id: RequestId,
    /// Slot permissions (informational for the guest; the host enforces them).
    pub permissions: SlotPermissions,
    /// Slot context text (e.g. selection), if the slot is allowed to see it.
    #[serde(default)]
    pub input: Option<ProtoText>,
}

impl HostRequest {
    /// Serializes the request, enforcing [`MAX_HOST_FRAME_BYTES`].
    pub fn to_frame(&self) -> Result<Vec<u8>, ProtoError> {
        let bytes = serde_json::to_vec(self).map_err(|_| ProtoError::Malformed)?;
        if bytes.len() > MAX_HOST_FRAME_BYTES {
            return Err(ProtoError::FrameTooLarge);
        }
        Ok(bytes)
    }

    /// Parses a host frame (guest side and tests), with the same pre-checks as guest frames.
    pub fn from_frame(frame: &[u8]) -> Result<Self, ProtoError> {
        precheck(frame, MAX_HOST_FRAME_BYTES)?;
        let req: Self = serde_json::from_slice(frame).map_err(|_| ProtoError::Malformed)?;
        if req.v != PROTOCOL_VERSION {
            return Err(ProtoError::UnsupportedVersion);
        }
        Ok(req)
    }
}

/// An action that passed [`validate_action`] for a given request and slot.
pub struct ValidatedAction {
    action: Action,
}

impl ValidatedAction {
    /// The validated action.
    pub fn action(&self) -> &Action {
        &self.action
    }
    /// Consumes the wrapper.
    pub fn into_action(self) -> Action {
        self.action
    }
}

/// Parses and validates one guest -> host frame.
pub fn parse_guest_frame(
    frame: &[u8],
    expected: RequestId,
    permissions: SlotPermissions,
) -> Result<ValidatedAction, ProtoError> {
    precheck(frame, MAX_GUEST_FRAME_BYTES)?;
    let msg: GuestMessage = serde_json::from_slice(frame).map_err(|_| ProtoError::Malformed)?;
    if msg.v != PROTOCOL_VERSION {
        return Err(ProtoError::UnsupportedVersion);
    }
    if msg.request_id != expected {
        return Err(ProtoError::RequestIdMismatch);
    }
    validate_action(&msg.action, permissions)?;
    Ok(ValidatedAction { action: msg.action })
}

/// Validates an action against the output limits and slot permissions (SR-47, SR-49).
pub fn validate_action(action: &Action, permissions: SlotPermissions) -> Result<(), ProtoError> {
    match action {
        Action::Insert { text } | Action::Replace { text } => validate_text(text.as_str(), permissions),
        Action::Keys { keys } => {
            if keys.is_empty() {
                return Err(ProtoError::NoKeys);
            }
            if keys.len() > MAX_KEYS {
                return Err(ProtoError::TooManyKeys);
            }
            if !permissions.enter_tab && keys.iter().any(|k| k.needs_enter_tab()) {
                return Err(ProtoError::KeyNotPermitted);
            }
            Ok(())
        }
        Action::None {} => Ok(()),
    }
}

/// Validates output text: length, forbidden characters, permission-gated `\n`/`\t`.
pub fn validate_text(text: &str, permissions: SlotPermissions) -> Result<(), ProtoError> {
    let mut count = 0usize;
    for c in text.chars() {
        count += 1;
        if count > MAX_OUTPUT_CHARS {
            return Err(ProtoError::TextTooLong);
        }
        if let Some(class) = forbidden_class(c) {
            return Err(ProtoError::ForbiddenChar(class));
        }
        if (c == '\n' || c == '\t') && !permissions.enter_tab {
            return Err(ProtoError::LineBreakNotPermitted);
        }
    }
    Ok(())
}

/// Size and depth checks done before handing bytes to serde.
fn precheck(frame: &[u8], max: usize) -> Result<(), ProtoError> {
    if frame.is_empty() {
        return Err(ProtoError::EmptyFrame);
    }
    if frame.len() > max {
        return Err(ProtoError::FrameTooLarge);
    }
    check_depth(frame, MAX_JSON_DEPTH)
}

/// Rejects JSON whose `{`/`[` nesting exceeds `max_depth`, ignoring brackets inside strings.
/// Linear, allocation-free; runs before serde so deep input never reaches a recursive parser.
pub fn check_depth(bytes: &[u8], max_depth: usize) -> Result<(), ProtoError> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for &b in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > max_depth {
                    return Err(ProtoError::TooDeep);
                }
            }
            // Unbalanced closers are left to serde (-> Malformed).
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

/// Length-prefixed framing for the pipe transport (S-3b): 4-byte little-endian length + payload.
pub mod framing {
    use super::ProtoError;

    /// Size of the length prefix.
    pub const HEADER_LEN: usize = 4;

    /// Prepends the length prefix, refusing payloads over `max`.
    pub fn encode(payload: &[u8], max: usize) -> Result<Vec<u8>, ProtoError> {
        if payload.is_empty() {
            return Err(ProtoError::EmptyFrame);
        }
        if payload.len() > max {
            return Err(ProtoError::FrameTooLarge);
        }
        let len = u32::try_from(payload.len()).map_err(|_| ProtoError::FrameTooLarge)?;
        let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(payload);
        Ok(out)
    }

    /// Decodes the announced length; rejects zero and anything over `max` *before* the reader
    /// allocates or reads the body.
    pub fn decode_len(header: [u8; HEADER_LEN], max: usize) -> Result<usize, ProtoError> {
        let len = usize::try_from(u32::from_le_bytes(header))
            .map_err(|_| ProtoError::LengthPrefixTooLarge)?;
        if len == 0 {
            return Err(ProtoError::EmptyFrame);
        }
        if len > max {
            return Err(ProtoError::LengthPrefixTooLarge);
        }
        Ok(len)
    }

    /// Splits one complete frame from `buf`; the body must be exactly the announced length.
    pub fn decode(buf: &[u8], max: usize) -> Result<&[u8], ProtoError> {
        let header: [u8; HEADER_LEN] = buf
            .get(..HEADER_LEN)
            .and_then(|h| h.try_into().ok())
            .ok_or(ProtoError::Malformed)?;
        let len = decode_len(header, max)?;
        let body = &buf[HEADER_LEN..];
        if body.len() != len {
            return Err(ProtoError::Malformed);
        }
        Ok(body)
    }
}

#[cfg(test)]
mod tests;
