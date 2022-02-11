use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal};
use cw_storage_plus::Item;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FeeConfig {
    pub enable_swap_fee: bool,
    pub swap_percent_fee: Decimal,
    pub fee_receiver: Addr,
}

pub const FEE_CONFIG: Item<FeeConfig> = Item::new("fee_config");
