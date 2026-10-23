use ballpump::{
    aggregate::Window,
    decoder::{self, Instruction, Transaction},
    model::*,
};
use serde_json::Value;
fn observed(file: &str) -> Transaction {
    decoder::normalize(&proto(file)).unwrap()
}
fn rpc(file: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(format!("../fixtures/{file}.rpc.json")).unwrap())
        .unwrap()
}
fn proto(file: &str) -> helius_laserstream::grpc::SubscribeUpdateTransaction {
    use helius_laserstream::{grpc::*, solana::storage::confirmed_block as pb};
    let root = rpc(file);
    let r = &root["result"];
    let msg = &r["transaction"]["message"];
    let meta = &r["meta"];
    let bytes = |v: &Value| bs58::decode(v.as_str().unwrap()).into_vec().unwrap();
    let accounts = |v: &Value| {
        v.as_array()
            .unwrap()
            .iter()
            .map(|i| i.as_u64().unwrap() as u8)
            .collect()
    };
    SubscribeUpdateTransaction {
        slot: r["slot"].as_u64().unwrap(),
        transaction: Some(SubscribeUpdateTransactionInfo {
            signature: bytes(&r["transaction"]["signatures"][0]),
            transaction: Some(pb::Transaction {
                message: Some(pb::Message {
                    account_keys: msg["accountKeys"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(bytes)
                        .collect(),
                    instructions: msg["instructions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|ix| pb::CompiledInstruction {
                            program_id_index: ix["programIdIndex"].as_u64().unwrap() as u32,
                            accounts: accounts(&ix["accounts"]),
                            data: bytes(&ix["data"]),
                        })
                        .collect(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            meta: Some(pb::TransactionStatusMeta {
                loaded_writable_addresses: meta["loadedAddresses"]["writable"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(bytes)
                    .collect(),
                loaded_readonly_addresses: meta["loadedAddresses"]["readonly"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(bytes)
                    .collect(),
                inner_instructions: meta["innerInstructions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|g| pb::InnerInstructions {
                        index: g["index"].as_u64().unwrap() as u32,
                        instructions: g["instructions"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|ix| pb::InnerInstruction {
                                program_id_index: ix["programIdIndex"].as_u64().unwrap() as u32,
                                accounts: accounts(&ix["accounts"]),
                                data: bytes(&ix["data"]),
                                stack_height: Some(ix["stackHeight"].as_u64().unwrap() as u32),
                            })
                            .collect(),
                    })
                    .collect(),
                log_messages: meta["logMessages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_str().unwrap().to_string())
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
#[test]
fn observed_mainnet_trades() {
    for (file, side, amount) in [
        ("observed-pump-0", Side::Sell, 58_226_455),
        ("observed-pump-sol-0", Side::Buy, 111_288_166),
        ("observed-pump-sol-1", Side::Buy, 110_894_027),
        ("observed-amm-sol-0", Side::Sell, 13_316_584),
        ("observed-amm-sol-1", Side::Buy, 12_954_823),
        ("observed-amm-sol-2", Side::Buy, 56_147_779),
        ("observed-amm-sol-3", Side::Buy, 14_173),
    ] {
        let d = decoder::decode(&observed(file));
        assert_eq!(d.failures, 0, "{file}");
        assert_eq!(d.trades.len(), 1, "{file}");
        assert_eq!(d.trades[0].side, side, "{file}");
        assert_eq!(d.trades[0].quote_lamports, amount, "{file}");
    }
}
#[test]
fn exclude_non_sol_and_mentions() {
    for file in [
        "observed-pump-1",
        "observed-pump-2",
        "observed-amm-0",
        "observed-amm-1",
    ] {
        let d = decoder::decode(&observed(file));
        assert!(d.trades.is_empty());
        assert_eq!(d.excluded_quotes, 1);
        assert_eq!(d.failures, 0);
    }
    let d = decoder::decode(&observed("observed-amm-2"));
    assert!(d.trades.is_empty());
    assert_eq!(d.trade_instructions, 0);
}
#[test]
fn pump_amount_matches_curve_balance_change() {
    let r = rpc("observed-pump-0");
    let r = &r["result"];
    let account = r["transaction"]["message"]["instructions"][2]["accounts"][3]
        .as_u64()
        .unwrap() as usize;
    let delta = r["meta"]["preBalances"][account].as_u64().unwrap()
        - r["meta"]["postBalances"][account].as_u64().unwrap();
    assert_eq!(
        decoder::decode(&observed("observed-pump-0")).trades[0].quote_lamports,
        delta
    );
}
#[test]
fn amm_amount_matches_quote_vault_change_less_lp_fee() {
    for file in [
        "observed-amm-sol-0",
        "observed-amm-sol-1",
        "observed-amm-sol-2",
        "observed-amm-sol-3",
    ] {
        let raw = proto(file);
        let info = raw.transaction.unwrap();
        let msg = info.transaction.unwrap().message.unwrap();
        let meta = info.meta.unwrap();
        let tx = observed(file);
        let trade = decoder::decode(&tx).trades.remove(0);
        let parent = tx
            .instructions
            .iter()
            .find(|ix| ix.program == AMM && ix.accounts.len() > 10)
            .unwrap();
        let all = msg
            .account_keys
            .iter()
            .chain(&meta.loaded_writable_addresses)
            .chain(&meta.loaded_readonly_addresses)
            .map(|k| bs58::encode(k).into_string())
            .collect::<Vec<_>>();
        let vault = all.iter().position(|k| k == &parent.accounts[8]).unwrap();
        let r = rpc(file);
        let meta = &r["result"]["meta"];
        let balance = |name: &str| {
            meta[name]
                .as_array()
                .unwrap()
                .iter()
                .find(|b| b["accountIndex"] == vault)
                .unwrap()["uiTokenAmount"]["amount"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap()
        };
        let pre = balance("preTokenBalances");
        let post = balance("postTokenBalances");
        let event = tx
            .instructions
            .iter()
            .find(|ix| {
                ix.program == AMM && ix.data.starts_with(&[228, 69, 165, 46, 81, 203, 154, 29])
            })
            .unwrap();
        // Stable BuyEvent/SellEvent prefix: LP fee at event payload offset 72.
        let lp = u64::from_le_bytes(event.data[88..96].try_into().unwrap());
        if trade.side == Side::Buy {
            assert_eq!(post - pre, trade.quote_lamports + lp, "{file}");
        } else {
            assert_eq!(pre - post, trade.quote_lamports - lp, "{file}");
        }
    }
}
#[test]
fn failed_malformed_and_unattributed_events() {
    let mut tx = observed("observed-pump-0");
    tx.failed = true;
    assert!(decoder::decode(&tx).trades.is_empty());
    tx.failed = false;
    let event = tx
        .instructions
        .iter_mut()
        .find(|i| i.data.starts_with(&[228, 69, 165, 46, 81, 203, 154, 29]))
        .unwrap();
    event.data.truncate(25);
    let d = decoder::decode(&tx);
    assert!(d.trades.is_empty());
    assert_eq!(d.failures, 1);
    let mut tx = observed("observed-pump-0");
    tx.instructions.retain(|i| i.inner_index.is_some());
    assert!(decoder::decode(&tx).trades.is_empty());
    let mut p = proto("observed-pump-0");
    p.transaction
        .as_mut()
        .unwrap()
        .meta
        .as_mut()
        .unwrap()
        .log_messages
        .pop();
    assert!(decoder::normalize(&p).is_err());
}
#[test]
fn multiple_swaps_and_restart_replay_are_idempotent() {
    let mut tx = observed("observed-pump-sol-0");
    let mut second = tx.instructions.clone();
    for ix in &mut second {
        ix.outer_index += 10;
    }
    tx.instructions.extend(second);
    let d = decoder::decode(&tx);
    assert_eq!(d.trades.len(), 2);
    assert_ne!(d.trades[0].key(), d.trades[1].key());
    let now = d.trades[0].event_timestamp_ms + 1000;
    let mut live = Window::default();
    for trade in d.trades.iter().chain(d.trades.iter()) {
        live.insert(trade.clone(), now);
    }
    let mut restarted = Window::default();
    for trade in d.trades {
        restarted.insert(trade, now);
    }
    assert_eq!(
        serde_json::to_value(live.snapshot("warming".into(), false)).unwrap(),
        serde_json::to_value(restarted.snapshot("warming".into(), false)).unwrap()
    );
    let snapshot = live.snapshot("warming".into(), false);
    assert_eq!(snapshot.net_flow[0].trade_count, 2);
    assert_eq!(snapshot.net_flow[0].net_lamports, "222576332");
}
#[test]
fn caught_cpi_failure_rolls_back_emitted_event() {
    let mut instructions = vec![
        Instruction {
            program: "router".into(),
            accounts: vec![],
            data: vec![],
            stack_height: 1,
            succeeded: false,
            outer_index: 0,
            inner_index: None,
        },
        Instruction {
            program: PUMP.into(),
            accounts: vec![],
            data: vec![],
            stack_height: 2,
            succeeded: false,
            outer_index: 0,
            inner_index: Some(0),
        },
        Instruction {
            program: PUMP.into(),
            accounts: vec![],
            data: vec![],
            stack_height: 3,
            succeeded: false,
            outer_index: 0,
            inner_index: Some(1),
        },
    ];
    let logs = vec![
        "Program router invoke [1]".into(),
        format!("Program {PUMP} invoke [2]"),
        format!("Program {PUMP} invoke [3]"),
        format!("Program {PUMP} success"),
        format!("Program {PUMP} failed: custom program error: 1"),
        "Program router success".into(),
    ];
    decoder::apply_log_outcomes(&mut instructions, &logs).unwrap();
    assert!(instructions[0].succeeded);
    assert!(!instructions[1].succeeded);
    assert!(!instructions[2].succeeded);
}

#[test]
fn schema_derived_route_boost_and_post_completion_events() {
    use ballpump::schemas::*;
    let mint = bs58::encode([7u8; 32]).into_string();
    let pool = bs58::encode([8u8; 32]).into_string();
    let make = |disc: [u8; 8], data: Vec<u8>, program: &str, accounts: Vec<String>| {
        let mut event = vec![228, 69, 165, 46, 81, 203, 154, 29];
        event.extend(disc);
        event.extend(data);
        vec![
            Instruction {
                program: program.into(),
                accounts,
                data: vec![],
                stack_height: 1,
                succeeded: true,
                outer_index: 0,
                inner_index: None,
            },
            Instruction {
                program: program.into(),
                accounts: vec![],
                data: event,
                stack_height: 2,
                succeeded: true,
                outer_index: 0,
                inner_index: Some(0),
            },
        ]
    };
    let mut accounts = vec![String::new(); 21];
    accounts[16] = mint.clone();
    accounts[17] = WSOL.into();
    accounts[18] = pool.clone();
    let event = BuyEvent {
        pool: [8; 32],
        timestamp: 100,
        quote_amount_in: 999_999,
        quote_amount_in_with_lp_fee: 110,
        lp_fee: 10,
        ..Default::default()
    };
    let mut ixs = make(BUYEVENT_DISC, borsh::to_vec(&event).unwrap(), AMM, accounts);
    ixs[0].data = MULTI_HOP_SWAP_DISC.to_vec();
    let tx = Transaction {
        signature: "route".into(),
        slot: 1,
        failed: false,
        instructions: ixs,
    };
    let d = decoder::decode(&tx);
    assert_eq!(d.failures, 0);
    assert_eq!(d.trades[0].mint, mint);
    assert_eq!(d.trades[0].quote_lamports, 100);
    let mut accounts = vec![String::new(); 5];
    accounts[0] = pool;
    accounts[3] = mint.clone();
    accounts[4] = WSOL.into();
    let event = BoostBuyAndBurnEvent {
        pool: [8; 32],
        mint: [7; 32],
        timestamp: 100,
        quote_amount_in_requested: 999,
        quote_amount_in_used: 150,
        ..Default::default()
    };
    let mut ixs = make(
        BOOSTBUYANDBURNEVENT_DISC,
        borsh::to_vec(&event).unwrap(),
        AMM,
        accounts,
    );
    ixs[0].data = BOOST_BUY_AND_BURN_DISC.to_vec();
    let d = decoder::decode(&Transaction {
        signature: "boost".into(),
        slot: 1,
        failed: false,
        instructions: ixs,
    });
    assert_eq!(d.failures, 0);
    assert_eq!(d.trades[0].quote_lamports, 150);
    let event = TradeEvent {
        mint: [7; 32],
        quote_mint: bs58::decode(WSOL).into_vec().unwrap().try_into().unwrap(),
        quote_amount: 200,
        is_buy: true,
        timestamp: 100,
        ..Default::default()
    };
    let mut ixs = make(
        TRADEEVENT_DISC,
        borsh::to_vec(&event).unwrap(),
        PUMP,
        vec![mint.clone(), WSOL.into()],
    );
    ixs[0].data = MULTI_HOP_CURVE_SWAP_DISC.to_vec();
    let extra = PostCompleteBuyEvent {
        mint: [7; 32],
        quote_mint: [0; 32],
        timestamp: 100,
        quote_in: 300,
        ..Default::default()
    };
    let mut bytes = vec![228, 69, 165, 46, 81, 203, 154, 29];
    bytes.extend(POSTCOMPLETEBUYEVENT_DISC);
    bytes.extend(borsh::to_vec(&extra).unwrap());
    ixs.push(Instruction {
        program: PUMP.into(),
        accounts: vec![],
        data: bytes,
        stack_height: 2,
        succeeded: true,
        outer_index: 0,
        inner_index: Some(1),
    });
    let d = decoder::decode(&Transaction {
        signature: "curve-route".into(),
        slot: 1,
        failed: false,
        instructions: ixs,
    });
    assert_eq!(d.failures, 0);
    assert_eq!(d.trades.len(), 2);
    assert_eq!(d.trades.iter().map(|t| t.quote_lamports).sum::<u64>(), 500);
}
