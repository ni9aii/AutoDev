//! Manual live-API probe (NOT part of CI): verifies the OpenRouter-hosted
//! SystemOne endpoint works through kunobi-jev's base_url override.
//!
//! Run: TYPESAFE_API_KEY=$OPENROUTER_API_KEY cargo test -p autodev-jev \
//!      --test live_probe -- --ignored --nocapture
use kunobi_jev::blocking::Client;
use kunobi_jev::{choice, Questions, SystemOneRequest};

#[test]
#[ignore = "live API probe; run manually with OPENROUTER_API_KEY as TYPESAFE_API_KEY"]
fn live_openrouter_systemone() {
    let client = Client::builder()
        .configure(|b| b.base_url("https://openrouter.ai/api".to_string()))
        .build()
        .unwrap();
    let mut q = Questions::new();
    let k = q.add(
        "triage_0",
        choice(
            "Should this finding be fixed now?",
            [("do_now", "concrete defect"), ("defer", "refactor")],
        ),
    );
    let res = client.system_one(SystemOneRequest::new(
        "SQL injection in src/db.rs line 42",
        q,
    ));
    match res {
        Ok(r) => {
            let a = r.answer(&k).unwrap();
            println!("LIVE OK choice={} conf={}", a.choice, a.confidence);
        }
        Err(e) => panic!("LIVE ERR: {e}"),
    }
}
