use andromeda_std::{
    ado_base::InstantiateMsg as BaseInstantiateMsg,
    ado_contract::{permissioning::is_context_permissioned, ADOContract},
    common::context::ExecuteContext,
    error::ContractError,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{ensure, to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use cw_utils::PaymentError;

use crate::msg::{ExecuteMsg, GetStakedAmount, InstantiateMsg, QueryMsg};
use crate::state::{STAKINGS, VALIDATOR};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:andr-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let contract = ADOContract::default();

    let resp = contract.instantiate(
        deps.storage,
        env,
        deps.api,
        info.clone(),
        BaseInstantiateMsg {
            ado_type: "andr-staking".to_string(),
            ado_version: CONTRACT_VERSION.to_string(),
            operators: None,
            kernel_address: msg.kernel_address,
            owner: msg.owner,
        },
    )?;

    VALIDATOR.save(deps.storage, &msg.validator);

    Ok(resp
        .add_attribute("method", "instantiate")
        .add_attribute("owner", info.sender)
        .add_attribute("validator", msg.validator.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    let ctx = ExecuteContext::new(deps, info, env);
    if let ExecuteMsg::AMPReceive(pkt) = msg {
        ADOContract::default().execute_amp_receive(ctx, pkt, handle_execute)
    } else {
        handle_execute(ctx, msg)
    }
}

pub fn handle_execute(ctx: ExecuteContext, msg: ExecuteMsg) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::STAKE {} => execute::stake(ctx),
        ExecuteMsg::UNSTAKE { amount } => execute::unstake(ctx, amount),
        _ => ADOContract::default().execute(ctx, msg),
    }
}

pub mod execute {
    use cosmwasm_std::{coin, BankMsg, Event, StakingMsg};

    use super::*;

    pub fn stake(ctx: ExecuteContext) -> Result<Response, ContractError> {
        let ExecuteContext { deps, info, .. } = ctx;
        let validator = VALIDATOR.load(deps.storage);
        let fund = info.funds.iter().find(|fund| fund.denom == "andr");
        if info.funds.len() == 0 {
            return Err(ContractError::Payment(PaymentError::NoFunds {}));
        } else if info.funds.len() > 1 {
            return Err(ContractError::Payment(PaymentError::MultipleDenoms {}));
        }
        if fund.is_none() {
            return Err(ContractError::Payment(PaymentError::MissingDenom(
                "andr".to_owned(),
            )));
        }
        let staked_amount = STAKINGS.may_load(deps.storage, info.sender.to_string())?;
        let _ = match staked_amount {
            None => STAKINGS.save(
                deps.storage,
                info.sender.to_string(),
                &u128::from(fund.clone().unwrap().amount),
            ),
            _ => STAKINGS.save(
                deps.storage,
                info.sender.to_string(),
                &(staked_amount.unwrap() + u128::from(fund.clone().unwrap().amount)),
            ),
        };
        Ok(Response::new()
            .add_message(StakingMsg::Delegate {
                validator: validator.unwrap(),
                amount: fund.unwrap().clone(),
            })
            .add_event(
                Event::new("staked")
                    .add_attribute("staker", info.sender.to_string())
                    .add_attribute("amount", fund.unwrap().to_string()),
            ))
    }

    pub fn unstake(ctx: ExecuteContext, amount: u128) -> Result<Response, ContractError> {
        let ExecuteContext { deps, info, .. } = ctx;
        let validator = VALIDATOR.load(deps.storage);
        let staked_amount = STAKINGS.may_load(deps.storage, info.sender.to_string())?;
        let _ = match staked_amount {
            None => return Err(ContractError::UserNotFound {}),
            _ => {
                if staked_amount.clone().unwrap() < amount {
                    return Err(ContractError::NotEnoughTokens {});
                }
                STAKINGS.save(
                    deps.storage,
                    info.sender.to_string(),
                    &(staked_amount.unwrap() - amount),
                )
            }
        };
        Ok(Response::new()
            .add_message(StakingMsg::Undelegate {
                validator: validator.unwrap(),
                amount: coin(amount, "andr"),
            })
            .add_message(BankMsg::Send {
                to_address: info.sender.to_string(),
                amount: vec![coin(amount, "andr")],
            })
            .add_event(
                Event::new("unstaked")
                    .add_attribute("staker", info.sender.to_string())
                    .add_attribute("amount", coin(amount, "andr").to_string()),
            ))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::GetStakedAmount { staker } => {
            Ok(to_binary(&query::get_staked_amount(deps, staker)?)?)
        }
        _ => ADOContract::default().query(deps, env, msg),
    }
}

pub mod query {
    use super::*;

    pub fn get_staked_amount(deps: Deps, staker: String) -> Result<GetStakedAmount, ContractError> {
        let staked_amount = STAKINGS.may_load(deps.storage, staker)?;
        match staked_amount {
            None => Ok(GetStakedAmount { amount: 0 }),
            _ => Ok(GetStakedAmount {
                amount: staked_amount.unwrap(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use andromeda_std::testing::mock_querier::MOCK_KERNEL_CONTRACT;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, from_binary};

    #[test]
    fn stake() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            validator: "valoper1".to_owned(),
            kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
            owner: None,
        };

        let info = mock_info("creator", &vec![]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &coins(2, "andr"));
        let msg = ExecuteMsg::STAKE {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetStakedAmount {
                staker: "anyone".to_owned(),
            },
        )
        .unwrap();
        let value: GetStakedAmount = from_binary(&res).unwrap();
        assert_eq!(2, value.amount);
    }

    #[test]
    fn unstake() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {
            validator: "valoper1".to_owned(),
            kernel_address: MOCK_KERNEL_CONTRACT.to_string(),
            owner: None,
        };

        let info = mock_info("creator", &vec![]);
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &coins(2, "andr"));
        let msg = ExecuteMsg::STAKE {};
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let info = mock_info("anyone", &vec![]);
        let msg = ExecuteMsg::UNSTAKE { amount: 1 };
        let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        let res = query(
            deps.as_ref(),
            mock_env(),
            QueryMsg::GetStakedAmount {
                staker: "anyone".to_owned(),
            },
        )
        .unwrap();
        let value: GetStakedAmount = from_binary(&res).unwrap();
        assert_eq!(1, value.amount);
    }
}
