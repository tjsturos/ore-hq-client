use base64::{prelude::BASE64_STANDARD, Engine};
use clap::Parser;
use colored::*;
use inquire::{InquireError, Text};
use reqwest::StatusCode;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};
use std::{str::FromStr, time::Duration};

#[derive(Debug, Parser)]
pub struct UnboostArgs {
    #[arg(
        long,
        value_name = "AMOUNT",
        help = "Amount of boost token to unstake."
    )]
    pub amount: f64,

    #[arg(long, value_name = "MINT", help = "Mint address of the boost token.")]
    pub mint: String,
}

pub async fn undelegate_boost(args: UnboostArgs, key: Keypair, url: String, unsecure: bool) {
    let base_url = url;
    let client = reqwest::Client::new();
    let url_prefix = if unsecure {
        "http".to_string()
    } else {
        "https".to_string()
    };

    // RED TEXT
    match Text::new(
        &format!(
            "  Are you sure you want to undelegate {} boost tokens? (Y/n or 'esc' to cancel)",
            args.amount
        )
        .red()
        .to_string(),
    )
    .prompt()
    {
        Ok(confirm) => {
            if confirm.trim().eq_ignore_ascii_case("esc") {
                println!("  Unboosting canceled.");
                return;
            } else if confirm.trim().is_empty() || confirm.trim().to_lowercase() == "y" {
                // Proceed with staking
            } else {
                println!("  Unboosting canceled.");
                return;
            }
        }
        Err(InquireError::OperationCanceled) => {
            println!("  Unboosting operation canceled.");
            return;
        }
        Err(_) => {
            println!("  Invalid input. Unboosting canceled.");
            return;
        }
    }

    let resp = client
        .get(format!(
            "{}://{}/pool/authority/pubkey",
            url_prefix, base_url
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let pool_pubkey = Pubkey::from_str(&resp).unwrap();

    let resp = client
        .get(format!(
            "{}://{}/pool/fee_payer/pubkey",
            url_prefix, base_url
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let fee_pubkey = Pubkey::from_str(&resp).unwrap();

    let resp = client
        .get(format!("{}://{}/latest-blockhash", url_prefix, base_url))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let decoded_blockhash = BASE64_STANDARD.decode(resp).unwrap();
    let deserialized_blockhash = bincode::deserialize(&decoded_blockhash).unwrap();

    let boost_amount_u64 =
        (args.amount * 10f64.powf(ore_api::consts::TOKEN_DECIMALS as f64)) as u64;
    let ix = ore_miner_delegation::instruction::undelegate_boost(
        key.pubkey(),
        pool_pubkey,
        Pubkey::from_str(&args.mint).unwrap(),
        boost_amount_u64,
    );

    let mut tx = Transaction::new_with_payer(&[ix], Some(&fee_pubkey));
    tx.partial_sign(&[&key], deserialized_blockhash);
    let serialized_tx = bincode::serialize(&tx).unwrap();
    let encoded_tx = BASE64_STANDARD.encode(&serialized_tx);

    let resp = client
        .post(format!(
            "{}://{}/unstake-boost?pubkey={}&mint={}&amount={}",
            url_prefix,
            base_url,
            key.pubkey().to_string(),
            args.mint,
            boost_amount_u64
        ))
        .body(encoded_tx)
        .send()
        .await;
    if let Ok(res) = resp {
        if let Ok(txt) = res.text().await {
            match txt.as_str() {
                "SUCCESS" => {
                    println!("  Successfully unstaked boost!");
                }
                other => {
                    println!("  Transaction failed: {}", other);
                }
            }
        } else {
            println!("  Transaction failed, please wait and try again.");
        }
    } else {
        println!("  Transaction failed, please wait and try again.");
    }
}

// Helper function to fetch server timestamp
async fn get_timestamp(client: &reqwest::Client, url_prefix: &str, base_url: &str) -> u64 {
    loop {
        if let Ok(response) = client
            .get(format!("{}://{}/timestamp", url_prefix, base_url))
            .send()
            .await
        {
            match response.status() {
                StatusCode::OK => {
                    if let Ok(ts) = response.text().await {
                        if let Ok(ts) = ts.parse::<u64>() {
                            return ts;
                        }
                    }
                }
                _ => {
                    println!("  Server restarting, trying again in 3 seconds...");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    continue;
                }
            }
        }
        println!("  Unable to retrieve timestamp, retrying in 3 seconds...");
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}
