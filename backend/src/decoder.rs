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
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TraceReport {
    pub incomplete: bool,
    pub unverified_trade_calls: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Transaction {
    pub signature: String,
    pub slot: u64,
    pub failed: bool,
    #[serde(default)]
    pub trace: TraceReport,
    /// Execution order, including outer instructions and inner instructions with stack heights.
    pub instructions: Vec<Instruction>,
}
#[derive(Default)]
pub struct Decoded {
    pub trades: Vec<Trade>,
    pub excluded_quotes: u64,
    pub failures: u64,
    pub trade_instructions: u64,
    pub trace: TraceReport,
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
        if e.quote_mint != [0; 32] && bs58::encode(e.quote_mint).into_string() != WSOL {
            return Ok(None);
        }
        // Legacy SOL events use sol_amount; quote-aware instructions use quote_amount.
        if quote.is_some() {
            ensure!(
                e.quote_mint == [0; 32] || bs58::encode(e.quote_mint).into_string() == WSOL,
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
        (
            e.quote_amount_in_with_lp_fee
                .checked_sub(e.lp_fee)
                .context("LP fee exceeds quote credit")?,
            e.timestamp,
            Side::Buy,
        )
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

#[derive(Clone, Copy)]
enum Call {
    Single(TradeSpec),
    Route,
    Boost,
}
fn event_trade(
    ix: &Instruction,
    mint: String,
    amount: u64,
    timestamp: i64,
    side: Side,
) -> Result<Trade> {
    ensure!(bs58::decode(&mint).into_vec()?.len() == 32, "invalid mint");
    Ok(Trade {
        signature: String::new(),
        slot: 0,
        instruction_index: ix.outer_index,
        inner_instruction_index: ix.inner_index,
        mint,
        event_timestamp_ms: timestamp.checked_mul(1000).context("invalid timestamp")?,
        side,
        quote_lamports: amount,
        venue: if ix.program == PUMP {
            "PumpFun"
        } else {
            "PumpSwap"
        }
        .into(),
    })
}
fn extended_event(ix: &Instruction, parent: &Instruction, call: Call) -> Result<Option<Trade>> {
    let disc = ix.data.get(8..16).context("missing event discriminator")?;
    let mut payload = &ix.data[16..];
    if disc == POSTCOMPLETEBUYEVENT_DISC && ix.program == PUMP {
        let e = PostCompleteBuyEvent::deserialize(&mut payload)?;
        let mint = bs58::encode(e.mint).into_string();
        let quote = bs58::encode(e.quote_mint).into_string();
        ensure!(
            parent.accounts.contains(&mint),
            "post-completion mint not in call"
        );
        if e.quote_mint != [0; 32] && quote != WSOL {
            return Ok(None);
        }
        return Ok(Some(event_trade(
            ix,
            mint,
            e.quote_in,
            e.timestamp,
            Side::Buy,
        )?));
    }
    match call {
        Call::Single(spec) => parse_event(ix, parent, spec),
        Call::Boost => {
            let e = BoostBuyAndBurnEvent::deserialize(&mut payload)?;
            ensure!(
                bs58::encode(e.pool).into_string() == key(parent, 0)?,
                "boost pool mismatch"
            );
            let mint = bs58::encode(e.mint).into_string();
            ensure!(mint == key(parent, 3)?, "boost mint mismatch");
            if key(parent, 4)? != WSOL {
                return Ok(None);
            }
            Ok(Some(event_trade(
                ix,
                mint,
                e.quote_amount_in_used,
                e.timestamp,
                Side::Buy,
            )?))
        }
        Call::Route => {
            if ix.program == PUMP {
                let e = TradeEvent::deserialize(&mut payload)?;
                let mint = bs58::encode(e.mint).into_string();
                let mint_index = parent
                    .accounts
                    .iter()
                    .position(|a| a == &mint)
                    .context("route mint missing")?;
                if e.quote_mint != [0; 32] && bs58::encode(e.quote_mint).into_string() != WSOL {
                    return Ok(None);
                }
                let quote_index = parent
                    .accounts
                    .iter()
                    .position(|a| a == WSOL)
                    .context("route SOL quote missing")?;
                parse_event(
                    ix,
                    parent,
                    TradeSpec {
                        disc: [0; 8],
                        buy: e.is_buy,
                        mint: mint_index,
                        quote: Some(quote_index),
                        pool: None,
                    },
                )
            } else {
                let buy = disc == BUYEVENT_DISC;
                let pool = if buy {
                    BuyEvent::deserialize(&mut payload)?.pool
                } else {
                    SellEvent::deserialize(&mut payload)?.pool
                };
                let pool = bs58::encode(pool).into_string();
                // Official multi-hop layout: 16 fixed accounts, then five per hop.
                let start = (16..parent.accounts.len())
                    .step_by(5)
                    .find(|i| parent.accounts.get(i + 2) == Some(&pool))
                    .context("route pool missing")?;
                parse_event(
                    ix,
                    parent,
                    TradeSpec {
                        disc: [0; 8],
                        buy,
                        mint: start,
                        quote: Some(start + 1),
                        pool: Some(start + 2),
                    },
                )
            }
        }
    }
}
fn classify_call(ix: &Instruction) -> Option<Call> {
    let specs = if ix.program == PUMP {
        PUMP_SPECS
    } else if ix.program == AMM {
        PUMP_AMM_SPECS
    } else {
        &[]
    };
    if (ix.program == PUMP && ix.data.starts_with(&MULTI_HOP_CURVE_SWAP_DISC))
        || (ix.program == AMM && ix.data.starts_with(&MULTI_HOP_SWAP_DISC))
    {
        Some(Call::Route)
    } else if ix.program == AMM && ix.data.starts_with(&BOOST_BUY_AND_BURN_DISC) {
        Some(Call::Boost)
    } else {
        specs
            .iter()
            .find(|s| ix.data.starts_with(&s.disc))
            .copied()
            .map(Call::Single)
    }
}
fn could_affect_sol(ix: &Instruction, call: Call) -> bool {
    match call {
        Call::Single(spec) => spec
            .quote
            .is_none_or(|i| ix.accounts.get(i).is_none_or(|q| q == WSOL)),
        Call::Boost => ix.accounts.get(4).is_none_or(|q| q == WSOL),
        Call::Route => ix.accounts.len() < 16 || ix.accounts.iter().any(|a| a == WSOL),
    }
}
pub fn decode(tx: &Transaction) -> Decoded {
    let mut result = Decoded {
        trace: tx.trace.clone(),
        ..Default::default()
    };
    if tx.failed {
        return result;
    }
    let mut stack: Vec<(&Instruction, Option<Call>, usize)> = vec![];
    let missing = |ix: &Instruction, call: Option<Call>, count: usize| {
        matches!(call, Some(Call::Single(_) | Call::Boost))
            && call.is_some_and(|c| could_affect_sol(ix, c))
            && count == 0
    };
    for ix in &tx.instructions {
        if !ix.succeeded {
            continue;
        }
        while stack.last().is_some_and(|(p, _, _)| {
            p.outer_index != ix.outer_index || p.stack_height >= ix.stack_height
        }) {
            let (parent, call, count) = stack.pop().unwrap();
            if missing(parent, call, count) {
                result.failures += 1;
            }
        }
        let call = classify_call(ix);
        if call.is_some() {
            result.trade_instructions += 1;
        }
        let mut handled_event = false;
        if ix.data.starts_with(&EVENT_TAG)
            && (ix.program == PUMP || ix.program == AMM)
            && let Some((parent, Some(parent_call), count)) = stack.last_mut()
            && parent.program == ix.program
            && ix.stack_height == parent.stack_height + 1
        {
            let disc = ix.data.get(8..16);
            let recognized = match parent_call {
                Call::Single(s) => {
                    disc == Some(if ix.program == PUMP {
                        TRADEEVENT_DISC.as_slice()
                    } else if s.buy {
                        BUYEVENT_DISC.as_slice()
                    } else {
                        SELLEVENT_DISC.as_slice()
                    })
                }
                Call::Boost => disc == Some(BOOSTBUYANDBURNEVENT_DISC.as_slice()),
                Call::Route => {
                    if ix.program == PUMP {
                        disc == Some(TRADEEVENT_DISC.as_slice())
                    } else {
                        disc == Some(BUYEVENT_DISC.as_slice())
                            || disc == Some(SELLEVENT_DISC.as_slice())
                    }
                }
            } || (ix.program == PUMP
                && disc == Some(POSTCOMPLETEBUYEVENT_DISC.as_slice()));
            if recognized {
                handled_event = true;
                *count += 1;
                match extended_event(ix, parent, *parent_call) {
                    Ok(Some(mut t)) => {
                        t.signature = tx.signature.clone();
                        t.slot = tx.slot;
                        result.trades.push(t);
                    }
                    Ok(None) => result.excluded_quotes += 1,
                    Err(e) => {
                        tracing::debug!(error=?e,"trade decode rejected");
                        result.failures += 1;
                    }
                }
            }
        }
        if !handled_event
            && ix.data.starts_with(&EVENT_TAG)
            && ((ix.program == PUMP
                && [TRADEEVENT_DISC, POSTCOMPLETEBUYEVENT_DISC]
                    .iter()
                    .any(|d| ix.data.get(8..16) == Some(d.as_slice())))
                || (ix.program == AMM
                    && [BUYEVENT_DISC, SELLEVENT_DISC, BOOSTBUYANDBURNEVENT_DISC]
                        .iter()
                        .any(|d| ix.data.get(8..16) == Some(d.as_slice()))))
        {
            result.failures += 1;
        }
        stack.push((ix, call, 0));
    }
    for (parent, call, count) in stack {
        if missing(parent, call, count) {
            result.failures += 1;
        }
    }
    result
}

/// Preserve verified branches when diagnostics are truncated. Transaction success proves
/// outer calls succeeded; inner calls also need their own and every ancestor's outcome.
/// Call only after transaction metadata confirms success.
pub fn apply_log_outcomes(
    instructions: &mut [Instruction],
    logs: &[String],
) -> Result<TraceReport> {
    let mut outcomes = vec![None; instructions.len()];
    for (i, ix) in instructions.iter().enumerate() {
        if ix.stack_height == 1 {
            outcomes[i] = Some(true);
        }
    }
    let mut stack: Vec<(&str, usize)> = vec![];
    let mut cursor = 0;
    let mut report = TraceReport::default();
    for line in logs {
        if line == "Log truncated" {
            report.incomplete = true;
            break;
        }
        let Some(rest) = line.strip_prefix("Program ") else {
            continue;
        };
        let Some((program, action)) = rest.split_once(' ') else {
            continue;
        };
        if matches!(program, "log:" | "data:" | "return:") {
            continue;
        }
        if let Some(depth) = action.strip_prefix("invoke [") {
            let height = depth.strip_suffix(']').and_then(|d| d.parse::<u32>().ok());
            let Some(height) = height else {
                report.incomplete = true;
                break;
            };
            if height as usize != stack.len() + 1 {
                report.incomplete = true;
                break;
            }
            let index = (cursor..instructions.len()).find(|i| {
                instructions[*i].program == program && instructions[*i].stack_height == height
            });
            let Some(index) = index else {
                report.incomplete = true;
                break;
            };
            if instructions[cursor..index]
                .iter()
                .any(|i| i.program == PUMP || i.program == AMM)
            {
                report.incomplete = true;
                break;
            }
            cursor = index + 1;
            stack.push((program, index));
        } else if action == "success" || action.starts_with("failed: ") {
            let Some((expected, start)) = stack.pop() else {
                report.incomplete = true;
                break;
            };
            if expected != program {
                report.incomplete = true;
                break;
            }
            outcomes[start] = Some(action == "success");
        }
    }
    report.incomplete |= !stack.is_empty();
    let mut ancestors: Vec<usize> = vec![];
    let mut unverified = std::collections::HashSet::new();
    for i in 0..instructions.len() {
        let height = instructions[i].stack_height;
        while ancestors
            .last()
            .is_some_and(|p| instructions[*p].stack_height >= height)
        {
            ancestors.pop();
        }
        ensure!(
            height > 0 && ancestors.len() + 1 == height as usize,
            "invalid instruction ancestry"
        );
        ensure!(
            ancestors
                .last()
                .is_none_or(|p| instructions[*p].outer_index == instructions[i].outer_index),
            "inner instruction crosses outer call"
        );
        let failed =
            outcomes[i] == Some(false) || ancestors.iter().any(|p| outcomes[*p] == Some(false));
        let verified =
            outcomes[i] == Some(true) && ancestors.iter().all(|p| outcomes[*p] == Some(true));
        instructions[i].succeeded = verified;
        if !failed && !verified {
            report.incomplete = true;
            if classify_call(&instructions[i])
                .is_some_and(|c| could_affect_sol(&instructions[i], c))
            {
                unverified.insert(i);
            }
            if (instructions[i].program == PUMP || instructions[i].program == AMM)
                && instructions[i].data.starts_with(&EVENT_TAG)
                && let Some(parent) = ancestors.last()
                && classify_call(&instructions[*parent])
                    .is_some_and(|c| could_affect_sol(&instructions[*parent], c))
            {
                unverified.insert(*parent);
            }
        }
        ancestors.push(i);
    }
    report.unverified_trade_calls = unverified.len() as u64;
    Ok(report)
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
            trace: TraceReport::default(),
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
    let trace = apply_log_outcomes(&mut instructions, &meta.log_messages)?;
    Ok(Transaction {
        signature: bs58::encode(&info.signature).into_string(),
        slot: update.slot,
        failed: false,
        trace,
        instructions,
    })
}
