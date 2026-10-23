//! Trade amounts come from executed Anchor CPI events, never slippage limits or wallet deltas.
use crate::{model::*, schemas::*};
use anyhow::{Context, Result, bail, ensure};
use borsh::BorshDeserialize;
use serde::{Deserialize, Serialize};

// Anchor's EVENT_IX_TAG_LE (event self-CPI instruction prefix).
const EVENT_TAG: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Instruction {
    pub program: String,
    pub accounts: Vec<String>,
    pub data: Vec<u8>,
    pub stack_height: u32,
    pub succeeded: bool,
    pub outer_index: u32,
    pub inner_index: Option<u32>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Transaction {
    pub signature: String,
    pub slot: u64,
    pub failed: bool,
    /// Execution order, including outer instructions and inner instructions with stack heights.
    pub instructions: Vec<Instruction>,
}
#[derive(Default)]
pub struct Decoded {
    pub trades: Vec<Trade>,
    pub excluded_quotes: u64,
    pub failures: u64,
    pub trade_instructions: u64,
}

fn key(ix: &Instruction, at: usize) -> Result<&str> {
    ix.accounts
        .get(at)
        .map(String::as_str)
        .context("missing account")
}
fn parse_event(ix: &Instruction, parent: &Instruction, spec: TradeSpec) -> Result<Option<Trade>> {
    ensure!(ix.data.len() >= 16, "short CPI event");
    let disc = &ix.data[8..16];
    let mut payload = &ix.data[16..];
    let mint = key(parent, spec.mint)?;
    let quote = spec.quote.map(|i| key(parent, i)).transpose()?;
    if quote.is_some_and(|q| q != WSOL) {
        return Ok(None);
    }
    let (amount, timestamp, side) = if ix.program == PUMP {
        ensure!(disc == TRADEEVENT_DISC, "not TradeEvent");
        let e = TradeEvent::deserialize(&mut payload).context("unsupported Pump event layout")?;
        ensure!(
            bs58::encode(e.mint).into_string() == mint,
            "event mint mismatch"
        );
        ensure!(e.is_buy == spec.buy, "event side mismatch");
        // Legacy SOL events use sol_amount; quote-aware instructions use quote_amount.
        if quote.is_some() {
            ensure!(
                bs58::encode(e.quote_mint).into_string() == WSOL,
                "event quote mismatch"
            );
        }
        (
            if quote.is_some() {
                e.quote_amount
            } else {
                e.sol_amount
            },
            e.timestamp,
            if e.is_buy { Side::Buy } else { Side::Sell },
        )
    } else if spec.buy {
        ensure!(disc == BUYEVENT_DISC, "not BuyEvent");
        let e = BuyEvent::deserialize(&mut payload).context("unsupported AMM buy event layout")?;
        ensure!(
            Some(bs58::encode(e.pool).into_string().as_str())
                == spec.pool.map(|i| key(parent, i)).transpose()?,
            "event pool mismatch"
        );
        (e.quote_amount_in, e.timestamp, Side::Buy)
    } else {
        ensure!(disc == SELLEVENT_DISC, "not SellEvent");
        let e =
            SellEvent::deserialize(&mut payload).context("unsupported AMM sell event layout")?;
        ensure!(
            Some(bs58::encode(e.pool).into_string().as_str())
                == spec.pool.map(|i| key(parent, i)).transpose()?,
            "event pool mismatch"
        );
        (e.quote_amount_out, e.timestamp, Side::Sell)
    };
    let decoded_mint = bs58::decode(mint).into_vec()?;
    ensure!(decoded_mint.len() == 32, "invalid mint");
    Ok(Some(Trade {
        signature: String::new(),
        slot: 0,
        instruction_index: ix.outer_index,
        inner_instruction_index: ix.inner_index,
        mint: mint.into(),
        event_timestamp_ms: timestamp.checked_mul(1000).context("invalid timestamp")?,
        side,
        quote_lamports: amount,
        venue: if ix.program == PUMP {
            "PumpFun"
        } else {
            "PumpSwap"
        }
        .into(),
    }))
}

pub fn decode(tx: &Transaction) -> Decoded {
    let mut result = Decoded::default();
    if tx.failed {
        return result;
    }
    let mut stack: Vec<(&Instruction, Option<TradeSpec>, bool)> = Vec::new();
    for ix in &tx.instructions {
        if !ix.succeeded {continue;}
        // Unwind completed sibling calls. Missing heights cannot prove event ancestry.
        while stack.last().is_some_and(|(p, _, _)| {
            p.outer_index != ix.outer_index || p.stack_height >= ix.stack_height
        }) {
            let (_, spec, emitted) = stack.pop().unwrap();
            if spec.is_some() && !emitted {
                result.failures += 1;
            }
        }
        let specs = if ix.program == PUMP {
            PUMP_SPECS
        } else if ix.program == AMM {
            PUMP_AMM_SPECS
        } else {
            &[]
        };
        let spec = specs.iter().find(|s| ix.data.starts_with(&s.disc)).copied();
        if spec.is_some() {
            result.trade_instructions += 1;
        }
        if ix.data.starts_with(&EVENT_TAG) && (ix.program == PUMP || ix.program == AMM) {
            if let Some((parent, Some(s), emitted)) = stack.last_mut() {
                if parent.program == ix.program && ix.stack_height == parent.stack_height + 1 {
                    // Ignore other event kinds emitted by the same trade (incentives etc.).
                    let expected = if ix.program == PUMP {
                        TRADEEVENT_DISC
                    } else if s.buy {
                        BUYEVENT_DISC
                    } else {
                        SELLEVENT_DISC
                    };
                    if ix.data.get(8..16) == Some(expected.as_slice()) {
                        if *emitted {
                            result.failures += 1;
                            continue;
                        }
                        *emitted = true;
                        match parse_event(ix, parent, *s) {
                            Ok(Some(mut t)) => {
                                t.signature = tx.signature.clone();
                                t.slot = tx.slot;
                                result.trades.push(t);
                            }
                            Ok(None) => result.excluded_quotes += 1,
                            Err(e) => {
                                tracing::debug!(error=?e, "trade decode rejected");
                                result.failures += 1;
                            }
                        }
                    }
                }
            }
        }
        stack.push((ix, spec, false));
    }
    for (_, spec, emitted) in stack {
        if spec.is_some() && !emitted {
            result.failures += 1;
        }
    }
    result
}

/// Logs are used only to verify invocation completion, never as a source of amounts.
/// A caller can catch a failed CPI while the overall transaction still succeeds.
pub fn apply_log_outcomes(instructions: &mut [Instruction], logs: &[String]) -> Result<()> {
    let mut stack: Vec<(String,usize)> = vec![];
    let mut cursor=0;
    for line in logs {
        let Some(rest)=line.strip_prefix("Program ") else {continue};
        if let Some((program,depth))=rest.split_once(" invoke [") {
            let height: u32=depth.strip_suffix(']').context("invalid invocation log")?.parse()?;
            ensure!(height as usize==stack.len()+1,"incomplete invocation trace");
            let index=(cursor..instructions.len()).find(|i| instructions[*i].program==program && instructions[*i].stack_height==height).context("invocation not found in instruction trace")?;
            ensure!(!instructions[cursor..index].iter().any(|i|i.program==PUMP||i.program==AMM),"missing program invocation log");
            cursor=index+1;stack.push((program.into(),index));
        } else if rest.ends_with(" success") || rest.contains(" failed: ") {
            let program=rest.split_whitespace().next().context("missing program")?;
            let (expected,start)=stack.pop().context("unmatched program completion")?;
            ensure!(expected==program,"mismatched program completion");
            if rest.ends_with(" success") {instructions[start].succeeded=true;}
            else {for ix in &mut instructions[start..cursor] {ix.succeeded=false;}}
        }
    }
    ensure!(stack.is_empty(),"truncated invocation trace");
    ensure!(!instructions[cursor..].iter().any(|i|i.program==PUMP||i.program==AMM),"missing program completion");
    Ok(())
}

pub fn normalize(
    update: &helius_laserstream::grpc::SubscribeUpdateTransaction,
) -> Result<Transaction> {
    let info = update
        .transaction
        .as_ref()
        .context("missing transaction info")?;
    let tx = info.transaction.as_ref().context("missing transaction")?;
    let meta = info.meta.as_ref().context("missing metadata")?;
    if meta.err.is_some() {
        return Ok(Transaction {
            signature: bs58::encode(&info.signature).into_string(),
            slot: update.slot,
            failed: true,
            instructions: vec![],
        });
    }
    let msg = tx.message.as_ref().context("missing message")?;
    let keys: Vec<String> = msg
        .account_keys
        .iter()
        .chain(&meta.loaded_writable_addresses)
        .chain(&meta.loaded_readonly_addresses)
        .map(|k| bs58::encode(k).into_string())
        .collect();
    let mut instructions = vec![];
    let convert = |program: u32,
                   accounts: &[u8],
                   data: &[u8],
                   height: u32,
                   outer: u32,
                   inner: Option<u32>|
     -> Result<Instruction> {
        Ok(Instruction {
            program: keys
                .get(program as usize)
                .context("bad program index")?
                .clone(),
            accounts: accounts
                .iter()
                .map(|i| keys.get(*i as usize).cloned().context("bad account index"))
                .collect::<Result<_>>()?,
            data: data.to_vec(),
            stack_height: height,
            succeeded: false,
            outer_index: outer,
            inner_index: inner,
        })
    };
    for (i, ix) in msg.instructions.iter().enumerate() {
        instructions.push(convert(
            ix.program_id_index,
            &ix.accounts,
            &ix.data,
            1,
            i as u32,
            None,
        )?);
        for group in meta
            .inner_instructions
            .iter()
            .filter(|g| g.index == i as u32)
        {
            for (j, inner) in group.instructions.iter().enumerate() {
                let height = inner.stack_height.context("missing inner stack height")?;
                if height < 2 {
                    bail!("invalid inner height");
                }
                instructions.push(convert(
                    inner.program_id_index,
                    &inner.accounts,
                    &inner.data,
                    height,
                    i as u32,
                    Some(j as u32),
                )?);
            }
        }
    }
    apply_log_outcomes(&mut instructions, &meta.log_messages)?;
    Ok(Transaction {
        signature: bs58::encode(&info.signature).into_string(),
        slot: update.slot,
        failed: false,
        instructions,
    })
}
