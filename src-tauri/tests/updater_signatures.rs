use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};

#[test]
#[ignore = "Requires packaged updater files in TETHER_UPDATER_ARTIFACTS"]
fn packaged_updates_match_the_embedded_key_and_reject_tampering() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let key_text = STANDARD
        .decode(config["plugins"]["updater"]["pubkey"].as_str().unwrap())
        .unwrap();
    let key = PublicKey::decode(std::str::from_utf8(&key_text).unwrap()).unwrap();
    let directory =
        std::env::var("TETHER_UPDATER_ARTIFACTS").expect("Set updater artifact directory");
    let mut verified = 0;
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "sig") {
            continue;
        }
        let signature_text = STANDARD
            .decode(std::fs::read_to_string(&path).unwrap().trim())
            .unwrap();
        let signature = Signature::decode(std::str::from_utf8(&signature_text).unwrap()).unwrap();
        let mut bytes = std::fs::read(path.with_extension("")).unwrap();
        key.verify(&bytes, &signature, false)
            .expect("Updater package must match the app's bundled public key");
        bytes[0] ^= 1;
        assert!(
            key.verify(&bytes, &signature, false).is_err(),
            "Tampered updates must be rejected"
        );
        verified += 1;
    }
    assert_eq!(
        verified, 1,
        "Each platform must contain exactly one signed updater package"
    );
}
