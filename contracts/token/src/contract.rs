use std::ops::{Div, Mul, Sub};

use cosmwasm_std::{Addr, Binary, Decimal, Deps, DepsMut, Env, from_binary, MessageInfo, Response, StdError, StdResult, Storage, to_binary, Uint128};
use cosmwasm_std::entry_point;
use cw20_base::allowances::{
    execute_burn_from as cw20_execute_burn_from, execute_decrease_allowance as cw20_execute_decrease_allowance,
    execute_increase_allowance as cw20_execute_increase_allowance, execute_send_from as cw20_execute_send_from,
    execute_transfer_from as cw20_execute_transfer_from, query_allowance,
};
use cw20_base::contract::{
    create_accounts, execute_burn as cw20_execute_burn, execute_mint as cw20_execute_mint,
    execute_send as cw20_execute_send, execute_transfer as cw20_execute_transfer, query_balance,
    query_minter, query_token_info,
};
use cw20_base::ContractError;
use cw20_base::enumerable::{query_all_accounts, query_all_allowances};
use cw20_base::state::{BALANCES, MinterData, TOKEN_INFO, TokenInfo};
use cw2::set_contract_version;
use terraswap::pair::Cw20HookMsg;

use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, SwapFeeConfigResponse};
use crate::state::{SWAP_FEE_CONFIG, SwapFeeConfig};

// version info for migration info
const CONTRACT_NAME: &str = "luart-token";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // check valid token info
    msg.validate()?;

    // create initial accounts
    let total_supply = create_accounts(&mut deps, &msg.initial_balances)?;

    if let Some(limit) = msg.get_cap() {
        if total_supply > limit {
            return Err(StdError::generic_err("Initial supply greater than cap"));
        }
    }

    let mint = match msg.mint {
        Some(m) => Some(MinterData {
            minter: deps.api.addr_validate(&m.minter)?,
            cap: m.cap,
        }),
        None => None,
    };

    // store token info
    let data = TokenInfo {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply,
        mint,
    };

    TOKEN_INFO.save(deps.storage, &data)?;

    if let Some(swap_fee_config) = msg.swap_fee_config {
        let data = SwapFeeConfig {
            fee_admin: deps.api.addr_validate(&swap_fee_config.fee_admin)?,
            enable_swap_fee: swap_fee_config.enable_swap_fee,
            swap_percent_fee: swap_fee_config.swap_percent_fee,
            fee_receiver: deps.api.addr_validate(&swap_fee_config.fee_receiver)?,
        };
        SWAP_FEE_CONFIG.save(deps.storage, &data)?;
    }

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer { recipient, amount } => {
            cw20_execute_transfer(deps, env, info, recipient, amount)
        }
        ExecuteMsg::Burn { amount } => cw20_execute_burn(deps, env, info, amount),
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => execute_send(deps, env, info, contract, amount, msg),
        ExecuteMsg::Mint { recipient, amount } => cw20_execute_mint(deps, env, info, recipient, amount),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => cw20_execute_increase_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => cw20_execute_decrease_allowance(deps, env, info, spender, amount, expires),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => cw20_execute_transfer_from(deps, env, info, owner, recipient, amount),
        ExecuteMsg::BurnFrom { owner, amount } => cw20_execute_burn_from(deps, env, info, owner, amount),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => execute_send_from(deps, env, info, owner, contract, amount, msg),
        ExecuteMsg::UpdateSwapFeeConfig {
            fee_admin,
            enable_swap_fee,
            swap_percent_fee,
            fee_receiver,
        } => update_swap_fee_config(deps, info, fee_admin, enable_swap_fee, swap_percent_fee, fee_receiver)
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(
    _deps: DepsMut,
    _env: Env,
    _msg: MigrateMsg,
) -> StdResult<Response> {
    Ok(Response::default())
}

pub fn execute_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let fee_config = SWAP_FEE_CONFIG.may_load(deps.storage)?;

    match fee_config {
        Some(fee_config) => {
            // Calculate fee amount based on message type
            let fee_amount = calculate_fee_amount(amount, &msg, &fee_config);

            // If the fee is non zero then transfer the fee amount to the fee recipient address and execute cw20 send for left amount
            if !fee_amount.is_zero() {
                // Transfer fee to configured receiver address
                transfer(deps.storage, &info.sender, &fee_config.fee_receiver, fee_amount)?;

                let send_amount = amount.sub(fee_amount);
                let res = cw20_execute_send(deps, env, info.clone(), contract.clone(), send_amount, msg)?;

                return Ok(Response::new()
                    .add_attribute("action", "send")
                    .add_attribute("from", &info.sender)
                    .add_attribute("to", &contract)
                    .add_attribute("amount", amount)
                    .add_attribute("fee_amount", fee_amount.to_string())
                    .add_submessages(res.messages));
            }
        }
        None => ()
    }

    Ok(cw20_execute_send(deps, env, info, contract, amount, msg)?)
}

