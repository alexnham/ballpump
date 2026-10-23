use ballpump::{
    aggregate::Window,
    decoder::{self, Instruction, Transaction},
    model::*,
};
use serde_json::Value;
fn observed(file: &str) -> Transaction {
    let root: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("../fixtures/{file}.rpc.json")).unwrap(),
    )
    .unwrap();
    let r = &root["result"];
    let msg = &r["transaction"]["message"];
    let meta = &r["meta"];
    let mut keys: Vec<String> = msg["accountKeys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k.as_str().unwrap().into())
        .collect();
    for name in ["writable", "readonly"] {
        if let Some(a) = meta["loadedAddresses"][name].as_array() {
            keys.extend(a.iter().map(|v| v.as_str().unwrap().into()));
        }
    }
    let make = |ix: &Value, outer: u32, inner: Option<u32>, height: u32| Instruction {
        program: keys[ix["programIdIndex"].as_u64().unwrap() as usize].clone(),
        accounts: ix["accounts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| keys[i.as_u64().unwrap() as usize].clone())
            .collect(),
        data: bs58::decode(ix["data"].as_str().unwrap())
            .into_vec()
            .unwrap(),
        outer_index: outer,
        inner_index: inner,
        stack_height: height,
        succeeded: false,
    };
    let mut instructions = vec![];
    for (i, ix) in msg["instructions"].as_array().unwrap().iter().enumerate() {
        instructions.push(make(ix, i as u32, None, 1));
        for group in meta["innerInstructions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|g| g["index"] == i)
        {
            for (j, ix) in group["instructions"].as_array().unwrap().iter().enumerate() {
                instructions.push(make(
                    ix,
                    i as u32,
                    Some(j as u32),
                    ix["stackHeight"].as_u64().unwrap() as u32,
                ));
            }
        }
    }
    decoder::apply_log_outcomes(&mut instructions, &meta["logMessages"].as_array().unwrap().iter().map(|s|s.as_str().unwrap().to_string()).collect::<Vec<_>>()).unwrap();
    Transaction {
        signature: r["transaction"]["signatures"][0].as_str().unwrap().into(),
        slot: r["slot"].as_u64().unwrap(),
        failed: !meta["err"].is_null(),
        instructions,
    }
}
#[test]
fn observed_mainnet_trades() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();
    for file in [
        "observed-pump-0",
        "observed-pump-1",
        "observed-pump-2",
        "observed-amm-0",
        "observed-amm-1",
        "observed-amm-2",
        "observed-pump-sol-0", "observed-pump-sol-1",
        "observed-amm-sol-0", "observed-amm-sol-1", "observed-amm-sol-2", "observed-amm-sol-3",
    ] {
        let tx = observed(file);
        let d = decoder::decode(&tx);
        println!(
            "{file}: instructions={} trades={} failures={} excluded={} {:?}",
            d.trade_instructions,
            d.trades.len(),
            d.failures,
            d.excluded_quotes,
            d.trades
        );
        assert_eq!(d.failures, 0, "{file}");
    }
}
