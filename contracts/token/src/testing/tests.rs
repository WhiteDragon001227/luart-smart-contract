use std::str::FromStr;

use cosmwasm_std::{Decimal, DepsMut, Env, from_binary, Response, SubMsg, to_binary, Uint128};
use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
use cw20::{BalanceResponse, Cw20Coin, Cw20ReceiveMsg};
use cw20_base::ContractError;
use terraswap::pair::Cw20HookMsg;

use crate::contract::{execute, instantiate, query};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, SwapFeeConfigResponse};

const OWNER: &str = "mock_owner";
const SENDER: &str = "mock_sender";
const FEE_ADMIN: &str = "mock_fee_admin";
const FEE_RECEIVER: &str = "mock_fee_receiver";

fn get_default_instantiate_msg() -> InstantiateMsg {
    InstantiateMsg {
        name: "name".to_string(),
        symbol: "symbol".to_string(),
        decimals: 6,
        initial_balances: vec![
            Cw20Coin {
                address: OWNER.to_string(),
                amount: Uint128::new(1_000_000_000),
            }
        ],
        mint: None,
        swap_fee_config: Some(SwapFeeConfigResponse {
            fee_admin: FEE_ADMIN.to_string(),
            enable_swap_fee: true,
            swap_percent_fee: Decimal::from_str("10").unwrap(),
            fee_receiver: FEE_RECEIVER.to_string(),
        }),
    }
}

fn default_instantiate(
    deps: DepsMut,
    env: Env,
) -> Response {
    let msg = get_default_instantiate_msg();
    let info = mock_info(OWNER, &[]);
    instantiate(deps, env, info, msg).unwrap()
}

#[test]
fn test_update_sawp_fee_config() {
    let mut deps = mock_dependencies(&[]);
    let env = mock_env();
    default_instantiate(deps.as_mut(), env.clone());

    // Querying initial config
    let res = query(deps.as_ref(), env.clone(), QueryMsg::SwapFeeConfig {}).unwrap();
    let config: SwapFeeConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        SwapFeeConfigResponse {
            fee_admin: FEE_ADMIN.to_string(),
            enable_swap_fee: true,
            swap_percent_fee: Decimal::from_str("10").unwrap(),
            fee_receiver: FEE_RECEIVER.to_string(),
        });

    // Cannot update swap fee config by non fee admin
    let err = execute(deps.as_mut(), env.clone(), mock_info(OWNER, &[]),
                      ExecuteMsg::UpdateSwapFeeConfig {
                          fee_admin: None,
                          enable_swap_fee: None,
                          swap_percent_fee: None,
                          fee_receiver: None,
                      }).unwrap_err();
    assert_eq!(err, ContractError::Unauthorized {});

    // Update swap fee config
    execute(deps.as_mut(), env.clone(), mock_info(FEE_ADMIN, &[]),
            ExecuteMsg::UpdateSwapFeeConfig {
                fee_admin: Option::from("new_fee_admin".to_string()),
                enable_swap_fee: Option::from(false),
                swap_percent_fee: Option::from(Decimal::from_str("5").unwrap()),
                fee_receiver: Option::from("new_fee_receiver".to_string()),
            }).unwrap();

    let res = query(deps.as_ref(), env.clone(), QueryMsg::SwapFeeConfig {}).unwrap();
    let config: SwapFeeConfigResponse = from_binary(&res).unwrap();
    assert_eq!(
        config,
        SwapFeeConfigResponse {
            fee_admin: "new_fee_admin".to_string(),
            enable_swap_fee: false,
            swap_percent_fee: Decimal::from_str("5").unwrap(),
            fee_receiver: "new_fee_receiver".to_string(),
        });
}

#[test]
fn test_send() {
    let mut deps = mock_dependencies(&[]);
    let env = mock_env();
    default_instantiate(deps.as_mut(), env.clone());

    let swap_msg = to_binary(&Cw20HookMsg::Swap {
        belief_price: None,
        max_spread: None,
        to: None,
    }).unwrap();

    // Send tokens to smart contract with swap msg
    let res = execute(deps.as_mut(), env.clone(), mock_info(OWNER, &[]),
                      ExecuteMsg::Send {
                          contract: "dex_contract".to_string(),
                          amount: Uint128::new(10_000_000),
                          msg: swap_msg.clone(),
                      }).unwrap();

    // The smart contract should receive the amount of tokens decreased by the swap fee
    assert_eq!(res.messages, vec![
        SubMsg::new(Cw20ReceiveMsg {
            sender: OWNER.to_string(),
            amount: Uint128::new(9_000_000),
            msg: swap_msg,
        }.into_cosmos_msg("dex_contract".to_string()).unwrap()),
    ]);

    // Checking if fee was transfered to the fee receiver address
    let res = query(deps.as_ref(), env.clone(), QueryMsg::Balance {
        address: FEE_RECEIVER.to_string()
    }).unwrap();
    let balance: BalanceResponse = from_binary(&res).unwrap();
    assert_eq!(balance.balance, Uint128::new(1_000_000));
}

#[test]
fn test_send_from() {
    let mut deps = mock_dependencies(&[]);
    let env = mock_env();
    default_instantiate(deps.as_mut(), env.clone());

    let swap_msg = to_binary(&Cw20HookMsg::Swap {
        belief_price: None,
        max_spread: None,
        to: None,
    }).unwrap();

    // Increase allowance for the sender address
    execute(deps.as_mut(), env.clone(), mock_info(OWNER, &[]),
            ExecuteMsg::IncreaseAllowance {
                spender: SENDER.to_string(),
                amount: Uint128::new(10_000_000),
                expires: None,
            }).unwrap();

    // Send tokens to smart contract with swap msg
    let res = execute(deps.as_mut(), env.clone(), mock_info(SENDER, &[]),
                      ExecuteMsg::SendFrom {
                          owner: OWNER.to_string(),
                          contract: "dex_contract".to_string(),
                          amount: Uint128::new(10_000_000),
                          msg: swap_msg.clone(),
                      }).unwrap();

    // The smart contract should receive the amount of tokens decreased by the swap fee
    assert_eq!(res.messages, vec![
        SubMsg::new(Cw20ReceiveMsg {
            sender: SENDER.to_string(),
            amount: Uint128::new(9_000_000),
            msg: swap_msg,
        }.into_cosmos_msg("dex_contract".to_string()).unwrap()),
    ]);

    // Checking if fee was transfered to the fee receiver address
    let res = query(deps.as_ref(), env.clone(), QueryMsg::Balance {
        address: FEE_RECEIVER.to_string()
    }).unwrap();
    let balance: BalanceResponse = from_binary(&res).unwrap();
    assert_eq!(balance.balance, Uint128::new(1_000_000));
}

