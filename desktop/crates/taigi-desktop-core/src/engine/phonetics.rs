//! Phonetics slice of the engine bridge: the small conversions the
//! dictionary pages need (the search keys a custom entry is stored and
//! looked up under are the engine `userdata`'s own, behind the user-data
//! ops). Port of iOS `RustEngineBridge+Phonetics.swift`.

use protos::engine::{
    phonetics_request, phonetics_response, request, response, NfdPreprocessForLookup,
    PhoneticsRequest, PhoneticsResponse, StripTone, TlToPoj,
};

use super::bridge::{record_failure, roundtrip};

/// The POJ spelling of a TL reading, for rendering results while the user
/// is typing POJ.
pub fn tl_to_poj(input: &str) -> Option<String> {
    string_result(
        phonetics_request::Method::TlToPoj(TlToPoj {
            input: input.to_owned(),
        }),
        "tlToPoj",
    )
}

/// The syllable without its tone, and the tone digit that was on it.
pub fn strip_tone(input: &str) -> Option<(String, String)> {
    let op = "stripTone";
    let response = phonetics_response(
        phonetics_request::Method::StripTone(StripTone {
            input: input.to_owned(),
        }),
        op,
    )?;
    match response.result {
        Some(phonetics_response::Result::StripToneResult(result)) => {
            Some((result.bare, result.tone))
        }
        _ => {
            record_failure(op, "response carried no strip-tone result");
            None
        }
    }
}

/// Taigi-specific Unicode preprocessing before a lookup: the nasal marker
/// and `o͘` folded to the ASCII spellings the external dictionaries index by.
pub fn nfd_preprocess_for_lookup(input: &str) -> Option<String> {
    string_result(
        phonetics_request::Method::NfdPreprocessForLookup(NfdPreprocessForLookup {
            input: input.to_owned(),
        }),
        "nfdPreprocessForLookup",
    )
}

fn string_result(method: phonetics_request::Method, op: &str) -> Option<String> {
    let response = phonetics_response(method, op)?;
    match response.result {
        Some(phonetics_response::Result::StringResult(result)) => Some(result.output),
        _ => {
            record_failure(op, "response carried no string result");
            None
        }
    }
}

fn phonetics_response(method: phonetics_request::Method, op: &str) -> Option<PhoneticsResponse> {
    let payload = request::Payload::Phonetics(PhoneticsRequest {
        method: Some(method),
    });
    match roundtrip(payload, op, 0, None)? {
        response::Payload::Phonetics(response) => Some(response),
        other => {
            record_failure(op, &format!("expected a phonetics payload, got {other:?}"));
            None
        }
    }
}
