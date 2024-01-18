use andromeda_std::{andr_exec, andr_instantiate, andr_query};
use cosmwasm_schema::{cw_serde, QueryResponses};

#[andr_instantiate]
#[cw_serde]
pub struct InstantiateMsg {
    pub validator: String,
}

#[andr_exec]
#[cw_serde]
pub enum ExecuteMsg {
    STAKE {},
    UNSTAKE { amount: u128 },
}

#[andr_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    // GetCount returns the current count as a json-encoded number
    #[returns(GetStakedAmount)]
    GetStakedAmount { staker: String },
}

// We define a custom struct for each query response
#[cw_serde]
pub struct GetStakedAmount {
    pub amount: u128,
}
