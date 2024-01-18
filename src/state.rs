use cosmwasm_std::Addr;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cw_storage_plus::{Item, Map};

pub const VALIDATOR: Item<String> = Item::new("config");
pub const STAKINGS: Map<String, u128> = Map::new("stakings");
