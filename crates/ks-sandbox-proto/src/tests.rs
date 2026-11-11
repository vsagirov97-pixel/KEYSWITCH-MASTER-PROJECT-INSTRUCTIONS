//! Unit and deterministic random tests (SR-46, SR-47, SR-49; L-10; H-25, H-26, H-27).
//! Test data is synthetic only.

use super::*;

const ID: RequestId = RequestId(7);
const NO_PERMS: SlotPermissions = SlotPermissions { enter_tab: false };
const ET: SlotPermissions = SlotPermissions { enter_tab: true };

fn frame(action_json: &str) -> Vec<u8> {
    format!(r#"{{"v":1,"request_id":7,"action":{action_json}}}"#).into_bytes()
}

fn parse(bytes: &[u8], perms: SlotPermissions) -> Result<ActionKind, ProtoError> {
    parse_guest_frame(bytes, ID, perms).map(|a| a.action().kind())
}

fn insert_frame(text: &str) -> Vec<u8> {
    GuestMessage {
        v: PROTOCOL_VERSION,
        request_id: ID,
        action: Action::Insert { text: ProtoText::new(text.to_owned()) },
    }
    .to_frame()
    .unwrap()
}

#[test]
fn valid_actions_parse() {
    assert_eq!(parse(&frame(r#"{"type":"insert","text":"hello"}"#), NO_PERMS), Ok(ActionKind::Insert));
    assert_eq!(parse(&frame(r#"{"type":"replace","text":""}"#), NO_PERMS), Ok(ActionKind::Replace));
    assert_eq!(parse(&frame(r#"{"type":"keys","keys":["Left","Home"]}"#), NO_PERMS), Ok(ActionKind::Keys));
    assert_eq!(parse(&frame(r#"{"type":"none"}"#), NO_PERMS), Ok(ActionKind::None));
    let ok = parse_guest_frame(&insert_frame("Привет, мир"), ID, NO_PERMS).unwrap();
    match ok.into_action() {
        Action::Insert { text } => assert_eq!(text.as_str(), "Привет, мир"),
        _ => panic!("wrong kind"),
    }
}

#[test]
fn roundtrip_serialization() {
    for action in [
        Action::Insert { text: ProtoText::new("a\u{00e9}".into()) },
        Action::Replace { text: ProtoText::new("b".into()) },
        Action::Keys { keys: AllowedKey::ALL[2..].to_vec() },
        Action::None {},
    ] {
        let kind = action.kind();
        let bytes = GuestMessage { v: 1, request_id: ID, action }.to_frame().unwrap();
        assert_eq!(parse(&bytes, NO_PERMS), Ok(kind));
    }
}

#[test]
fn unknown_and_duplicate_fields_rejected() {
    let cases: [&[u8]; 7] = [
        br#"{"v":1,"request_id":7,"action":{"type":"none"},"extra":1}"#,
        br#"{"v":1,"request_id":7,"action":{"type":"insert","text":"a","extra":1}}"#,
        br#"{"v":1,"request_id":7,"action":{"type":"keys","keys":["Left"],"mods":["Ctrl"]}}"#,
        br#"{"v":1,"request_id":7,"action":{"type":"none","x":0}}"#,
        br#"{"v":1,"v":1,"request_id":7,"action":{"type":"none"}}"#,
        br#"{"v":1,"request_id":7,"action":{"type":"insert","text":"a","text":"b"}}"#,
        br#"{"v":1,"request_id":7,"action":{"type":"exec","cmd":"x"}}"#,
    ];
    for c in cases {
        assert_eq!(parse(c, ET), Err(ProtoError::Malformed));
    }
}

#[test]
fn missing_fields_and_wrong_types_rejected() {
    let cases: [&[u8]; 8] = [
        br#"{"v":1,"request_id":7}"#,
        br#"{"request_id":7,"action":{"type":"none"}}"#,
        br#"{"v":1,"action":{"type":"none"}}"#,
        br#"{"v":"1","request_id":7,"action":{"type":"none"}}"#,
        br#"{"v":1,"request_id":-7,"action":{"type":"none"}}"#,
        br#"{"v":1,"request_id":7,"action":{"type":"insert"}}"#,
        br#"{"v":1,"request_id":7,"action":{"type":"insert","text":5}}"#,
        br#"{"v":1,"request_id":7,"action":{"type":"none"}} trailing"#,
    ];
    for c in cases {
        assert_eq!(parse(c, ET), Err(ProtoError::Malformed));
    }
    assert_eq!(parse(br#"[]"#, ET), Err(ProtoError::Malformed));
    assert_eq!(parse(b"", ET), Err(ProtoError::EmptyFrame));
    // Lone surrogate escape is not valid UTF-16/UTF-8 text.
    assert_eq!(parse(&frame(r#"{"type":"insert","text":"\ud800"}"#), ET), Err(ProtoError::Malformed));
    // Invalid UTF-8 bytes.
    let mut bad = frame(r#"{"type":"insert","text":"__"}"#);
    let pos = bad.iter().position(|&b| b == b'_').unwrap();
    bad[pos] = 0xFF;
    assert_eq!(parse(&bad, ET), Err(ProtoError::Malformed));
}

#[test]
fn version_and_request_id_checked() {
    assert_eq!(
        parse(br#"{"v":2,"request_id":7,"action":{"type":"none"}}"#, NO_PERMS),
        Err(ProtoError::UnsupportedVersion)
    );
    assert_eq!(
        parse(br#"{"v":1,"request_id":8,"action":{"type":"none"}}"#, NO_PERMS),
        Err(ProtoError::RequestIdMismatch)
    );
}

#[test]
fn text_length_boundary() {
    let at = "я".repeat(MAX_OUTPUT_CHARS);
    assert_eq!(parse(&insert_frame(&at), NO_PERMS), Ok(ActionKind::Insert));
    let over = "я".repeat(MAX_OUTPUT_CHARS + 1);
    assert_eq!(parse(&insert_frame(&over), NO_PERMS), Err(ProtoError::TextTooLong));
    // Astral chars count once each.
    let astral = "\u{1F600}".repeat(MAX_OUTPUT_CHARS);
    assert_eq!(parse(&insert_frame(&astral), NO_PERMS), Ok(ActionKind::Insert));
}

#[test]
fn guest_frame_size_boundary() {
    // Build a frame of exactly MAX_GUEST_FRAME_BYTES using JSON whitespace padding.
    let base = frame(r#"{"type":"none"}"#);
    let mut exact = base.clone();
    exact.splice(1..1, std::iter::repeat_n(b' ', MAX_GUEST_FRAME_BYTES - base.len()));
    assert_eq!(exact.len(), MAX_GUEST_FRAME_BYTES);
    assert_eq!(parse(&exact, NO_PERMS), Ok(ActionKind::None));
    exact.insert(1, b' ');
    assert_eq!(parse(&exact, NO_PERMS), Err(ProtoError::FrameTooLarge));
    // A 5 000-char text of 4-byte chars is ~20 KiB (> any legitimate need, < 64 KiB) - fine;
    // a 5 000-char text of \u escapes (6 bytes/char = 30 KiB) also fits.
    let escaped = format!(r#"{{"type":"insert","text":"{}"}}"#, r"\u0444".repeat(MAX_OUTPUT_CHARS));
    assert_eq!(parse(&frame(&escaped), NO_PERMS), Ok(ActionKind::Insert));
}

#[test]
fn host_frame_size_boundary() {
    let mk = |n: usize| HostRequest {
        v: 1,
        request_id: ID,
        permissions: SlotPermissions::default(),
        input: Some(ProtoText::new("x".repeat(n))),
    };
    let overhead = mk(0).to_frame().unwrap().len();
    let ok = mk(MAX_HOST_FRAME_BYTES - overhead).to_frame().unwrap();
    assert_eq!(ok.len(), MAX_HOST_FRAME_BYTES);
    assert!(HostRequest::from_frame(&ok).is_ok());
    assert_eq!(mk(MAX_HOST_FRAME_BYTES - overhead + 1).to_frame().err(), Some(ProtoError::FrameTooLarge));
    let mut big = ok.clone();
    big.insert(1, b' ');
    assert_eq!(HostRequest::from_frame(&big).err(), Some(ProtoError::FrameTooLarge));
    assert_eq!(
        HostRequest::from_frame(br#"{"v":1,"request_id":1,"permissions":{},"env":{}}"#).err(),
        Some(ProtoError::Malformed)
    );
}

#[test]
fn depth_limit() {
    let nested = |d: usize| format!("{}{}", "[".repeat(d), "]".repeat(d)).into_bytes();
    assert_eq!(check_depth(&nested(MAX_JSON_DEPTH), MAX_JSON_DEPTH), Ok(()));
    assert_eq!(check_depth(&nested(MAX_JSON_DEPTH + 1), MAX_JSON_DEPTH), Err(ProtoError::TooDeep));
    // Brackets inside strings (including escaped quotes) do not count.
    let s = format!(r#"{{"type":"insert","text":"\"{}"}}"#, "[{".repeat(50));
    assert_eq!(parse(&frame(&s), NO_PERMS), Ok(ActionKind::Insert));
    // Deep value under an unknown field is caught by the depth check first.
    let deep = format!(
        r#"{{"v":1,"request_id":7,"action":{{"type":"none"}},"x":{}1{}}}"#,
        "[".repeat(10_000),
        "]".repeat(10_000)
    );
    assert_eq!(parse(deep.as_bytes(), NO_PERMS), Err(ProtoError::TooDeep));
    // Our schema itself is depth 3 (message > action > keys).
    assert_eq!(parse(&frame(r#"{"type":"keys","keys":["Up"]}"#), NO_PERMS), Ok(ActionKind::Keys));
}

#[test]
fn forbidden_characters_rejected_h27() {
    let control = [0x00u32, 0x01, 0x08, 0x0B, 0x0C, 0x0D, 0x1B, 0x1F, 0x7F, 0x80, 0x85, 0x9F, 0x2028, 0x2029, 0xFFFE, 0xFFFF];
    let bidi = [0x061Cu32, 0x200E, 0x200F, 0x202A, 0x202B, 0x202C, 0x202D, 0x202E, 0x2066, 0x2067, 0x2068, 0x2069];
    let invisible = [0x00ADu32, 0x180E, 0x200B, 0x200C, 0x200D, 0x2060, 0x2061, 0x2064, 0xFEFF, 0xFFF9, 0xFFFB, 0xE0000, 0xE0041, 0xE007F];
    for (set, class) in [
        (&control[..], ForbiddenClass::Control),
        (&bidi[..], ForbiddenClass::Bidi),
        (&invisible[..], ForbiddenClass::Invisible),
    ] {
        for &u in set {
            let c = char::from_u32(u).unwrap();
            assert_eq!(forbidden_class(c), Some(class), "U+{u:04X}");
            let text = format!("ok{c}ok");
            // Rejected even with Enter/Tab permission.
            assert_eq!(parse(&insert_frame(&text), ET), Err(ProtoError::ForbiddenChar(class)), "U+{u:04X}");
            // Also when sent as a JSON \u escape.
            let esc = if u > 0xFFFF {
                let v = u - 0x10000;
                format!(r"\u{:04x}\u{:04x}", 0xD800 + (v >> 10), 0xDC00 + (v & 0x3FF))
            } else {
                format!(r"\u{u:04x}")
            };
            let raw = frame(&format!(r#"{{"type":"replace","text":"a{esc}"}}"#));
            assert_eq!(parse(&raw, ET), Err(ProtoError::ForbiddenChar(class)), "U+{u:04X} escaped");
        }
    }
    // Neighbours of the ranges are allowed.
    for u in [0x20u32, 0x7E, 0xA0, 0xAC, 0xAE, 0x2027, 0x202F, 0x2065, 0x206A, 0x2010, 0xFEFE, 0xFFF8, 0xFFFC, 0xDFFFF, 0xE0080] {
        let c = char::from_u32(u).unwrap();
        assert_eq!(forbidden_class(c), None, "U+{u:04X}");
    }
}

#[test]
fn line_breaks_need_enter_tab_permission() {
    for t in ["a\nb", "a\tb", "\n"] {
        assert_eq!(parse(&insert_frame(t), NO_PERMS), Err(ProtoError::LineBreakNotPermitted));
        assert_eq!(parse(&insert_frame(t), ET), Ok(ActionKind::Insert));
    }
    // \r is never allowed (no CRLF smuggling), even with permission.
    assert_eq!(
        parse(&insert_frame("a\r\nb"), ET),
        Err(ProtoError::ForbiddenChar(ForbiddenClass::Control))
    );
}

#[test]
fn keys_whitelist_h26() {
    for k in AllowedKey::ALL {
        let name = serde_json::to_string(&k).unwrap();
        let f = frame(&format!(r#"{{"type":"keys","keys":[{name}]}}"#));
        let expected_no_perm = if k.needs_enter_tab() { Err(ProtoError::KeyNotPermitted) } else { Ok(ActionKind::Keys) };
        assert_eq!(parse(&f, NO_PERMS), expected_no_perm, "{name}");
        assert_eq!(parse(&f, ET), Ok(ActionKind::Keys), "{name}");
    }
    assert_eq!(AllowedKey::ALL.iter().filter(|k| k.needs_enter_tab()).count(), 2);
    // Not in the closed enum: modifiers, Escape, F-keys, Windows key, chords, case variants, numbers.
    for bad in [
        "\"Ctrl\"", "\"Control\"", "\"Alt\"", "\"Shift\"", "\"Win\"", "\"Meta\"", "\"Escape\"", "\"Esc\"",
        "\"F4\"", "\"Ctrl+V\"", "\"enter\"", "\"ENTER\"", "\"Enter \"", "\"\"", "13", "null",
        "{\"Enter\":null}", "[\"Enter\"]", "\"PrintScreen\"", "\"Insert\"", "\"Apps\"",
    ] {
        let f = frame(&format!(r#"{{"type":"keys","keys":[{bad}]}}"#));
        assert_eq!(parse(&f, ET), Err(ProtoError::Malformed), "{bad}");
    }
}

#[test]
fn keys_count_limits() {
    let list = |n: usize| frame(&format!(r#"{{"type":"keys","keys":[{}]}}"#, vec!["\"Left\""; n].join(",")));
    assert_eq!(parse(&list(0), NO_PERMS), Err(ProtoError::NoKeys));
    assert_eq!(parse(&list(1), NO_PERMS), Ok(ActionKind::Keys));
    assert_eq!(parse(&list(MAX_KEYS), NO_PERMS), Ok(ActionKind::Keys));
    assert_eq!(parse(&list(MAX_KEYS + 1), NO_PERMS), Err(ProtoError::TooManyKeys));
    assert_eq!(parse(&list(5_000), NO_PERMS), Err(ProtoError::TooManyKeys));
}

#[test]
fn errors_are_content_free() {
    // A guest-chosen marker must not appear in any error's Debug/Display.
    let marker = "MARKER_8f3a";
    let inputs = [
        frame(&format!(r#"{{"type":"{marker}"}}"#)),
        frame(&format!(r#"{{"type":"keys","keys":["{marker}"]}}"#)),
        format!(r#"{{"v":1,"request_id":7,"{marker}":1,"action":{{"type":"none"}}}}"#).into_bytes(),
        insert_frame(&format!("{marker}\u{202e}")),
    ];
    for i in inputs {
        let e = parse(&i, NO_PERMS).unwrap_err();
        assert!(!format!("{e:?} {e}").contains(marker));
    }
}

#[test]
fn framing_limits() {
    use framing::*;
    let f = encode(b"{}", MAX_GUEST_FRAME_BYTES).unwrap();
    assert_eq!(decode(&f, MAX_GUEST_FRAME_BYTES), Ok(&b"{}"[..]));
    assert_eq!(encode(b"", 10).err(), Some(ProtoError::EmptyFrame));
    assert_eq!(encode(&[b' '; 11], 10).err(), Some(ProtoError::FrameTooLarge));
    assert_eq!(decode_len(0u32.to_le_bytes(), 10), Err(ProtoError::EmptyFrame));
    assert_eq!(decode_len(10u32.to_le_bytes(), 10), Ok(10));
    assert_eq!(decode_len(11u32.to_le_bytes(), 10), Err(ProtoError::LengthPrefixTooLarge));
    assert_eq!(decode_len(u32::MAX.to_le_bytes(), MAX_HOST_FRAME_BYTES), Err(ProtoError::LengthPrefixTooLarge));
    assert_eq!(decode(&[1, 0, 0], 10), Err(ProtoError::Malformed));
    assert_eq!(decode(&[2, 0, 0, 0, b'{'], 10), Err(ProtoError::Malformed));
    assert_eq!(decode(&[1, 0, 0, 0, b'{', b'}'], 10), Err(ProtoError::Malformed));
}

// ---------------------------------------------------------------------------------------------
// Deterministic random testing (no external crates): SplitMix64, fixed seed, reproducible.
// Iterations: 100 000 by default; KS_PROTO_FUZZ_ITERS overrides (e.g. 1 000 000 for SR-46).
// ---------------------------------------------------------------------------------------------

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

const TOKENS: &[&str] = &[
    "{", "}", "[", "]", ":", ",", "\"", "\\", "\"v\"", "\"request_id\"", "\"action\"", "\"type\"",
    "\"text\"", "\"keys\"", "\"insert\"", "\"replace\"", "\"none\"", "\"Enter\"", "\"Tab\"",
    "\"Left\"", "\"Ctrl\"", "1", "7", "-1", "1e999", "18446744073709551616", "0.5", "null", "true",
    "false", "\"\\u202e\"", "\"\\u0000\"", "\"\\ud800\"", "\"\\n\"", "\"\u{200b}\"", "\"x\"",
    "\"\\\"\"", " ", "\n",
];

fn random_text(r: &mut Rng) -> String {
    let len = r.below(40);
    (0..len)
        .map(|_| match r.below(6) {
            0 => char::from_u32(r.below(0x80) as u32).unwrap_or('a'),
            1 => *r.pick(&['\n', '\t', '\r', '\u{202e}', '\u{200b}', '\u{feff}', '\u{e0041}', '\u{85}']),
            2 => char::from_u32(0x400 + r.below(0x60) as u32).unwrap_or('я'),
            3 => char::from_u32(r.below(0x11_0000) as u32).unwrap_or('?'),
            _ => 'a',
        })
        .collect()
}

fn random_input(r: &mut Rng, seeds: &[Vec<u8>]) -> Vec<u8> {
    match r.below(5) {
        // Raw random bytes.
        0 => (0..r.below(300)).map(|_| r.next() as u8).collect(),
        // Token soup.
        1 => (0..r.below(60)).flat_map(|_| r.pick(TOKENS).as_bytes().to_vec()).collect(),
        // Mutated valid frame.
        2 | 3 => {
            let mut v = r.pick(seeds).clone();
            for _ in 0..=r.below(4) {
                if v.is_empty() {
                    break;
                }
                let i = r.below(v.len());
                match r.below(5) {
                    0 => v[i] = r.next() as u8,
                    1 => {
                        v.remove(i);
                    }
                    2 => v.splice(i..i, r.pick(TOKENS).bytes()).for_each(drop),
                    3 => v.truncate(i),
                    _ => v[i] ^= 1 << r.below(8),
                }
            }
            v
        }
        // Structurally valid message with random content.
        _ => {
            let action = match r.below(4) {
                0 => Action::Insert { text: ProtoText::new(random_text(r)) },
                1 => Action::Replace { text: ProtoText::new(random_text(r)) },
                2 => Action::Keys { keys: (0..r.below(20)).map(|_| *r.pick(&AllowedKey::ALL)).collect() },
                _ => Action::None {},
            };
            let id = if r.below(10) == 0 { RequestId(r.next()) } else { ID };
            GuestMessage { v: if r.below(20) == 0 { 2 } else { 1 }, request_id: id, action }
                .to_frame()
                .unwrap()
        }
    }
}

#[test]
fn random_inputs_never_panic_and_accepted_ones_satisfy_limits() {
    let iters: usize = std::env::var("KS_PROTO_FUZZ_ITERS").ok().and_then(|s| s.parse().ok()).unwrap_or(100_000);
    let seeds = vec![
        frame(r#"{"type":"insert","text":"hello"}"#),
        frame(r#"{"type":"replace","text":"мир"}"#),
        frame(r#"{"type":"keys","keys":["Left","Enter","Tab"]}"#),
        frame(r#"{"type":"none"}"#),
        HostRequest { v: 1, request_id: ID, permissions: ET, input: Some(ProtoText::new("q".into())) }
            .to_frame()
            .unwrap(),
    ];
    let mut r = Rng(0x5EED_0000_0000_0001);
    let mut accepted = 0usize;
    for _ in 0..iters {
        let input = random_input(&mut r, &seeds);
        let perms = if r.below(2) == 0 { NO_PERMS } else { ET };
        if let Ok(v) = parse_guest_frame(&input, ID, perms) {
            accepted += 1;
            assert!(input.len() <= MAX_GUEST_FRAME_BYTES);
            assert_eq!(validate_action(v.action(), perms), Ok(()));
            match v.action() {
                Action::Insert { text } | Action::Replace { text } => {
                    assert!(text.char_len() <= MAX_OUTPUT_CHARS);
                    assert!(text.as_str().chars().all(|c| forbidden_class(c).is_none()));
                    assert!(perms.enter_tab || !text.as_str().contains(['\n', '\t']));
                }
                Action::Keys { keys } => {
                    assert!(!keys.is_empty() && keys.len() <= MAX_KEYS);
                    assert!(perms.enter_tab || keys.iter().all(|k| !k.needs_enter_tab()));
                }
                Action::None {} => {}
            }
        }
        let _ = HostRequest::from_frame(&input);
        let _ = framing::decode(&input, MAX_GUEST_FRAME_BYTES);
    }
    // The generator must exercise the accept path too, not only rejections.
    assert!(accepted > iters / 20, "accepted {accepted} of {iters}");
}