pub fn execute_send_from(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let fee_config = SWAP_FEE_CONFIG.may_load(deps.storage)?;

    match fee_config {
        Some(fee_config) => {
            // Calculate fee amount based on message type
            let fee_amount = calculate_fee_amount(amount, &msg, &fee_config);

            // If the fee is non zero then transfer the fee amount to the fee recipient address and execute cw20 send for left amount
            if !fee_amount.is_zero() {
                // Transfer fee to configured receiver address
                let owner_addr = deps.api.addr_validate(&owner)?;
                transfer(deps.storage, &owner_addr, &fee_config.fee_receiver, fee_amount)?;

                let send_amount = amount.sub(fee_amount);
                let res = cw20_execute_send_from(deps, env, info.clone(), owner.clone(), contract.clone(), send_amount, msg)?;

                return Ok(Response::new()
                    .add_attribute("action", "send_from")
                    .add_attribute("from", &owner)
                    .add_attribute("to", &contract)
                    .add_attribute("by", &info.sender)
                    .add_attribute("amount", amount)
                    .add_attribute("fee_amount", fee_amount.to_string())
                    .add_submessages(res.messages));
            }
        }
        None => ()
    }

    Ok(cw20_execute_send_from(deps, env, info, owner, contract, amount, msg)?)
}

pub fn update_swap_fee_config(
    deps: DepsMut,
    info: MessageInfo,
    fee_admin: Option<String>,
    enable_swap_fee: Option<bool>,
    swap_percent_fee: Option<Decimal>,
    fee_receiver: Option<String>,
) -> Result<Response, ContractError> {
    let mut swap_fee_config = SWAP_FEE_CONFIG
        .may_load(deps.storage)?
        .ok_or(ContractError::Unauthorized {})?;

    if swap_fee_config.fee_admin != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    match fee_admin {
        Some(fee_admin) => swap_fee_config.fee_admin = deps.api.addr_validate(&fee_admin)?,
        None => (),
    }

    match enable_swap_fee {
        Some(enable_swap_fee) => swap_fee_config.enable_swap_fee = enable_swap_fee,
        None => ()
    }

    match swap_percent_fee {
        Some(swap_percent_fee) => swap_fee_config.swap_percent_fee = swap_percent_fee,
        None => ()
    }

    match fee_receiver {
        Some(fee_receiver) => swap_fee_config.fee_receiver = deps.api.addr_validate(&fee_receiver)?,
        None => ()
    }

    SWAP_FEE_CONFIG.save(deps.storage, &swap_fee_config)?;

    Ok(Response::new()
        .add_attribute("method", "update_swap_fee_config"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Balance { address } => to_binary(&query_balance(deps, address)?),
        QueryMsg::TokenInfo {} => to_binary(&query_token_info(deps)?),
        QueryMsg::Minter {} => to_binary(&query_minter(deps)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        }
        QueryMsg::AllAllowances {
            owner,
            start_after,
            limit,
        } => to_binary(&query_all_allowances(deps, owner, start_after, limit)?),
        QueryMsg::AllAccounts { start_after, limit } => {
            to_binary(&query_all_accounts(deps, start_after, limit)?)
        }
        QueryMsg::SwapFeeConfig {} => {
            to_binary(&query_swap_fee_config(deps)?)
        }
    }
}

pub fn query_swap_fee_config(deps: Deps) -> StdResult<SwapFeeConfigResponse> {
    let swap_fee_config = SWAP_FEE_CONFIG.may_load(deps.storage)?;
    match swap_fee_config {
        Some(swap_fee_config) => {
            Ok(SwapFeeConfigResponse {
                fee_admin: swap_fee_config.fee_admin.to_string(),
                enable_swap_fee: swap_fee_config.enable_swap_fee,
                swap_percent_fee: swap_fee_config.swap_percent_fee,
                fee_receiver: swap_fee_config.fee_receiver.to_string(),
            })
        }
        None => Ok(Default::default())
    }
}

fn calculate_fee_amount(amount: Uint128, msg: &Binary, swap_fee_config: &SwapFeeConfig) -> Uint128 {
    if swap_fee_config.enable_swap_fee && is_swap_message(msg.clone()) {
        amount.mul(swap_fee_config.swap_percent_fee).div(Uint128::new(100))
    } else {
        Uint128::zero()
    }
}

fn is_swap_message(msg: Binary) -> bool {
    match from_binary(&msg) {
        Ok(Cw20HookMsg::Swap { .. }) => {
            true
        }
        _ => false
    }
}

fn transfer(
    storage: &mut dyn Storage,
    sender: &Addr,
    recipient: &Addr,
    amount: Uint128,
) -> Result<(), ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    BALANCES.update(
        storage,
        &sender,
        |balance: Option<Uint128>| -> StdResult<_> {
            Ok(balance.unwrap_or_default().checked_sub(amount)?)
        },
    )?;
    BALANCES.update(
        storage,
        &recipient,
        |balance: Option<Uint128>| -> StdResult<_> { Ok(balance.unwrap_or_default() + amount) },
    )?;

    Ok(())
}
