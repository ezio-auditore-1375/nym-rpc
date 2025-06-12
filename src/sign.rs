use nym_bin_common::output_format::OutputFormat;
use nym_crypto::asymmetric::ed25519;
use nym_types::helpers::ConsoleSigningOutput;

fn print_signed_contract_msg(
    private_key: &ed25519::PrivateKey,
    raw_msg: &str,
    output: OutputFormat,
) {
    let trimmed = raw_msg.trim();
    eprintln!(">>> attempting to sign {trimmed}");

    let Ok(decoded) = bs58::decode(trimmed).into_vec() else {
        println!(
            "decoded: it seems you have incorrectly copied the message to sign. Make sure you didn't accidentally skip any characters"
        );
        return;
    };

    eprintln!(">>> decoding the message...");

    // we don't really care about what particular information is embedded inside of it,
    // we just want to know if user correctly copied the string, i.e. whether it's a valid bs58 encoded json
    if serde_json::from_slice::<serde_json::Value>(&decoded).is_err() {
        println!(
            "serde_json: it seems you have incorrectly copied the message to sign. Make sure you didn't accidentally skip any characters"
        );
        return;
    };

    // SAFETY:
    // if this is a valid json, it MUST be a valid string
    #[allow(clippy::unwrap_used)]
    let decoded_string = String::from_utf8(decoded.clone()).unwrap();
    let signature = private_key.sign(&decoded).to_base58_string();

    let sign_output = ConsoleSigningOutput::new(decoded_string, signature);
    println!("{}", output.format(&sign_output));
}

// SAFETY: clippy ArgGroup ensures only a single branch is actually called
#[allow(clippy::unreachable)]
pub async fn execute(private_key: &ed25519::PrivateKey, contract_msg: &str) -> anyhow::Result<()> {
    print_signed_contract_msg(private_key, contract_msg, OutputFormat::Text);
    Ok(())
}
