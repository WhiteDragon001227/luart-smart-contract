use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError, StdResult};
use cosmwasm_std::entry_point;
use cw20_base::allowances::{
    execute_burn_from as cw20_execute_burn_from,
    execute_decrease_allowance as cw20_execute_decrease_allowance,
    execute_increase_allowance as cw20_execute_increase_allowance,
    execute_send_from as cw20_execute_send_from,
    execute_transfer_from as cw20_execute_transfer_from,
};
use cw20_base::contract::{
    create_accounts, execute_burn as cw20_execute_burn,
    execute_mint as cw20_execute_mint,
    execute_send as cw20_execute_send,
    execute_transfer as cw20_execute_transfer, execute_update_marketing as cw20_execute_update_marketing,
    execute_upload_logo as cw20_execute_upload_logo,
    query as cw20_query,
};
use cw20_base::ContractError;
use cw20_base::msg::{ExecuteMsg, QueryMsg};
use cw20_base::state::{MinterData, TOKEN_INFO, TokenInfo};
use cw2::set_contract_version;

use crate::msg::{InstantiateMsg, MigrateMsg};
use crate::state::{FEE_CONFIG, FeeConfig};

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

    // store fee config
    let fee_config = FeeConfig{
        enable_swap_fee: msg.enable_swap_fee,
        swap_percent_fee: msg.swap_percent_fee,
        fee_receiver: deps.api.addr_validate(&msg.fee_receiver)?,
    };

    FEE_CONFIG.save(deps.storage, &fee_config)?;

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
        } => cw20_execute_send(deps, env, info, contract, amount, msg),
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
        } => cw20_execute_send_from(deps, env, info, owner, contract, amount, msg),
        ExecuteMsg::UpdateMarketing {
            project,
            description,
            marketing,
        } => cw20_execute_update_marketing(deps, env, info, project, description, marketing),
        ExecuteMsg::UploadLogo(logo) => cw20_execute_upload_logo(deps, env, info, logo),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    cw20_query(deps, env, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(
    _deps: DepsMut,
    _env: Env,
    _msg: MigrateMsg,
) -> StdResult<Response> {
    Ok(Response::default())
}
